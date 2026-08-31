use std::fmt;

#[derive(Clone, Default)]
pub struct DataLeaseBackends;

impl fmt::Debug for DataLeaseBackends {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("DataLeaseBackends")
            .field("has_any", &self.has_any())
            .finish()
    }
}

impl DataLeaseBackends {
    pub fn has_any(&self) -> bool {
        false
    }
}
