use crate::{
    ProviderKeyHealthBucket, SchedulerMinimalCandidateSelectionCandidate, SchedulerPriorityMode,
};

#[derive(Debug, Clone, Copy, PartialEq, Eq, Default, serde::Serialize, serde::Deserialize)]
pub enum SchedulerRankingMode {
    FixedOrder,
    #[default]
    CacheAffinity,
    LoadBalance,
    /// R10: cost-priority mode. Within one requested model, candidates rank by
    /// their per-format rate multiplier ascending (cheapest key first); equal
    /// multipliers fall back to the priority slot, then the seeded hash.
    /// Session affinity still outranks cost in the comparator chain.
    CostBased,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Default)]
pub enum SchedulerTunnelAffinityBucket {
    LocalTunnel = 0,
    #[default]
    Neutral = 1,
    RemoteTunnel = 2,
}

#[derive(Debug, Clone, PartialEq)]
pub struct SchedulerRankableCandidate {
    pub provider_id: String,
    pub endpoint_id: String,
    pub key_id: String,
    pub selected_provider_model_name: String,
    pub provider_priority: i32,
    pub key_internal_priority: i32,
    pub key_global_priority_for_format: Option<i32>,
    pub capability_priority: (u32, u32),
    pub cached_affinity_match: bool,
    pub affinity_hash: Option<u64>,
    pub tunnel_bucket: SchedulerTunnelAffinityBucket,
    pub demote_cross_format: bool,
    pub format_preference: (u8, u8),
    pub health_bucket: Option<ProviderKeyHealthBucket>,
    pub health_score: f64,
    /// P1-4: gateway-local in-flight request count for this key at ranking
    /// time. Only participates when the ranking context enables the signal;
    /// `None` (or equal counts) falls through to later comparators. Lower is
    /// better.
    pub inflight_count: Option<u32>,
    /// P1-5: EWMA latency (milliseconds) observed for this key. Samples below
    /// the minimum are treated as absent so cold keys are not penalized.
    /// Lower is better.
    pub latency_ewma_ms: Option<LatencyEwma>,
    /// R10: this candidate's rate multiplier for the request's api format
    /// (from the key's `rate_multipliers` map). Only participates when the
    /// ranking mode is cost-based. Absent defaults to 1.0 (neutral).
    pub rate_multiplier: f64,
    pub original_index: usize,
}

/// P1-5: latency EWMA snapshot used purely for ranking. The live tracker
/// (gateway side) owns the update math; this is the immutable view handed to
/// the comparator.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct LatencyEwma {
    pub samples: u32,
    pub ewma_ms: f64,
}

impl LatencyEwma {
    /// Minimum samples before the EWMA participates in ranking. Below this
    /// the signal is noise from a cold start, not information.
    pub const MIN_SAMPLES: u32 = 5;
}

impl SchedulerRankableCandidate {
    pub fn from_candidate(
        candidate: &SchedulerMinimalCandidateSelectionCandidate,
        original_index: usize,
    ) -> Self {
        Self {
            provider_id: candidate.provider_id.clone(),
            endpoint_id: candidate.endpoint_id.clone(),
            key_id: candidate.key_id.clone(),
            selected_provider_model_name: candidate.selected_provider_model_name.clone(),
            provider_priority: candidate.provider_priority,
            key_internal_priority: candidate.key_internal_priority,
            key_global_priority_for_format: candidate.key_global_priority_for_format,
            capability_priority: (0, 0),
            cached_affinity_match: false,
            affinity_hash: None,
            tunnel_bucket: SchedulerTunnelAffinityBucket::Neutral,
            demote_cross_format: false,
            format_preference: (0, 0),
            health_bucket: None,
            health_score: 1.0,
            inflight_count: None,
            latency_ewma_ms: None,
            rate_multiplier: 1.0,
            original_index,
        }
    }

    pub fn with_capability_priority(mut self, value: (u32, u32)) -> Self {
        self.capability_priority = value;
        self
    }

    pub fn with_cached_affinity_match(mut self, value: bool) -> Self {
        self.cached_affinity_match = value;
        self
    }

    pub fn with_affinity_hash(mut self, value: Option<u64>) -> Self {
        self.affinity_hash = value;
        self
    }

    pub fn with_tunnel_bucket(mut self, value: SchedulerTunnelAffinityBucket) -> Self {
        self.tunnel_bucket = value;
        self
    }

    pub fn with_format_state(
        mut self,
        demote_cross_format: bool,
        format_preference: (u8, u8),
    ) -> Self {
        self.demote_cross_format = demote_cross_format;
        self.format_preference = format_preference;
        self
    }

    pub fn with_health(mut self, bucket: Option<ProviderKeyHealthBucket>, score: f64) -> Self {
        self.health_bucket = bucket;
        self.health_score = score;
        self
    }

    /// R10: set this candidate's cost-based (成本优先) rate multiplier.
    pub fn with_rate_multiplier(mut self, value: f64) -> Self {
        if value.is_finite() && value > 0.0 {
            self.rate_multiplier = value;
        }
        self
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct SchedulerRankingContext {
    pub priority_mode: SchedulerPriorityMode,
    pub ranking_mode: SchedulerRankingMode,
    pub include_health: bool,
    pub load_balance_seed: u64,
    /// P1-4: when true, the in-flight count participates in ranking (after
    /// health, before the seeded hash). Data lives on the candidate.
    pub include_inflight: bool,
    /// P1-5: when true, latency EWMA participates after in-flight. Collection
    /// can run with ranking disabled (observe-first rollout).
    pub include_latency: bool,
}

impl Default for SchedulerRankingContext {
    fn default() -> Self {
        Self {
            priority_mode: SchedulerPriorityMode::Provider,
            ranking_mode: SchedulerRankingMode::CacheAffinity,
            include_health: false,
            include_inflight: false,
            include_latency: false,
            load_balance_seed: 0,
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq, serde::Serialize)]
pub struct SchedulerRankingOutcome {
    pub original_index: usize,
    pub ranking_index: usize,
    pub priority_mode: SchedulerPriorityMode,
    pub ranking_mode: SchedulerRankingMode,
    pub priority_slot: i32,
    pub promoted_by: Option<&'static str>,
    pub demoted_by: Option<&'static str>,
}
