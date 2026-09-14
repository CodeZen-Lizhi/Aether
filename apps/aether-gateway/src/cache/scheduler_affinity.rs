use std::sync::Mutex;
use std::time::Duration;

use aether_cache::{ExpiringMap, ExpiringMapFreshEntry};
pub(crate) use aether_scheduler_core::SchedulerAffinityTarget;

#[derive(Debug, Default)]
pub(crate) struct SchedulerAffinityCache {
    entries: ExpiringMap<String, SchedulerAffinityCacheValue>,
    minimum_epoch: Mutex<u64>,
    pub(crate) runtime_write_gate: tokio::sync::Mutex<()>,
}

#[cfg(test)]
mod tests {
    use super::*;

    fn target(key: &str) -> SchedulerAffinityTarget {
        SchedulerAffinityTarget {
            provider_id: "provider-1".to_string(),
            endpoint_id: "endpoint-1".to_string(),
            key_id: key.to_string(),
        }
    }

    #[test]
    fn late_older_request_and_duplicate_cannot_overwrite_newer_binding() {
        let cache = SchedulerAffinityCache::default();
        let old = uuid::Uuid::now_v7();
        let new = uuid::Uuid::now_v7();
        let ttl = Duration::from_secs(300);
        assert!(cache.insert_for_request(
            "session".to_string(),
            target("new"),
            ttl,
            10,
            1,
            Some(new)
        ));
        assert!(!cache.insert_for_request(
            "session".to_string(),
            target("old"),
            ttl,
            10,
            1,
            Some(old)
        ));
        assert!(!cache.insert_for_request(
            "session".to_string(),
            target("duplicate"),
            ttl,
            10,
            1,
            Some(new)
        ));
        assert!(!cache.insert_for_epoch("session".to_string(), target("unversioned"), ttl, 10, 1));
        assert_eq!(
            cache.get_fresh_for_epoch("session", ttl, 1),
            Some(target("new"))
        );
        assert!(cache.insert_for_request(
            "other-session".to_string(),
            target("old"),
            ttl,
            10,
            1,
            Some(old)
        ));
    }

    #[test]
    fn concurrent_request_completions_keep_the_latest_request() {
        let cache = std::sync::Arc::new(SchedulerAffinityCache::default());
        let orders: Vec<_> = (0..16).map(|_| uuid::Uuid::now_v7()).collect();
        let latest = *orders.last().unwrap();
        let barrier = std::sync::Arc::new(std::sync::Barrier::new(orders.len()));
        std::thread::scope(|scope| {
            for order in orders {
                let cache = cache.clone();
                let barrier = barrier.clone();
                scope.spawn(move || {
                    barrier.wait();
                    cache.insert_for_request(
                        "session".to_string(),
                        target(&order.to_string()),
                        Duration::from_secs(300),
                        10,
                        1,
                        Some(order),
                    );
                });
            }
        });
        assert_eq!(
            cache.get_fresh_for_epoch("session", Duration::from_secs(300), 1),
            Some(target(&latest.to_string()))
        );
    }

    #[test]
    fn invalidation_rejects_stale_epoch_even_when_cache_is_empty() {
        let cache = SchedulerAffinityCache::default();
        let ttl = Duration::from_secs(300);
        cache.clear_for_epoch(2);
        assert!(!cache.insert_for_request(
            "session".to_string(),
            target("old-epoch"),
            ttl,
            10,
            1,
            Some(uuid::Uuid::now_v7())
        ));
        assert!(cache.insert_for_request(
            "session".to_string(),
            target("current"),
            ttl,
            10,
            2,
            Some(uuid::Uuid::now_v7())
        ));
        assert_eq!(
            cache.get_fresh_for_epoch("session", ttl, 2),
            Some(target("current"))
        );
        cache.clear_for_epoch(1);
        assert_eq!(
            cache.get_fresh_for_epoch("session", ttl, 2),
            Some(target("current"))
        );
    }
}

#[derive(Debug, Clone)]
struct SchedulerAffinityCacheValue {
    target: SchedulerAffinityTarget,
    epoch: u64,
    request_order: Option<uuid::Uuid>,
}

