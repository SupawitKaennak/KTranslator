use crate::core::ports::Translator;
use crate::core::types::LanguageTag;
use anyhow::Result;
use reqwest::blocking::Client;
use serde_json::Value;

use super::llm_shared_utilities;

pub struct GoogleTranslator {
    client: Client,
}

impl GoogleTranslator {
    pub fn new() -> Result<Self> {
        let client = llm_shared_utilities::build_client(15)?;
        Ok(Self { client })
    }
}

impl Translator for GoogleTranslator {
    fn translate(
        &self,
        text: &str,
        source: Option<&LanguageTag>,
        target: &LanguageTag,
        _context_hint: Option<&str>,
    ) -> anyhow::Result<String> {
        let sl = source.map(|s| s.as_str()).unwrap_or("auto");
        let tl = target.as_str();

        let mut last_err = None;
        for attempt in 0..3 {
            let req = self
                .client
                .get("https://translate.googleapis.com/translate_a/t")
                .header(
                    "User-Agent",
                    "Mozilla/5.0 (Windows NT 10.0; Win64; x64) AppleWebKit/537.36 (KHTML, like Gecko) Chrome/120.0.0.0 Safari/537.36",
                )
                .query(&[
                    ("client", "dict-chrome-ex"),
                    ("sl", sl),
                    ("tl", tl),
                    ("q", text),
                ]);

            match req.send() {
                Ok(resp) => {
                    if resp.status().is_success() {
                        let resp_text = match resp.text() {
                            Ok(t) => t,
                            Err(e) => {
                                last_err = Some(format!("Read text error: {}", e));
                                continue;
                            }
                        };

                        let v: Value = match serde_json::from_str(&resp_text) {
                            Ok(val) => val,
                            Err(e) => {
                                last_err = Some(format!("JSON parse error: {}", e));
                                continue;
                            }
                        };

                        let mut translated = String::new();
                        // dict-chrome-ex format: [["Translated text", "source_lang_code"]]
                        if let Some(outer) = v.as_array() {
                            if let Some(inner) = outer.get(0).and_then(|v| v.as_array()) {
                                if let Some(t) = inner.get(0).and_then(|v| v.as_str()) {
                                    translated = t.to_string();
                                }
                            }
                        }

                        if !translated.is_empty() {
                            return Ok(translated);
                        } else {
                            last_err = Some("Empty translation returned".to_string());
                        }
                    } else if resp.status().as_u16() == 429 {
                        // Rate limit hit, backoff heavily
                        last_err = Some("Rate limit (429)".to_string());
                        std::thread::sleep(std::time::Duration::from_millis(1000 * (attempt + 1)));
                        continue;
                    } else {
                        last_err = Some(format!("HTTP error: {}", resp.status()));
                    }
                }
                Err(e) => {
                    last_err = Some(format!("Request send error: {}", e));
                }
            }

            // Simple backoff for general errors before retrying
            if attempt < 2 {
                std::thread::sleep(std::time::Duration::from_millis(300 * (attempt + 1)));
            }
        }

        Err(anyhow::anyhow!(format!(
            "Google Translate failed after 3 attempts. Last error: {:?}",
            last_err
        )))
    }
}
