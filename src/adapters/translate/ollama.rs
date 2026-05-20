use anyhow::{bail, Context, Result};
use reqwest::blocking::Client;
use serde::{Deserialize, Serialize};

use crate::core::{ports::Translator, types::LanguageTag};

use super::llm_common;

#[derive(Clone)]
pub struct OllamaTranslator {
    client: Client,
    url: String, // e.g. "http://localhost:11434"
    model: String,
    behavior: Option<crate::infrastructure::settings::TranslationBehaviorSettings>,
}

impl OllamaTranslator {
    pub fn new(
        url: String,
        model: String,
        behavior: Option<crate::infrastructure::settings::TranslationBehaviorSettings>,
    ) -> Result<Self> {
        // Ollama uses a longer timeout (60s) since local models can be slow on first load
        let client = llm_common::build_client(60)?;
        Ok(Self {
            client,
            url: url.trim_end_matches('/').to_string(),
            model,
            behavior,
        })
    }

    pub fn list_models(url: &str) -> Result<Vec<String>> {
        let client = Client::builder()
            .timeout(std::time::Duration::from_secs(10))
            .build()
            .context("build http client")?;

        let endpoint = format!("{}/api/tags", url.trim_end_matches('/'));
        let resp = client
            .get(&endpoint)
            .send()
            .context("send ollama listModels request")?;

        if !resp.status().is_success() {
            let status = resp.status();
            let body = resp.text().unwrap_or_default();
            bail!("Ollama listModels error: {status} {body}");
        }

        let data: serde_json::Value = resp.json().context("parse ollama models response")?;
        let mut out = Vec::new();
        if let Some(list) = data.get("models").and_then(|v| v.as_array()) {
            for item in list {
                if let Some(name) = item.get("name").and_then(|v| v.as_str()) {
                    out.push(name.to_string());
                }
            }
        }
        out.sort();
        Ok(out)
    }
}

impl Translator for OllamaTranslator {
    fn translate(
        &self,
        text: &str,
        source: Option<&LanguageTag>,
        target: &LanguageTag,
        context_hint: Option<&str>,
    ) -> Result<String, crate::core::error::KError> {
        let prompt =
            llm_common::build_prompt(text, source, target, self.behavior.as_ref(), context_hint);
        let temp = llm_common::get_temperature(self.behavior.as_ref(), 0.1);

        self.call_ollama(&prompt.system, &prompt.user, temp)
    }
}

impl OllamaTranslator {
    fn call_ollama(
        &self,
        system_prompt: &str,
        user_prompt: &str,
        temp: f32,
    ) -> Result<String, crate::core::error::KError> {
        let req = OllamaChatRequest {
            model: self.model.clone(),
            messages: vec![
                OllamaMessage {
                    role: "system".to_string(),
                    content: system_prompt.to_string(),
                },
                OllamaMessage {
                    role: "user".to_string(),
                    content: user_prompt.to_string(),
                },
            ],
            stream: false,
            options: Some(OllamaOptions {
                temperature: temp,
                num_predict: -1,
                repeat_penalty: 1.2,   // Penalty for repeating the same words
                presence_penalty: 0.6, // Penalty for repeating topics/lines
            }),
        };

        let endpoint = format!("{}/api/chat", self.url);
        let resp = self.client.post(&endpoint).json(&req).send().map_err(|e| {
            crate::core::error::KError::Translation(format!("send ollama request: {:?}", e))
        })?;

        if !resp.status().is_success() {
            let status = resp.status();
            let body = resp.text().unwrap_or_default();
            return Err(crate::core::error::KError::Translation(format!("Ollama error: {status} {body} (Make sure Ollama is running and model '{}' is pulled)", self.model)));
        }

        let data: OllamaChatResponse = resp.json().map_err(|e| {
            crate::core::error::KError::Translation(format!("parse ollama response: {:?}", e))
        })?;
        Ok(data.message.content.trim().to_string())
    }
}

#[derive(Serialize)]
struct OllamaChatRequest {
    model: String,
    messages: Vec<OllamaMessage>,
    stream: bool,
    #[serde(skip_serializing_if = "Option::is_none")]
    options: Option<OllamaOptions>,
}

#[derive(Serialize, Deserialize)]
struct OllamaMessage {
    role: String,
    content: String,
}

#[derive(Serialize)]
struct OllamaOptions {
    temperature: f32,
    num_predict: i32,
    repeat_penalty: f32,
    presence_penalty: f32,
}

#[derive(Deserialize)]
struct OllamaChatResponse {
    message: OllamaMessage,
}
