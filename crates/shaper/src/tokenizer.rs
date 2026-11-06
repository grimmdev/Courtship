use std::sync::Arc;
use tiktoken_rs::{cl100k_base, o200k_base, CoreBPE};
use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, Default)]
pub enum TokenizerProvider {
    #[default]
    ClaudeApproximation, // cl100k_base closely mirrors modern frontier token densities
    Gpt4o,               // o200k_base
    Gpt4,                // cl100k_base
}

#[derive(Clone)]
pub struct TokenCounter {
    provider: TokenizerProvider,
    bpe: Arc<CoreBPE>,
}

impl TokenCounter {
    pub fn new(provider: TokenizerProvider) -> Result<Self, anyhow::Error> {
        let bpe = match provider {
            TokenizerProvider::ClaudeApproximation | TokenizerProvider::Gpt4 => cl100k_base()?,
            TokenizerProvider::Gpt4o => o200k_base()?,
        };
        Ok(Self {
            provider,
            bpe: Arc::new(bpe),
        })
    }

    pub fn count_tokens(&self, text: &str) -> usize {
        self.bpe.encode_with_special_tokens(text).len()
    }

    pub fn count_json_tokens(&self, val: &serde_json::Value) -> usize {
        let formatted = serde_json::to_string(val).unwrap_or_default();
        self.count_tokens(&formatted)
    }

    pub fn provider(&self) -> TokenizerProvider {
        self.provider
    }
}

impl Default for TokenCounter {
    fn default() -> Self {
        Self::new(TokenizerProvider::default()).expect("Failed to initialize default BPE tokenizer")
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_token_counter_calibration() {
        let counter = TokenCounter::default();
        let prompt = "Jumping twice doesn't work. Find and fix it.";
        let count = counter.count_tokens(prompt);
        // Prompt is roughly 10-12 tokens on Claude/OpenAI BPE
        assert!(count >= 8 && count <= 14, "Count was {count}");

        let json_data = serde_json::json!({
            "id": "player_1",
            "x": 120.5,
            "y": 450.0,
            "state": "grounded"
        });
        let json_tokens = counter.count_json_tokens(&json_data);
        assert!(json_tokens > 0 && json_tokens < 30);
    }
}
