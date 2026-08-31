mod quota;

pub use quota::{ProviderBillingType, ProviderQuotaSnapshot};

pub fn quantize_money(value: f64) -> f64 {
    (value * 100_000_000.0).round() / 100_000_000.0
}
