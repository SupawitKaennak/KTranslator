use anyhow::Result;
use reqwest::blocking::Client;
use serde::{Deserialize, Serialize};

use crate::core::{ports::Translator, types::LanguageTag};

use super::llm_shared_utilities;

#[derive(Clone)]
pub struct ClaudeTranslator {
    client: Client,
    api_key: String,
    model: String,
    behavior: Option<crate::infrastructure::settings::TranslationBehaviorSettings>,
}

impl ClaudeTranslator {
    pub fn new(
        api_key: String,
        model: String,
        behavior: Option<crate::infrastructure::settings::TranslationBehaviorSettings>,
    ) -> Result<Self> {
        let client =
            llm_shared_utilities::build_client(llm_shared_utilities::DEFAULT_TIMEOUT_SECS)?;

        Ok(Self {
            client,
            api_key,
            model,
            behavior,
        })
    }
}

impl Translator for ClaudeTranslator {
    fn translate(
        &self,
        text: &str,
        source: Option<&LanguageTag>,
        target: &LanguageTag,
        context_hint: Option<&str>,
    ) -> anyhow::Result<String> {
        if self.api_key.is_empty() {
            return Err(anyhow::anyhow!(
                "Claude API key is empty".to_string(),
            ));
        }

        let prompt = llm_shared_utilities::build_prompt(
            text,
            source,
            target,
            self.behavior.as_ref(),
            context_hint,
        );
        let temp = llm_shared_utilities::get_temperature(self.behavior.as_ref(), 0.3);
        let max_tokens = llm_shared_utilities::estimate_max_tokens(text);

        let req_body = ClaudeRequest {
            model: self.model.clone(),
            max_tokens,
            temperature: temp,
            system: prompt.system,
            messages: vec![ClaudeMessage {
                role: "user".to_string(),
                content: prompt.user,
            }],
        };

        let res = self
            .client
            .post("https://api.anthropic.com/v1/messages")
            .header("x-api-key", &self.api_key)
            .header("anthropic-version", "2023-06-01")
            .json(&req_body)
            .send()
            .map_err(|e| {
                anyhow::anyhow!(format!(
                    "Claude request failed: {:?}",
                    e
                ))
            })?;

        let status = res.status();
        let body_text = res.text().unwrap_or_default();

        if !status.is_success() {
            return Err(anyhow::anyhow!(format!(
                "Claude API error {}: {}",
                status, body_text
            )));
        }

        let resp: ClaudeResponse = serde_json::from_str(&body_text).map_err(|e| {
            anyhow::anyhow!(format!(
                "Failed to parse Claude API response: {}, response was: {}",
                e, body_text
            ))
        })?;

        let translated = resp
            .content
            .first()
            .map(|c| c.text.trim().to_string())
            .unwrap_or_default();

        Ok(translated)
    }

    fn correct_text(
        &self,
        text: &str,
        _lang_hint: Option<&LanguageTag>,
    ) -> anyhow::Result<String> {
        if self.api_key.is_empty() {
            return Err(anyhow::anyhow!(
                "Claude API key is empty".to_string(),
            ));
        }

        let system = "You are an OCR error correction engine. Fix typos and garbled text in the following input. Return ONLY the corrected text. Do NOT translate it. Preserve the original formatting.";
        let max_tokens = llm_shared_utilities::estimate_max_tokens(text);

        let req_body = ClaudeRequest {
            model: self.model.clone(),
            max_tokens,
            temperature: 0.1,
            system: system.to_string(),
            messages: vec![ClaudeMessage {
                role: "user".to_string(),
                content: text.to_string(),
            }],
        };

        let res = self
            .client
            .post("https://api.anthropic.com/v1/messages")
            .header("x-api-key", &self.api_key)
            .header("anthropic-version", "2023-06-01")
            .json(&req_body)
            .send()
            .map_err(|e| {
                anyhow::anyhow!(format!(
                    "Claude request failed during OCR correction: {:?}",
                    e
                ))
            })?;

        let status = res.status();
        let body_text = res.text().unwrap_or_default();

        if !status.is_success() {
            return Err(anyhow::anyhow!(format!(
                "Claude API error during OCR correction {}: {}",
                status, body_text
            )));
        }

        let resp: ClaudeResponse = serde_json::from_str(&body_text).map_err(|e| {
            anyhow::anyhow!(format!(
                "Failed to parse Claude API response: {}, response was: {}",
                e, body_text
            ))
        })?;

        let corrected = resp
            .content
            .first()
            .map(|c| c.text.trim().to_string())
            .unwrap_or_default();

        Ok(corrected)
    }
}

#[derive(Debug, Serialize)]
struct ClaudeRequest {
    model: String,
    max_tokens: u32,
    temperature: f32,
    system: String,
    messages: Vec<ClaudeMessage>,
}

#[derive(Debug, Serialize, Deserialize)]
struct ClaudeMessage {
    role: String,
    content: String,
}

#[derive(Debug, Deserialize)]
struct ClaudeResponse {
    content: Vec<ClaudeContent>,
}

#[derive(Debug, Deserialize)]
struct ClaudeContent {
    text: String,
}
