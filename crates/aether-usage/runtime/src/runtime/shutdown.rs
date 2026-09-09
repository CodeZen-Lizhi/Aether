use std::sync::atomic::{AtomicUsize, Ordering};
use std::sync::Arc;
use std::time::Duration;

use aether_data_contracts::DataLayerError;
use tokio::time::{sleep, timeout, Instant};

use super::{UsageRuntime, UsageRuntimeAccess};

/// Retains shutdown ownership while a detached producer prepares a usage event.
#[derive(Debug)]
pub struct UsagePersistenceHandoff {
    pending: Option<Arc<AtomicUsize>>,
}

impl Drop for UsagePersistenceHandoff {
    fn drop(&mut self) {
        if let Some(pending) = &self.pending {
            pending.fetch_sub(1, Ordering::AcqRel);
        }
    }
}

impl UsageRuntime {
    /// Register before spawning a terminal-report task and move the guard into
    /// that task. It must remain alive through event submission, including
    /// any preparation that happens before the regular dispatcher counters.
    pub fn track_persistence_handoff(&self) -> UsagePersistenceHandoff {
        let pending = self.is_enabled().then(|| {
            self.persistence_handoffs.fetch_add(1, Ordering::AcqRel);
            Arc::clone(&self.persistence_handoffs)
        });
        UsagePersistenceHandoff { pending }
    }

    /// Wait for local dispatchers and the persistence queue to become quiet.
    ///
    /// The caller must first stop request producers and keep queue workers
    /// running. This is a bounded shutdown barrier, not an admission lock:
    /// submitting new events after it returns invalidates the idle result.
    /// Queue observation failures propagate; timeout returns `Ok(false)`.
    pub async fn wait_for_idle<T>(
        &self,
        data: &T,
        max_wait: Duration,
    ) -> Result<bool, DataLayerError>
    where
        T: UsageRuntimeAccess,
    {
        if !self.is_enabled() {
            return Ok(true);
        }

        let wait = async {
            let mut quiet_since = None;
            loop {
                let mut idle = self.local_persistence_is_idle();
                if idle {
                    let queue = self.queue_health_snapshot(data).await?;
                    idle = (!queue.configured
                        || (queue.group_pending == 0
                            && queue.group_lag.unwrap_or(queue.stream_length) == 0))
                        && self.local_persistence_is_idle();
                }
                if idle {
                    let since = quiet_since.get_or_insert_with(Instant::now);
                    // The dispatchers run on a separate runtime. Confirm a
                    // quiet interval across queue observation and handoffs,
                    // rather than trusting one non-atomic metrics snapshot.
                    if since.elapsed() >= Duration::from_millis(25) {
                        return Ok(true);
                    }
                } else {
                    quiet_since = None;
                }
                sleep(Duration::from_millis(10)).await;
            }
        };
        timeout(max_wait, wait).await.unwrap_or(Ok(false))
    }

    fn local_persistence_is_idle(&self) -> bool {
        let metrics = self.metrics_snapshot();
        self.persistence_handoffs.load(Ordering::Acquire) == 0
            && metrics.terminal_submission_pending == 0
            && metrics.terminal_submission_in_flight == 0
            && metrics.terminal_enqueue_in_flight == 0
            && metrics.terminal_direct_fallback_in_flight == 0
            && metrics.lifecycle_submission_pending == 0
            && metrics.ordered_lifecycle_pending == 0
            && metrics.pending_persistence_pending == 0
            && metrics.first_byte_persistence_pending == 0
            && metrics.lifecycle_enqueue_in_flight == 0
            && metrics.enqueue_retry_pending == 0
            && metrics.worker_record_concurrency_in_flight == 0
            // Admission remains held during inter-dispatcher handoffs even
            // when a dispatcher-specific pending counter has reached zero.
            && self.lifecycle_submission.state.admission.available_permits()
                == self.lifecycle_submission.state.capacity
            // Delayed lifecycle items can live outside any queue counter.
            && self.lifecycle_delay.sender.as_ref().is_none_or(|sender| {
                self.lifecycle_delay.admission.available_permits() == sender.max_capacity()
            })
    }
}