#[derive(Debug, Clone)]
pub(crate) struct SchedulerAffinitySnapshotEntry {
    pub(crate) cache_key: String,
    pub(crate) target: SchedulerAffinityTarget,
    pub(crate) epoch: u64,
    pub(crate) age: Duration,
}

impl SchedulerAffinityCache {
    pub(crate) fn get_fresh(
        &self,
        cache_key: &str,
        ttl: Duration,
    ) -> Option<SchedulerAffinityTarget> {
        self.entries
            .get_fresh(&cache_key.to_string(), ttl)
            .map(|value| value.target)
    }

    pub(crate) fn get_fresh_for_epoch(
        &self,
        cache_key: &str,
        ttl: Duration,
        epoch: u64,
    ) -> Option<SchedulerAffinityTarget> {
        let value = self.entries.get_fresh(&cache_key.to_string(), ttl)?;
        (value.epoch == epoch).then_some(value.target)
    }

    #[cfg_attr(not(test), allow(dead_code))]
    pub(crate) fn insert(
        &self,
        cache_key: String,
        target: SchedulerAffinityTarget,
        ttl: Duration,
        max_entries: usize,
    ) {
        self.insert_for_epoch(cache_key, target, ttl, max_entries, 0);
    }

    pub(crate) fn insert_for_epoch(
        &self,
        cache_key: String,
        target: SchedulerAffinityTarget,
        ttl: Duration,
        max_entries: usize,
        epoch: u64,
    ) -> bool {
        self.insert_for_request(cache_key, target, ttl, max_entries, epoch, None)
    }

    pub(crate) fn insert_for_request(
        &self,
        cache_key: String,
        target: SchedulerAffinityTarget,
        ttl: Duration,
        max_entries: usize,
        epoch: u64,
        request_order: Option<uuid::Uuid>,
    ) -> bool {
        // Serialize the comparison with mutation and epoch invalidation. The
        // underlying TTL map's independent get/insert locks are not a CAS.
        let Ok(minimum_epoch) = self.minimum_epoch.lock() else {
            return false;
        };
        if epoch < *minimum_epoch || ttl.is_zero() || max_entries == 0 {
            return false;
        }
        if let Some(current) = self.entries.get_fresh(&cache_key, ttl) {
            if current.epoch > epoch
                || (current.epoch == epoch
                    && current.request_order.is_some()
                    && request_order <= current.request_order)
            {
                return false;
            }
        }
        self.entries.insert(
            cache_key,
            SchedulerAffinityCacheValue {
                target,
                epoch,
                request_order,
            },
            ttl,
            max_entries,
        );
        true
    }

    pub(crate) fn get_fresh_write_for_epoch(
        &self,
        cache_key: &str,
        ttl: Duration,
        epoch: u64,
    ) -> Option<(SchedulerAffinityTarget, Option<uuid::Uuid>)> {
        let value = self.entries.get_fresh(&cache_key.to_string(), ttl)?;
        (value.epoch == epoch).then_some((value.target, value.request_order))
    }

    pub(crate) fn remove(&self, cache_key: &str) -> Option<SchedulerAffinityTarget> {
        let _guard = self.minimum_epoch.lock().ok()?;
        self.entries
            .remove(&cache_key.to_string())
            .map(|value| value.target)
    }

    pub(crate) fn clear_for_epoch(&self, epoch: u64) {
        let Ok(mut minimum_epoch) = self.minimum_epoch.lock() else {
            return;
        };
        if epoch <= *minimum_epoch {
            return;
        }
        *minimum_epoch = epoch;
        self.entries.clear();
    }

    pub(crate) fn fresh_entries(&self, ttl: Duration) -> Vec<SchedulerAffinitySnapshotEntry> {
        self.entries
            .snapshot_fresh(ttl)
            .into_iter()
            .map(
                |ExpiringMapFreshEntry { key, value, age }| SchedulerAffinitySnapshotEntry {
                    cache_key: key,
                    target: value.target,
                    epoch: value.epoch,
                    age,
                },
            )
            .collect()
    }

    pub(crate) fn fresh_entries_for_epoch(
        &self,
        ttl: Duration,
        epoch: u64,
    ) -> Vec<SchedulerAffinitySnapshotEntry> {
        self.fresh_entries(ttl)
            .into_iter()
            .filter(|entry| entry.epoch == epoch)
            .collect()
    }
}
