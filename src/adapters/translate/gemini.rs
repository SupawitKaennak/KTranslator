use anyhow::{bail, Context, Result};
use reqwest::blocking::Client;
use serde::{Deserialize, Serialize};

use crate::core::{ports::Translator, types::LanguageTag};

use super::llm_shared_utilities;

#[derive(Debug, Clone)]
pub struct GeminiModel {
    pub id: String,           // "gemini-2.0-flash"
    pub display_name: String, // "Gemini 2.0 Flash"
}

#[derive(Clone)]
pub struct GeminiTranslator {
    client: Client,
    api_key: String,
    model: String,
    behavior: Option<crate::infrastructure::settings::TranslationBehaviorSettings>,
}

impl GeminiTranslator {
    pub fn new(
        api_key: String,
        model: String,
        behavior: Option<crate::infrastructure::settings::TranslationBehaviorSettings>,
    ) -> Result<Self> {
        let client = llm_shared_utilities::build_client(llm_shared_utilities::DEFAULT_TIMEOUT_SECS)?;
        Ok(Self {
            client,
            api_key,
            model,
            behavior,
        })
    }

    pub fn list_models(api_key: &str) -> Result<Vec<GeminiModel>> {
        if api_key.trim().is_empty() {
            bail!("Gemini API key is empty");
        }
        let client = Client::builder()
            .timeout(std::time::Duration::from_secs(30))
            .tcp_keepalive(std::time::Duration::from_secs(60))
            .build()
            .context("build http client")?;

        let resp = client
            .get("https://generativelanguage.googleapis.com/v1beta/models")
            .query(&[("key", api_key)])
            .send()
            .context("send listModels request")?;

        if !resp.status().is_success() {
            let status = resp.status();
            let body = resp.text().unwrap_or_default();
            bail!("Gemini listModels error: {status} {body}");
        }

        let data: ListModelsResponse = resp.json().context("parse listModels response")?;
        let mut out = Vec::new();
        for m in data.models {
            let id = m
                .name
                .strip_prefix("models/")
                .unwrap_or(m.name.as_str())
                .to_string();
            let display_name = m.display_name.unwrap_or_else(|| id.clone());
            if m.supported_generation_methods
                .as_ref()
                .map(|xs| xs.iter().any(|x| x == "generateContent"))
                .unwrap_or(true)
            {
                out.push(GeminiModel { id, display_name });
            }
        }
        out.sort_by(|a, b| {
            a.display_name
                .to_lowercase()
                .cmp(&b.display_name.to_lowercase())
        });
        Ok(out)
    }

    fn endpoint(&self) -> String {
        format!(
            "https://generativelanguage.googleapis.com/v1beta/models/{}:generateContent",
            self.model
        )
    }
}

impl Translator for GeminiTranslator {
    fn translate(
        &self,
        text: &str,
        source: Option<&LanguageTag>,
        target: &LanguageTag,
        context_hint: Option<&str>,
    ) -> Result<String, crate::core::error::KError> {
        if self.api_key.trim().is_empty() {
            return Err(crate::core::error::KError::Translation(
                "Gemini API key is empty (open Settings and set it)".to_string(),
            ));
        }

        let prompt =
            llm_shared_utilities::build_prompt(text, source, target, self.behavior.as_ref(), context_hint);
        let temp = llm_shared_utilities::get_temperature(self.behavior.as_ref(), 0.1);
        let max_tokens = llm_shared_utilities::estimate_max_tokens(text);

        let body = RequestBody {
            system_instruction: Some(Content {
                parts: vec![Part {
                    text: prompt.system,
                }],
            }),
            contents: vec![Content {
                parts: vec![Part { text: prompt.user }],
            }],
            generation_config: Some(GenerationConfig {
                temperature: Some(temp), // Dynamically mapped to Translation Creativity Slider
                max_output_tokens: Some(max_tokens),
            }),
        };

        let resp = self
            .client
            .post(self.endpoint())
            .query(&[("key", &self.api_key)])
            .json(&body)
            .send()
            .map_err(|e| {
                crate::core::error::KError::Translation(format!(
                    "send generateContent request: {:?}",
                    e
                ))
            })?;

        if !resp.status().is_success() {
            let status = resp.status();
            let body = resp.text().unwrap_or_default();
            return Err(crate::core::error::KError::Translation(format!(
                "Gemini API error: {status} {body}"
            )));
        }

        let data: ResponseBody = resp.json().map_err(|e| {
            crate::core::error::KError::Translation(format!(
                "parse generateContent response: {:?}",
                e
            ))
        })?;
        let translated = data
            .candidates
            .first()
            .and_then(|c| c.content.parts.first())
            .map(|p| p.text.clone())
            .ok_or_else(|| {
                crate::core::error::KError::Translation(
                    "Gemini returned no candidates (Safety filter?)".to_string(),
                )
            })?;

        Ok(translated)
    }

