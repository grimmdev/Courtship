use serde::{Deserialize, Serialize};
use std::sync::atomic::{AtomicUsize, Ordering};
use std::sync::Arc;

#[derive(Debug, Default, Clone, Serialize, Deserialize)]
pub struct UsageReport {
    pub total_calls: usize,
    pub total_tokens_in: usize,
    pub total_tokens_out: usize,
    pub standing_schema_tokens: usize,
    pub loaded_schema_tokens: usize,
    pub cache_hits: usize,
    pub avg_latency_ms: f64,
}

#[derive(Clone, Default)]
pub struct SessionLedger {
    calls: Arc<AtomicUsize>,
    tokens_in: Arc<AtomicUsize>,
    tokens_out: Arc<AtomicUsize>,
    standing_tokens: Arc<AtomicUsize>,
    loaded_tokens: Arc<AtomicUsize>,
    cache_hits: Arc<AtomicUsize>,
}

impl SessionLedger {
    pub fn new() -> Self {
        Self::default()
    }

    pub fn set_standing_surface_tokens(&self, tokens: usize) {
        self.standing_tokens.store(tokens, Ordering::Relaxed);
    }

    pub fn update_loaded_schema_tokens(&self, tokens: usize) {
        self.loaded_tokens.store(tokens, Ordering::Relaxed);
    }

    pub fn record_interaction(&self, tokens_in: usize, tokens_out: usize) {
        self.calls.fetch_add(1, Ordering::Relaxed);
        self.tokens_in.fetch_add(tokens_in, Ordering::Relaxed);
        self.tokens_out.fetch_add(tokens_out, Ordering::Relaxed);
    }

    pub fn record_cache_hit(&self) {
        self.cache_hits.fetch_add(1, Ordering::Relaxed);
    }

    pub fn report(&self) -> UsageReport {
        let calls = self.calls.load(Ordering::Relaxed);
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
