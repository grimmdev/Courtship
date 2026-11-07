use std::sync::Arc; // Atomic reference counted pointer for shared BPE state
use tiktoken_rs::{cl100k_base, o200k_base, CoreBPE}; // Byte-pair encoding tokenizers
use serde::{Deserialize, Serialize}; // Serialization support for tokenizer options

/// Target tokenizer vocabulary family used for token estimation.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, Default)]
pub enum TokenizerProvider {
    /// cl100k_base byte pair encoding, closely matching modern frontier model token counts.
    #[default]
    ClaudeApproximation,
    /// o200k_base vocabulary used by GPT-4o.
    Gpt4o,
    /// cl100k_base vocabulary used by standard GPT-4.
    Gpt4,
}

/// Offline byte-pair encoding token counter for exact context tax measurement.
#[derive(Clone)]
pub struct TokenCounter {
    /// Active tokenizer vocabulary type.
    provider: TokenizerProvider,
    /// Shared compiled BPE encoding table.
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
        assert!((8..=14).contains(&count), "Count was {count}");

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
