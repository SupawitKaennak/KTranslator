use anyhow::{bail, Context, Result};
use reqwest::blocking::Client;
use serde::{Deserialize, Serialize};

use crate::core::{
    ports::Translator,
    prompt_builder,
    types::LanguageTag,
};

#[derive(Clone)]
pub struct GroqTranslator {
    client: Client,
    api_key: String,
    model: String,
    behavior: Option<crate::infrastructure::settings::TranslationBehaviorSettings>,
}

impl GroqTranslator {
    pub fn new(
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
        Ok(Self {
            client,
            api_key,
            model,
            behavior,
        })
    }

    pub fn list_models(api_key: &str) -> Result<Vec<String>> {
        if api_key.trim().is_empty() {
            bail!("Groq API key is empty");
        }
        let client = Client::builder()
            .timeout(std::time::Duration::from_secs(30))
            .build()
            .context("build http client")?;

        let resp = client
            .get("https://api.groq.com/openai/v1/models")
            .header("Authorization", format!("Bearer {}", api_key))
            .send()
            .context("send groq listModels request")?;

        if !resp.status().is_success() {
            let status = resp.status();
            let body = resp.text().unwrap_or_default();
            bail!("Groq listModels error: {status} {body}");
        }

        let data: serde_json::Value = resp.json().context("parse groq models response")?;
        let mut out = Vec::new();
        if let Some(list) = data.get("data").and_then(|v| v.as_array()) {
            for item in list {
                if let Some(id) = item.get("id").and_then(|v| v.as_str()) {
                    out.push(id.to_string());
                }
            }
        }
        out.sort();
        Ok(out)
    }
}

impl Translator for GroqTranslator {
    fn translate(
        &self,
        text: &str,
        source: Option<&LanguageTag>,
        target: &LanguageTag,
        context_hint: Option<&str>,
    ) -> Result<String> {
        if self.api_key.trim().is_empty() {
            bail!("Groq API key is empty (obtain it from console.groq.com)");
        }

        let lines: Vec<&str> = text.lines().collect();
        let ctx = if self.behavior.as_ref().map(|b| b.contextual_translation).unwrap_or(false) {
            context_hint
        } else {
            None
        };
        let prompt = prompt_builder::build_translation_prompt_with_behavior(
            &lines, source, target, self.behavior.as_ref(), ctx,
        );
        
        let temp = self.behavior.as_ref().map(|b| b.creativity).unwrap_or(0.2);

        // Dynamically calculate budget for output tokens based on actual input length.
        let estimated_tokens = (text.len() as f32 * 2.5).ceil() as u32 + 64;
        let max_tokens = estimated_tokens.clamp(128, 2048);

        let req = GroqChatRequest {
            model: self.model.clone(),
            messages: vec![
                GroqMessage {
                    role: "system".to_string(),
                    content: prompt.system,
                },
                GroqMessage {
                    role: "user".to_string(),
                    content: prompt.user,
                },
            ],
            temperature: temp,
            max_tokens,
        };

        let resp = self.client
            .post("https://api.groq.com/openai/v1/chat/completions")
            .header("Authorization", format!("Bearer {}", self.api_key))
            .json(&req)
            .send()
            .context("send groq request")?;

        if !resp.status().is_success() {
            let status = resp.status();
            let body = resp.text().unwrap_or_default();
            bail!("Groq error: {status} {body}");
        }

        let data: GroqChatResponse = resp.json().context("parse groq response")?;
        let out = data.choices.into_iter().next()
            .map(|c| c.message.content)
            .unwrap_or_default();

        Ok(out.trim().to_string())
    }
}

#[derive(Serialize)]
struct GroqChatRequest {
    model: String,
    messages: Vec<GroqMessage>,
    temperature: f32,
    max_tokens: u32,
}

#[derive(Serialize, Deserialize)]
struct GroqMessage {
    role: String,
    content: String,
}

#[derive(Deserialize)]
struct GroqChatResponse {
    choices: Vec<GroqChoice>,
}

#[derive(Deserialize)]
struct GroqChoice {
    message: GroqMessageResponse,
}

#[derive(Deserialize)]
struct GroqMessageResponse {
    content: String,
}
