use std::fmt;

#[derive(Clone, Default)]
pub struct DataTransactionBackends;

impl fmt::Debug for DataTransactionBackends {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("DataTransactionBackends")
            .field("has_any", &self.has_any())
            .finish()
    }
}

impl DataTransactionBackends {
    pub fn has_any(&self) -> bool {
        false
    }
}
