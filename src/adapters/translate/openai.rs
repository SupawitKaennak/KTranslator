use anyhow::{bail, Context, Result};
use reqwest::blocking::Client;
use serde::{Deserialize, Serialize};

use crate::core::{
    ports::Translator,
    prompt_builder,
    types::LanguageTag,
};

#[derive(Clone)]
pub struct OpenAiTranslator {
    client: Client,
    base_url: String,
    api_key: String,
    model: String,
    behavior: Option<crate::infrastructure::settings::TranslationBehaviorSettings>,
}

impl OpenAiTranslator {
    pub fn new(
        base_url: String, 
        api_key: String, 
        model: String,
        behavior: Option<crate::infrastructure::settings::TranslationBehaviorSettings>,
    ) -> Result<Self> {
        let client = Client::builder()
            .timeout(std::time::Duration::from_secs(30))
            .tcp_keepalive(std::time::Duration::from_secs(60))
            .pool_idle_timeout(std::time::Duration::from_secs(120))
            .build()
            .context("build http client")?;
            
        let base_url = base_url.trim_end_matches('/').to_string();
        
        Ok(Self {
            client,
            base_url,
            api_key,
            model,
            behavior,
        })
    }

    pub fn list_models(base_url: &str, api_key: &str) -> Result<Vec<String>> {
        let client = Client::builder().timeout(std::time::Duration::from_secs(10)).build()?;
        let endpoint = format!("{}/models", base_url.trim_end_matches('/'));
        let mut req = client.get(&endpoint);
        if !api_key.trim().is_empty() {
            req = req.bearer_auth(api_key.trim());
        }
        let resp = req.send()?;
        if resp.status().is_success() {
            #[derive(serde::Deserialize)]
            struct ModelsResp { data: Vec<ModelItem> }
            #[derive(serde::Deserialize)]
            struct ModelItem { id: String }

            let parsed: ModelsResp = serde_json::from_str(&resp.text().unwrap_or_default())?;
            let mut m_list: Vec<String> = parsed.data.into_iter().map(|i| i.id).collect();
            m_list.sort();
            Ok(m_list)
        } else {
            bail!("Failed to list models: {}", resp.status());
        }
    }
}

impl Translator for OpenAiTranslator {
    fn translate(
        &self,
        text: &str,
        source: Option<&LanguageTag>,
        target: &LanguageTag,
    ) -> Result<String> {
        if self.base_url.is_empty() {
            bail!("Custom OpenAI Base URL is empty");
        }

        let lines: Vec<&str> = text.lines().collect();
        let prompt = prompt_builder::build_translation_prompt_with_behavior(&lines, source, target, self.behavior.as_ref());
        
        let temp = self.behavior.as_ref().map(|b| b.creativity).unwrap_or(0.3);

        // Dynamically calculate budget for output tokens based on actual input length.
        let estimated_tokens = (text.len() as f32 * 2.5).ceil() as u32 + 64;
        let max_tokens = estimated_tokens.clamp(128, 2048);

        let req_body = OpenAiRequest {
            model: self.model.clone(),
            messages: vec![
                OpenAiMessage {
                    role: "system".to_string(),
                    content: prompt.system,
                },
                OpenAiMessage {
                    role: "user".to_string(),
                    content: prompt.user,
                },
            ],
            temperature: temp,
            max_tokens,
        };

        let endpoint = format!("{}/chat/completions", self.base_url);
        
        let mut req = self.client.post(&endpoint);
        if !self.api_key.trim().is_empty() {
            req = req.bearer_auth(self.api_key.trim());
        }

        let res = req
            .json(&req_body)
            .send()
            .context("OpenAI compatible request failed")?;

        let status = res.status();
        let body_text = res.text().unwrap_or_default();

        if !status.is_success() {
            bail!("OpenAI API error {}: {}", status, body_text);
        }

        let resp: OpenAiResponse = serde_json::from_str(&body_text)
            .with_context(|| format!("Failed to parse OpenAI API response: {}", body_text))?;

        let translated = resp
            .choices
            .first()
            .and_then(|c| c.message.as_ref())
            .map(|m| m.content.trim().to_string())
            .unwrap_or_default();

        Ok(translated)
    }
}

#[derive(Debug, Serialize)]
struct OpenAiRequest {
    model: String,
    messages: Vec<OpenAiMessage>,
    temperature: f32,
    max_tokens: u32,
}

#[derive(Debug, Serialize, Deserialize)]
struct OpenAiMessage {
    role: String,
    content: String,
}

#[derive(Debug, Deserialize)]
struct OpenAiResponse {
    choices: Vec<OpenAiChoice>,
}

#[derive(Debug, Deserialize)]
struct OpenAiChoice {
    message: Option<OpenAiMessage>,
}
