use serde::{Deserialize, Serialize}; // Serialization support for ledger reports
use std::sync::atomic::{AtomicUsize, Ordering}; // Lock-free atomic counters for metrics
use std::sync::Arc; // Atomic reference counted pointer

/// Summary report of token usage, schema taxes, and call metrics for the current session.
#[derive(Debug, Default, Clone, Serialize, Deserialize)]
pub struct UsageReport {
    /// Total number of RPC tool calls processed.
    pub total_calls: usize,
    /// Inbound request token volume.
    pub total_tokens_in: usize,
    /// Outbound response token volume after shaping.
    pub total_tokens_out: usize,
    /// Standing context tax consumed by permanent meta-tools.
    pub standing_schema_tokens: usize,
    /// Transient context tax consumed by currently loaded tool schemas.
    pub loaded_schema_tokens: usize,
    /// Number of duplicate state frames suppressed to zero tokens by delta tracker.
    pub cache_hits: usize,
    /// Average call latency in milliseconds.
    pub avg_latency_ms: f64,
}

/// Thread-safe session ledger tracking token consumption, call counts, and savings.
/// Uses atomic integers for lock-free updates across concurrent client requests.
#[derive(Clone, Default)]
pub struct SessionLedger {
    /// Atomic counter for total method invocations.
    calls: Arc<AtomicUsize>,
    /// Atomic counter for total inbound prompt tokens.
    tokens_in: Arc<AtomicUsize>,
    /// Atomic counter for total outbound response tokens.
    tokens_out: Arc<AtomicUsize>,
    /// Pinned token size of the standing tool schema surface.
    standing_tokens: Arc<AtomicUsize>,
    /// Current token size of on-demand loaded schemas.
    loaded_tokens: Arc<AtomicUsize>,
    /// Count of delta-suppressed redundant frames.
    cache_hits: Arc<AtomicUsize>,
}

impl SessionLedger {
    /// Creates a new empty session ledger with all counters initialized to zero.
    pub fn new() -> Self {
        Self::default()
    }

    /// Records the standing surface token measurement.
    ///
    /// # Parameters
    /// - `tokens`: Measured token count of the standing meta-tools.
    pub fn set_standing_surface_tokens(&self, tokens: usize) {
        self.standing_tokens.store(tokens, Ordering::Relaxed);
    }

    /// Updates the current token footprint of on-demand loaded tool schemas.
    ///
    /// # Parameters
    /// - `tokens`: Total tokens of all currently hydrated schemas.
    pub fn update_loaded_schema_tokens(&self, tokens: usize) {
        self.loaded_tokens.store(tokens, Ordering::Relaxed);
    }

    /// Records an interaction, incrementing total calls and token counters.
    ///
    /// # Parameters
    /// - `tokens_in`: Number of tokens in the request payload.
    /// - `tokens_out`: Number of tokens in the shaped response payload.
    pub fn record_interaction(&self, tokens_in: usize, tokens_out: usize) {
        self.calls.fetch_add(1, Ordering::Relaxed); // Increment invocation count
        self.tokens_in.fetch_add(tokens_in, Ordering::Relaxed); // Accumulate inbound tokens
        self.tokens_out.fetch_add(tokens_out, Ordering::Relaxed); // Accumulate outbound tokens
    }

    /// Records a delta cache hit when a redundant frame is suppressed to zero tokens.
    pub fn record_cache_hit(&self) {
        self.cache_hits.fetch_add(1, Ordering::Relaxed); // Increment suppression count
    }

    /// Generates a snapshot report of current session usage and savings metrics.
    ///
    /// # Returns
    /// An immutable `UsageReport` struct populated with current atomic values.
    pub fn report(&self) -> UsageReport {
        let calls = self.calls.load(Ordering::Relaxed); // Read current call total
        UsageReport {
            total_calls: calls,
            total_tokens_in: self.tokens_in.load(Ordering::Relaxed),
            total_tokens_out: self.tokens_out.load(Ordering::Relaxed),
            standing_schema_tokens: self.standing_tokens.load(Ordering::Relaxed),
            loaded_schema_tokens: self.loaded_tokens.load(Ordering::Relaxed),
            cache_hits: self.cache_hits.load(Ordering::Relaxed),
            avg_latency_ms: 0.0,
        }
    }
}
