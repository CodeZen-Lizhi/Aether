use super::super::provider_transport;
use super::{
    AppState, CachedProviderTransportSnapshot, GatewayError, ProviderTransportSnapshotCacheKey,
    ProviderTransportSnapshotFlight, ProviderTransportSnapshotFlightResult,
    PROVIDER_TRANSPORT_SNAPSHOT_CACHE_MAX_ENTRIES, PROVIDER_TRANSPORT_SNAPSHOT_CACHE_STALE_TTL,
    PROVIDER_TRANSPORT_SNAPSHOT_CACHE_TTL,
};
use dashmap::{mapref::entry::Entry as DashMapEntry, DashMap};
use std::sync::atomic::Ordering;
use std::sync::Arc;

enum ProviderTransportSnapshotCacheLookup {
    Fresh(Arc<provider_transport::GatewayProviderTransportSnapshot>),
    Stale(Arc<provider_transport::GatewayProviderTransportSnapshot>),
    Miss,
}

enum ProviderTransportSnapshotReloadResult {
    Published(Arc<provider_transport::GatewayProviderTransportSnapshot>),
    Missing,
    Invalidated,
}

enum ProviderTransportSnapshotInflightRegistration {
    Leader(ProviderTransportSnapshotInflightGuard),
    Follower(Arc<ProviderTransportSnapshotFlight>),
    Retry,
}

struct ProviderTransportSnapshotInflightGuard {
    inflight: Arc<DashMap<ProviderTransportSnapshotCacheKey, Arc<ProviderTransportSnapshotFlight>>>,
    cache_key: Option<ProviderTransportSnapshotCacheKey>,
    flight: Arc<ProviderTransportSnapshotFlight>,
}

impl ProviderTransportSnapshotInflightGuard {
    fn generation(&self) -> u64 {
        self.flight.generation()
    }

    fn generation_is_current(&self, state: &AppState) -> bool {
        state
            .provider_transport_snapshot_cache_generation
            .load(Ordering::Acquire)
            == self.generation()
    }

    fn finish(&mut self, result: ProviderTransportSnapshotFlightResult) {
        let Some(cache_key) = self.cache_key.take() else {
            return;
        };
        // Publish completion before exposing a vacant map entry. Requests in
        // this small window join the completed flight instead of issuing a
        // duplicate reload for a missing/error result.
        self.flight.complete(result);
        self.inflight
            .remove_if(&cache_key, |_, current| Arc::ptr_eq(current, &self.flight));
    }
}

impl Drop for ProviderTransportSnapshotInflightGuard {
    fn drop(&mut self) {
        // Cancellation must release the key and wake every follower. One of
        // them can then claim leadership and retry the interrupted load.
        self.finish(ProviderTransportSnapshotFlightResult::Retry);
    }
}

fn provider_transport_snapshot_flight_result(
    result: &Result<ProviderTransportSnapshotReloadResult, GatewayError>,
) -> ProviderTransportSnapshotFlightResult {
    match result {
        Ok(ProviderTransportSnapshotReloadResult::Published(snapshot)) => {
            ProviderTransportSnapshotFlightResult::Published(Arc::clone(snapshot))
        }
        Ok(ProviderTransportSnapshotReloadResult::Missing) => {
            ProviderTransportSnapshotFlightResult::Missing
        }
        Ok(ProviderTransportSnapshotReloadResult::Invalidated) => {
            ProviderTransportSnapshotFlightResult::Invalidated
        }
        Err(err) => ProviderTransportSnapshotFlightResult::Error(err.clone()),
    }
}

impl AppState {
    pub(crate) fn clear_provider_transport_snapshot_cache(&self) {
        self.provider_transport_snapshot_cache_generation
            .fetch_add(1, Ordering::AcqRel);
        self.provider_transport_snapshot_cache.clear();

        // Keep a concurrently-created flight from the new generation. Every
        // older flight is completed as invalidated so its followers retry
        // immediately instead of waiting for the old database read to finish.
        let mut invalidated = Vec::new();
        self.provider_transport_snapshot_inflight
            .retain(|_, flight| {
                let current_generation = self
                    .provider_transport_snapshot_cache_generation
                    .load(Ordering::Acquire);
                if flight.generation() < current_generation {
                    invalidated.push(Arc::clone(flight));
                    false
                } else {
                    true
                }
            });
        for flight in invalidated {
            flight.complete(ProviderTransportSnapshotFlightResult::Invalidated);
        }
    }