    fn correct_text(
        &self,
        text: &str,
        _lang_hint: Option<&LanguageTag>,
    ) -> Result<String, crate::core::error::KError> {
        if self.api_key.trim().is_empty() {
            return Err(crate::core::error::KError::Translation(
                "Gemini API key is empty".to_string(),
            ));
        }

        let system = "You are an OCR error correction engine. Fix typos and garbled text in the following input. Return ONLY the corrected text. Do NOT translate it. Preserve the original formatting.";
        let max_tokens = llm_shared_utilities::estimate_max_tokens(text);

        let body = RequestBody {
            system_instruction: Some(Content {
                parts: vec![Part {
                    text: system.to_string(),
                }],
            }),
            contents: vec![Content {
                parts: vec![Part {
                    text: text.to_string(),
                }],
            }],
            generation_config: Some(GenerationConfig {
                temperature: Some(0.1), // Low temperature for high deterministic accuracy
                max_output_tokens: Some(max_tokens),
            }),
        };

        let resp = self
            .client
            .post(self.endpoint())
            .query(&[("key", &self.api_key)])
            .json(&body)
            .send()
            .map_err(|e| {
                crate::core::error::KError::Translation(format!("send generateContent request: {:?}", e))
            })?;

        if !resp.status().is_success() {
            let status = resp.status();
            let body = resp.text().unwrap_or_default();
            return Err(crate::core::error::KError::Translation(format!(
                "Gemini API error during OCR correction: {status} {body}"
            )));
        }

        let data: ResponseBody = resp.json().map_err(|e| {
            crate::core::error::KError::Translation(format!("parse generateContent response: {:?}", e))
        })?;

        let corrected = data
            .candidates
            .first()
            .and_then(|c| c.content.parts.first())
            .map(|p| p.text.trim().to_string())
            .ok_or_else(|| {
                crate::core::error::KError::Translation(
                    "Gemini returned no candidates during OCR correction".to_string(),
                )
            })?;

        Ok(corrected)
    }
}

#[derive(Serialize)]
struct RequestBody {
    #[serde(rename = "systemInstruction", skip_serializing_if = "Option::is_none")]
    system_instruction: Option<Content>,
    contents: Vec<Content>,
    #[serde(rename = "generationConfig")]
    generation_config: Option<GenerationConfig>,
}

#[derive(Serialize, Deserialize)]
struct Content {
    parts: Vec<Part>,
}

#[derive(Serialize, Deserialize)]
struct Part {
    text: String,
}

#[derive(Serialize, Default)]
struct GenerationConfig {
    temperature: Option<f32>,
    #[serde(rename = "maxOutputTokens")]
    max_output_tokens: Option<u32>,
}

#[derive(Deserialize)]
struct ResponseBody {
    candidates: Vec<Candidate>,
}

#[derive(Deserialize)]
struct Candidate {
    content: Content,
}

#[derive(Deserialize)]
struct ListModelsResponse {
    models: Vec<ModelInfo>,
}

#[derive(Deserialize)]
struct ModelInfo {
    name: String,
    #[serde(rename = "displayName")]
    display_name: Option<String>,
    #[serde(rename = "supportedGenerationMethods")]
    supported_generation_methods: Option<Vec<String>>,
}
