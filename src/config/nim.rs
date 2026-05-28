use serde::{Deserialize, Serialize};

use crate::config::constants::ANTHROPIC_DEFAULT_MAX_OUTPUT_TOKENS;

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct NimSettings {
    pub temperature: f64,
    pub top_p: f64,
    pub top_k: i32,
    pub max_tokens: i32,
    pub presence_penalty: f64,
    pub frequency_penalty: f64,
    pub min_p: f64,
    pub repetition_penalty: f64,
    pub seed: Option<i64>,
    pub stop: Option<String>,
    pub parallel_tool_calls: bool,
    pub ignore_eos: bool,
    pub min_tokens: i32,
    pub chat_template: Option<String>,
    pub request_id: Option<String>,
}

impl Default for NimSettings {
    fn default() -> Self {
        Self {
            temperature: 1.0,
            top_p: 1.0,
            top_k: -1,
            max_tokens: ANTHROPIC_DEFAULT_MAX_OUTPUT_TOKENS,
            presence_penalty: 0.0,
            frequency_penalty: 0.0,
            min_p: 0.0,
            repetition_penalty: 1.0,
            seed: None,
            stop: None,
            parallel_tool_calls: true,
            ignore_eos: false,
            min_tokens: 0,
            chat_template: None,
            request_id: None,
        }
    }
}
