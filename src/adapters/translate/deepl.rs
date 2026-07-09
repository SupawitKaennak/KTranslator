use anyhow::Result;
use reqwest::blocking::Client;
use serde::{Deserialize, Serialize};

use crate::core::{ports::Translator, types::LanguageTag};

use super::llm_shared_utilities;

#[derive(Clone)]
pub struct DeeplTranslator {
    client: Client,
    api_key: String,
}

impl DeeplTranslator {
    pub fn new(api_key: String) -> Result<Self> {
        let client =
            llm_shared_utilities::build_client(llm_shared_utilities::DEFAULT_TIMEOUT_SECS)?;
        Ok(Self { client, api_key })
    }

    fn get_endpoint(&self) -> &'static str {
        if self.api_key.ends_with(":fx") {
            "https://api-free.deepl.com/v2/translate"
        } else {
            "https://api.deepl.com/v2/translate"
        }
    }
}

impl Translator for DeeplTranslator {
    fn translate(
        &self,
        text: &str,
        _source: Option<&LanguageTag>,
        target: &LanguageTag,
        _context_hint: Option<&str>,
    ) -> anyhow::Result<String> {
        if self.api_key.is_empty() {
            return Err(anyhow::anyhow!(
                "DeepL API key is empty".to_string(),
            ));
        }

        let mut target_lang = target.0.to_uppercase();
        // DeepL requires EN-US or EN-GB for English target
        if target_lang == "EN" {
            target_lang = "EN-US".to_string();
        }

        let req_body = DeeplRequest {
            text: vec![text.to_string()],
            target_lang,
        };

        let res = self
            .client
            .post(self.get_endpoint())
            .header("Authorization", format!("DeepL-Auth-Key {}", self.api_key))
            .json(&req_body)
            .send()
            .map_err(|e| {
                anyhow::anyhow!(format!(
                    "DeepL request failed: {:?}",
                    e
                ))
            })?;

        let status = res.status();
        let body_text = res.text().unwrap_or_default();

        if !status.is_success() {
            return Err(anyhow::anyhow!(format!(
                "DeepL API error {}: {}",
                status, body_text
            )));
        }

        let resp: DeeplResponse = serde_json::from_str(&body_text).map_err(|e| {
            anyhow::anyhow!(format!(
                "Failed to parse DeepL API response: {}, response was: {}",
                e, body_text
            ))
        })?;

        let translated = resp
            .translations
            .first()
            .map(|t| t.text.trim().to_string())
            .unwrap_or_default();

        Ok(translated)
    }

    fn correct_text(
        &self,
        text: &str,
        _lang_hint: Option<&LanguageTag>,
    ) -> anyhow::Result<String> {
        // DeepL cannot do OCR correction (it's not an LLM). We just return the original text.
        Ok(text.to_string())
    }
}

#[derive(Debug, Serialize)]
struct DeeplRequest {
    text: Vec<String>,
    target_lang: String,
}

#[derive(Debug, Deserialize)]
struct DeeplResponse {
    translations: Vec<DeeplTranslation>,
}

#[derive(Debug, Deserialize)]
struct DeeplTranslation {
    text: String,
}