    fn register_provider_transport_snapshot_inflight(
        &self,
        cache_key: &ProviderTransportSnapshotCacheKey,
        generation: u64,
    ) -> ProviderTransportSnapshotInflightRegistration {
        let flight = Arc::new(ProviderTransportSnapshotFlight::new(generation));
        match self
            .provider_transport_snapshot_inflight
            .entry(cache_key.clone())
        {
            DashMapEntry::Occupied(entry) => {
                let current = Arc::clone(entry.get());
                if current.generation() == generation {
                    return ProviderTransportSnapshotInflightRegistration::Follower(current);
                }

                // A caller that observed an older generation must never evict
                // a newer flight. If this caller is current, the occupied
                // entry is left over from a clear that has not retained its
                // shard yet and can be invalidated here.
                if self
                    .provider_transport_snapshot_cache_generation
                    .load(Ordering::Acquire)
                    != generation
                {
                    return ProviderTransportSnapshotInflightRegistration::Retry;
                }
                let invalidated = entry.remove();
                invalidated.complete(ProviderTransportSnapshotFlightResult::Invalidated);
                ProviderTransportSnapshotInflightRegistration::Retry
            }
            DashMapEntry::Vacant(entry) => {
                if self
                    .provider_transport_snapshot_cache_generation
                    .load(Ordering::Acquire)
                    != generation
                {
                    return ProviderTransportSnapshotInflightRegistration::Retry;
                }
                entry.insert(Arc::clone(&flight));
                ProviderTransportSnapshotInflightRegistration::Leader(
                    ProviderTransportSnapshotInflightGuard {
                        inflight: Arc::clone(&self.provider_transport_snapshot_inflight),
                        cache_key: Some(cache_key.clone()),
                        flight,
                    },
                )
            }
        }
    }

    fn get_cached_provider_transport_snapshot_arc(
        &self,
        cache_key: &ProviderTransportSnapshotCacheKey,
    ) -> ProviderTransportSnapshotCacheLookup {
        let cached = self
            .provider_transport_snapshot_cache
            .get(cache_key)
            .map(|entry| entry.clone());
        let Some(cached) = cached else {
            return ProviderTransportSnapshotCacheLookup::Miss;
        };
        if cached.generation
            != self
                .provider_transport_snapshot_cache_generation
                .load(Ordering::Acquire)
        {
            self.provider_transport_snapshot_cache
                .remove_if(cache_key, |_, current| {
                    current.generation == cached.generation
                });
            return ProviderTransportSnapshotCacheLookup::Miss;
        }
        let age = cached.loaded_at.elapsed();
        if age <= PROVIDER_TRANSPORT_SNAPSHOT_CACHE_TTL {
            return ProviderTransportSnapshotCacheLookup::Fresh(cached.snapshot);
        }
        if age <= PROVIDER_TRANSPORT_SNAPSHOT_CACHE_STALE_TTL {
            return ProviderTransportSnapshotCacheLookup::Stale(cached.snapshot);
        }
        if self
            .provider_transport_snapshot_cache
            .get(cache_key)
            .is_some_and(|entry| {
                entry.loaded_at.elapsed() > PROVIDER_TRANSPORT_SNAPSHOT_CACHE_STALE_TTL
            })
        {
            self.provider_transport_snapshot_cache
                .remove_if(cache_key, |_, current| {
                    current.generation == cached.generation
                });
        }
        ProviderTransportSnapshotCacheLookup::Miss
    }

    fn put_cached_provider_transport_snapshot(
        &self,
        cache_key: ProviderTransportSnapshotCacheKey,
        snapshot: Arc<provider_transport::GatewayProviderTransportSnapshot>,
        generation: u64,
    ) -> bool {
        if generation
            != self
                .provider_transport_snapshot_cache_generation
                .load(Ordering::Acquire)
        {
            return false;
        }
        if self.provider_transport_snapshot_cache.len()
            >= PROVIDER_TRANSPORT_SNAPSHOT_CACHE_MAX_ENTRIES
        {
            self.provider_transport_snapshot_cache.retain(|_, entry| {
                entry.loaded_at.elapsed() <= PROVIDER_TRANSPORT_SNAPSHOT_CACHE_STALE_TTL
            });
            if self.provider_transport_snapshot_cache.len()
                >= PROVIDER_TRANSPORT_SNAPSHOT_CACHE_MAX_ENTRIES
            {
                let oldest_key = self
                    .provider_transport_snapshot_cache
                    .iter()
                    .min_by_key(|entry| entry.value().loaded_at)
                    .map(|entry| entry.key().clone());
                if let Some(oldest_key) = oldest_key {
                    self.provider_transport_snapshot_cache.remove(&oldest_key);
                }
            }
        }
        self.provider_transport_snapshot_cache.insert(
            cache_key.clone(),
            CachedProviderTransportSnapshot {
                loaded_at: std::time::Instant::now(),
                generation,
                snapshot,
            },
        );
        if generation
            != self
                .provider_transport_snapshot_cache_generation
                .load(Ordering::Acquire)
        {
            self.provider_transport_snapshot_cache
                .remove_if(&cache_key, |_, current| current.generation == generation);
            return false;
        }
        true
    }

