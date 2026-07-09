use anyhow::Result;
use reqwest::blocking::Client;
use serde::{Deserialize, Serialize};

use crate::core::{ports::Translator, types::LanguageTag};

use super::llm_shared_utilities;

#[derive(Clone)]
pub struct AzureOpenAiTranslator {
    client: Client,
    endpoint: String,
    api_key: String,
    behavior: Option<crate::infrastructure::settings::TranslationBehaviorSettings>,
}

impl AzureOpenAiTranslator {
    pub fn new(
        base_url: String,
        api_key: String,
        deployment_name: String,
        api_version: String,
        behavior: Option<crate::infrastructure::settings::TranslationBehaviorSettings>,
    ) -> Result<Self> {
        let client =
            llm_shared_utilities::build_client(llm_shared_utilities::DEFAULT_TIMEOUT_SECS)?;
        
        let base_url = base_url.trim_end_matches('/');
        // URL format: {base_url}/openai/deployments/{deployment_name}/chat/completions?api-version={api_version}
        let endpoint = format!("{}/openai/deployments/{}/chat/completions?api-version={}", base_url, deployment_name, api_version);

        Ok(Self {
            client,
            endpoint,
            api_key,
            behavior,
        })
    }
}

impl Translator for AzureOpenAiTranslator {
    fn translate(
        &self,
        text: &str,
        source: Option<&LanguageTag>,
        target: &LanguageTag,
        context_hint: Option<&str>,
    ) -> anyhow::Result<String> {
        if self.endpoint.is_empty() || self.api_key.is_empty() {
            return Err(anyhow::anyhow!(
                "Azure OpenAI configuration is incomplete".to_string(),
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

        let req_body = AzureOpenAiRequest {
            messages: vec![
                AzureOpenAiMessage {
                    role: "system".to_string(),
                    content: prompt.system,
                },
                AzureOpenAiMessage {
                    role: "user".to_string(),
                    content: prompt.user,
                },
            ],
            temperature: temp,
            max_tokens,
        };

        let res = self.client.post(&self.endpoint)
            .header("api-key", self.api_key.trim())
            .json(&req_body).send().map_err(|e| {
            anyhow::anyhow!(format!(
                "Azure OpenAI request failed: {:?}",
                e
            ))
        })?;

        let status = res.status();
        let body_text = res.text().unwrap_or_default();

        if !status.is_success() {
            return Err(anyhow::anyhow!(format!(
                "Azure OpenAI API error {}: {}",
                status, body_text
            )));
        }

        let resp: AzureOpenAiResponse = serde_json::from_str(&body_text).map_err(|e| {
            anyhow::anyhow!(format!(
                "Failed to parse Azure OpenAI API response: {}, response was: {}",
                e, body_text
            ))
        })?;

        let translated = resp
            .choices
            .first()
            .and_then(|c| c.message.as_ref())
            .map(|m| m.content.trim().to_string())
            .unwrap_or_default();

        Ok(translated)
    }

    fn correct_text(
        &self,
        text: &str,
        _lang_hint: Option<&LanguageTag>,
    ) -> anyhow::Result<String> {
        if self.endpoint.is_empty() || self.api_key.is_empty() {
            return Err(anyhow::anyhow!(
                "Azure OpenAI configuration is incomplete".to_string(),
            ));
        }

        let system = "You are an OCR error correction engine. Fix typos and garbled text in the following input. Return ONLY the corrected text. Do NOT translate it. Preserve the original formatting.";
        let max_tokens = llm_shared_utilities::estimate_max_tokens(text);

        let req_body = AzureOpenAiRequest {
            messages: vec![
                AzureOpenAiMessage {
                    role: "system".to_string(),
                    content: system.to_string(),
                },
                AzureOpenAiMessage {
                    role: "user".to_string(),
                    content: text.to_string(),
                },
            ],
            temperature: 0.1,
            max_tokens,
        };

        let res = self.client.post(&self.endpoint)
            .header("api-key", self.api_key.trim())
            .json(&req_body).send().map_err(|e| {
            anyhow::anyhow!(format!(
                "Azure OpenAI request failed during OCR correction: {:?}",
                e
            ))
        })?;

        let status = res.status();
        let body_text = res.text().unwrap_or_default();

        if !status.is_success() {
            return Err(anyhow::anyhow!(format!(
                "Azure OpenAI API error during OCR correction {}: {}",
                status, body_text
            )));
        }

        let resp: AzureOpenAiResponse = serde_json::from_str(&body_text).map_err(|e| {
            anyhow::anyhow!(format!(
                "Failed to parse Azure OpenAI API response: {}, response was: {}",
                e, body_text
            ))
        })?;

        let corrected = resp
            .choices
            .first()
            .and_then(|c| c.message.as_ref())
            .map(|m| m.content.trim().to_string())
            .unwrap_or_default();

        Ok(corrected)
    }
}

#[derive(Debug, Serialize)]
struct AzureOpenAiRequest {
    messages: Vec<AzureOpenAiMessage>,
    temperature: f32,
    max_tokens: u32,
}

#[derive(Debug, Serialize, Deserialize)]
struct AzureOpenAiMessage {
    role: String,
    content: String,
}

#[derive(Debug, Deserialize)]
struct AzureOpenAiResponse {
    choices: Vec<AzureOpenAiChoice>,
}

#[derive(Debug, Deserialize)]
struct AzureOpenAiChoice {
    message: Option<AzureOpenAiMessage>,
}
