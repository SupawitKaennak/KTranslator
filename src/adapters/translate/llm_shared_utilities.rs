//! Shared utilities for LLM-based translator adapters.
//!
//! All LLM translators (OpenAI, Gemini, Groq, Ollama) share common logic for:
//! - HTTP client construction with consistent timeout/keepalive settings
//! - Translation prompt building from behavior settings
//! - Output token budget estimation
//!
//! This module eliminates ~40 lines of duplication per adapter.

use anyhow::{Context, Result};
use reqwest::blocking::Client;
use std::time::Duration;

use crate::core::{
    llm_prompt_builder::{self, TranslationPrompt},
    types::LanguageTag,
};
use crate::infrastructure::settings::TranslationBehaviorSettings;

/// Default HTTP client timeouts for LLM API calls.
pub const DEFAULT_TIMEOUT_SECS: u64 = 30;
pub const DEFAULT_KEEPALIVE_SECS: u64 = 60;
pub const DEFAULT_POOL_IDLE_SECS: u64 = 120;

/// Token estimation constants.
/// Average bytes-per-token ratio for CJK/mixed text (conservative upper bound).
const BYTES_PER_TOKEN_RATIO: f32 = 2.5;
/// Minimum token budget to avoid truncation on short strings.
const MIN_TOKEN_BUDGET: u32 = 128;
/// Maximum token budget to avoid wasting API quota.
const MAX_TOKEN_BUDGET: u32 = 2048;
/// Extra token padding for formatting/structural overhead.
const TOKEN_PADDING: u32 = 64;

/// Builds a standard `reqwest::blocking::Client` with consistent timeout settings.
///
/// All LLM adapters should use this instead of constructing their own client,
/// ensuring uniform timeout/keepalive behavior across the application.
pub fn build_client(timeout_secs: u64) -> Result<Client> {
    Client::builder()
        .timeout(Duration::from_secs(timeout_secs))
        .tcp_keepalive(Duration::from_secs(DEFAULT_KEEPALIVE_SECS))
        .pool_idle_timeout(Duration::from_secs(DEFAULT_POOL_IDLE_SECS))
        .build()
        .context("build http client")
}

/// Builds the translation prompt from text + behavior settings.
///
/// Handles:
/// - Splitting text into lines
/// - Checking whether contextual translation is enabled
/// - Delegating to `llm_prompt_builder::build_translation_prompt_with_behavior`
pub fn build_prompt(
    text: &str,
    source: Option<&LanguageTag>,
    target: &LanguageTag,
    behavior: Option<&TranslationBehaviorSettings>,
    context_hint: Option<&str>,
) -> TranslationPrompt {
    let lines: Vec<&str> = text.lines().collect();
    let ctx = if behavior.map(|b| b.contextual_translation).unwrap_or(false) {
        context_hint
    } else {
        None
    };
    llm_prompt_builder::build_translation_prompt_with_behavior(&lines, source, target, behavior, ctx)
}

/// Estimates the output token budget based on input text length.
///
/// Uses a conservative bytes-per-token ratio to avoid both:
/// - Truncation (too few tokens for the translation)
/// - Waste (allocating 4096 tokens for a 10-character input)
///
/// Returns a value clamped to [128, 2048].
pub fn estimate_max_tokens(text: &str) -> u32 {
    let estimated = (text.len() as f32 * BYTES_PER_TOKEN_RATIO).ceil() as u32 + TOKEN_PADDING;
    estimated.clamp(MIN_TOKEN_BUDGET, MAX_TOKEN_BUDGET)
}

/// Extracts the creativity/temperature from behavior settings with a default fallback.
pub fn get_temperature(behavior: Option<&TranslationBehaviorSettings>, default: f32) -> f32 {
    behavior.map(|b| b.creativity).unwrap_or(default)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn estimate_tokens_short_text() {
        let tokens = estimate_max_tokens("Hi");
        assert_eq!(tokens, MIN_TOKEN_BUDGET); // Clamped to minimum
    }

    #[test]
    fn estimate_tokens_medium_text() {
        // 100 chars * 2.5 + 64 = 314 → should be within bounds
        let text = "a".repeat(100);
        let tokens = estimate_max_tokens(&text);
        assert!(tokens >= MIN_TOKEN_BUDGET);
        assert!(tokens <= MAX_TOKEN_BUDGET);
        assert_eq!(tokens, 314);
    }

    #[test]
    fn estimate_tokens_very_long_text() {
        let text = "x".repeat(10000);
        let tokens = estimate_max_tokens(&text);
        assert_eq!(tokens, MAX_TOKEN_BUDGET); // Clamped to maximum
    }

    #[test]
    fn get_temperature_with_behavior() {
        let behavior = TranslationBehaviorSettings {
            creativity: 0.7,
            ..Default::default()
        };
        assert!((get_temperature(Some(&behavior), 0.3) - 0.7).abs() < f32::EPSILON);
    }

    #[test]
    fn get_temperature_without_behavior() {
        assert!((get_temperature(None, 0.3) - 0.3).abs() < f32::EPSILON);
    }
}