    async fn reload_provider_transport_snapshot(
        &self,
        cache_key: &ProviderTransportSnapshotCacheKey,
        provider_id: &str,
        endpoint_id: &str,
        key_id: &str,
        generation: u64,
    ) -> Result<ProviderTransportSnapshotReloadResult, GatewayError> {
        if generation
            != self
                .provider_transport_snapshot_cache_generation
                .load(Ordering::Acquire)
        {
            return Ok(ProviderTransportSnapshotReloadResult::Invalidated);
        }

        let loaded = self
            .read_provider_transport_snapshot_uncached(provider_id, endpoint_id, key_id)
            .await?;
        if generation
            != self
                .provider_transport_snapshot_cache_generation
                .load(Ordering::Acquire)
        {
            return Ok(ProviderTransportSnapshotReloadResult::Invalidated);
        }

        let Some(snapshot) = loaded else {
            return Ok(ProviderTransportSnapshotReloadResult::Missing);
        };
        let snapshot = self.apply_global_format_conversion_override(snapshot).await;
        if generation
            != self
                .provider_transport_snapshot_cache_generation
                .load(Ordering::Acquire)
        {
            return Ok(ProviderTransportSnapshotReloadResult::Invalidated);
        }

        let snapshot = Arc::new(snapshot);
        if self.put_cached_provider_transport_snapshot(
            cache_key.clone(),
            Arc::clone(&snapshot),
            generation,
        ) {
            Ok(ProviderTransportSnapshotReloadResult::Published(snapshot))
        } else {
            Ok(ProviderTransportSnapshotReloadResult::Invalidated)
        }
    }

    fn start_provider_transport_snapshot_background_refresh(
        &self,
        cache_key: ProviderTransportSnapshotCacheKey,
        provider_id: String,
        endpoint_id: String,
        key_id: String,
    ) {
        let mut inflight_guard = loop {
            let generation = self
                .provider_transport_snapshot_cache_generation
                .load(Ordering::Acquire);
            match self.register_provider_transport_snapshot_inflight(&cache_key, generation) {
                ProviderTransportSnapshotInflightRegistration::Leader(guard) => break guard,
                ProviderTransportSnapshotInflightRegistration::Follower(_) => return,
                ProviderTransportSnapshotInflightRegistration::Retry => continue,
            }
        };
        let generation = inflight_guard.generation();
        let state = self.clone();
        tokio::spawn(async move {
            let result = state
                .reload_provider_transport_snapshot(
                    &cache_key,
                    &provider_id,
                    &endpoint_id,
                    &key_id,
                    generation,
                )
                .await;
            if matches!(&result, Ok(ProviderTransportSnapshotReloadResult::Missing))
                && state
                    .provider_transport_snapshot_cache_generation
                    .load(Ordering::Acquire)
                    == generation
            {
                state
                    .provider_transport_snapshot_cache
                    .remove_if(&cache_key, |_, current| current.generation == generation);
            }
            let flight_result = if inflight_guard.generation_is_current(&state) {
                provider_transport_snapshot_flight_result(&result)
            } else {
                ProviderTransportSnapshotFlightResult::Invalidated
            };
            inflight_guard.finish(flight_result);
        });
    }

    pub(crate) async fn read_provider_transport_snapshot_uncached(
        &self,
        provider_id: &str,
        endpoint_id: &str,
        key_id: &str,
    ) -> Result<Option<crate::provider_transport::GatewayProviderTransportSnapshot>, GatewayError>
    {
        self.data
            .read_provider_transport_snapshot(provider_id, endpoint_id, key_id)
            .await
            .map_err(|err| GatewayError::Internal(err.to_string()))
    }

    async fn apply_global_format_conversion_override(
        &self,
        mut snapshot: crate::provider_transport::GatewayProviderTransportSnapshot,
    ) -> crate::provider_transport::GatewayProviderTransportSnapshot {
        let global_config =
            Box::pin(self.read_system_config_json_value("enable_format_conversion"))
                .await
                .ok()
                .flatten();
        let global_enabled = global_config
            .and_then(|value| value.as_bool())
            .unwrap_or(false);
        if global_enabled {
            snapshot.provider.enable_format_conversion = true;
        }
        snapshot
    }

    pub(crate) fn encryption_key(&self) -> Option<&str> {
        self.data.encryption_key()
    }

    pub(crate) fn has_auth_module_writer(&self) -> bool {
        self.data.has_auth_module_writer()
    }

