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
}

impl OpenAiTranslator {
    pub fn new(base_url: String, api_key: String, model: String) -> Result<Self> {
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
        })
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
        let prompt = prompt_builder::build_translation_prompt(&lines, source, target);

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
            temperature: 0.3,
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
