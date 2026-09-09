use std::time::Duration;

use aether_data_contracts::DataLayerError;

use crate::AppState;

impl AppState {
    /// Call after request producers have stopped, while the usage queue worker
    /// is still running. A false result means the deadline expired, not that
    /// queued records were discarded or safely persisted.
    pub async fn wait_for_usage_idle(&self, timeout: Duration) -> Result<bool, DataLayerError> {
        self.usage_runtime
            .wait_for_idle(self.data.as_ref(), timeout)
            .await
    }
}