    pub(crate) async fn read_provider_transport_snapshot_arc(
        &self,
        provider_id: &str,
        endpoint_id: &str,
        key_id: &str,
    ) -> Result<
        Option<Arc<crate::provider_transport::GatewayProviderTransportSnapshot>>,
        GatewayError,
    > {
        let Some(cache_key) =
            ProviderTransportSnapshotCacheKey::new(provider_id, endpoint_id, key_id)
        else {
            return Ok(self
                .read_provider_transport_snapshot_uncached(provider_id, endpoint_id, key_id)
                .await?
                .map(Arc::new));
        };
        loop {
            match self.get_cached_provider_transport_snapshot_arc(&cache_key) {
                ProviderTransportSnapshotCacheLookup::Fresh(snapshot) => {
                    return Ok(Some(snapshot));
                }
                ProviderTransportSnapshotCacheLookup::Stale(snapshot) => {
                    self.start_provider_transport_snapshot_background_refresh(
                        cache_key.clone(),
                        provider_id.to_string(),
                        endpoint_id.to_string(),
                        key_id.to_string(),
                    );
                    return Ok(Some(snapshot));
                }
                ProviderTransportSnapshotCacheLookup::Miss => {}
            }

            let generation = self
                .provider_transport_snapshot_cache_generation
                .load(Ordering::Acquire);
            match self.register_provider_transport_snapshot_inflight(&cache_key, generation) {
                ProviderTransportSnapshotInflightRegistration::Retry => continue,
                ProviderTransportSnapshotInflightRegistration::Follower(flight) => {
                    let flight_generation = flight.generation();
                    let result = flight.wait().await;
                    if self
                        .provider_transport_snapshot_cache_generation
                        .load(Ordering::Acquire)
                        != flight_generation
                    {
                        continue;
                    }
                    match result {
                        ProviderTransportSnapshotFlightResult::Published(snapshot) => {
                            return Ok(Some(snapshot));
                        }
                        ProviderTransportSnapshotFlightResult::Missing => return Ok(None),
                        ProviderTransportSnapshotFlightResult::Error(err) => return Err(err),
                        ProviderTransportSnapshotFlightResult::Invalidated
                        | ProviderTransportSnapshotFlightResult::Retry => continue,
                    }
                }
                ProviderTransportSnapshotInflightRegistration::Leader(mut inflight_guard) => {
                    if !inflight_guard.generation_is_current(self) {
                        inflight_guard.finish(ProviderTransportSnapshotFlightResult::Invalidated);
                        continue;
                    }

                    // A different flight may have published between the first
                    // cache check and this registration. Recheck before doing
                    // the only database reload for this flight.
                    if let ProviderTransportSnapshotCacheLookup::Fresh(snapshot) =
                        self.get_cached_provider_transport_snapshot_arc(&cache_key)
                    {
                        if !inflight_guard.generation_is_current(self) {
                            inflight_guard
                                .finish(ProviderTransportSnapshotFlightResult::Invalidated);
                            continue;
                        }
                        inflight_guard.finish(ProviderTransportSnapshotFlightResult::Published(
                            Arc::clone(&snapshot),
                        ));
                        if !inflight_guard.generation_is_current(self) {
                            continue;
                        }
                        return Ok(Some(snapshot));
                    }

                    let result = self
                        .reload_provider_transport_snapshot(
                            &cache_key,
                            provider_id,
                            endpoint_id,
                            key_id,
                            generation,
                        )
                        .await;
                    let flight_result = if inflight_guard.generation_is_current(self) {
                        provider_transport_snapshot_flight_result(&result)
                    } else {
                        ProviderTransportSnapshotFlightResult::Invalidated
                    };
                    inflight_guard.finish(flight_result);
                    if !inflight_guard.generation_is_current(self) {
                        continue;
                    }
                    match result {
                        Ok(ProviderTransportSnapshotReloadResult::Published(snapshot)) => {
                            return Ok(Some(snapshot));
                        }
                        Ok(ProviderTransportSnapshotReloadResult::Missing) => return Ok(None),
                        Ok(ProviderTransportSnapshotReloadResult::Invalidated) => continue,
                        Err(err) => return Err(err),
                    }
                }
            }
        }
    }

    pub(crate) async fn read_provider_transport_snapshot(
        &self,
        provider_id: &str,
        endpoint_id: &str,
        key_id: &str,
    ) -> Result<Option<crate::provider_transport::GatewayProviderTransportSnapshot>, GatewayError>
    {
        Ok(self
            .read_provider_transport_snapshot_arc(provider_id, endpoint_id, key_id)
            .await?
            .map(|snapshot| (*snapshot).clone()))
    }
}
