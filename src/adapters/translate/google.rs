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

/// Extract the translated text from Google Translate's JSON response.
///
/// The `dict-chrome-ex` client on `/translate_a/t` returns **different** JSON shapes
/// depending on whether source language is `auto` or explicit:
///
/// **Format A** – JSON object with `sentences` array (most common, returned for ANY `sl`):
/// ```json
/// {"sentences": [{"trans": "สวัสดี", "orig": "こんにちは"}], "src": "ja"}
/// ```
///
/// **Format B** – Nested array (sometimes returned for `sl=auto` on short text):
/// ```json
/// [["สวัสดี", "こんにちは", ...], ..., "ja"]
/// ```
///
/// Previously only Format B was handled, so selecting an explicit source language
/// (which always produces Format A) caused "Empty translation returned" every time.
fn extract_translation(v: &Value) -> Option<String> {
    // ── Format A: JSON object with a "sentences" key ──────────────────────────
    if let Some(sentences) = v.get("sentences").and_then(|s| s.as_array()) {
        let parts: Vec<&str> = sentences
            .iter()
            .filter_map(|s| s.get("trans").and_then(|t| t.as_str()))
            .collect();
        if !parts.is_empty() {
            return Some(parts.join(""));
        }
    }

    // ── Format B: outer array (gtx / legacy dict-chrome-ex on sl=auto) ────────
    if let Some(outer) = v.as_array() {
        if let Some(first) = outer.first() {
            // Deep form: [[["trans_seg", "orig_seg", ...], ...], ..., "src_lang"]
            if let Some(inner_arr) = first.as_array() {
                // Try collecting all translation segments (each is ["trans", "orig", ...])
                let parts: Vec<&str> = inner_arr
                    .iter()
                    .filter_map(|seg| {
                        seg.as_array()
                            .and_then(|a| a.first())
                            .and_then(|t| t.as_str())
                    })
                    .collect();
                if !parts.is_empty() {
                    return Some(parts.join(""));
                }

                // Shallow form: outer[0] is already ["trans_text", "src_lang"]
                if let Some(t) = inner_arr.first().and_then(|v| v.as_str()) {
                    return Some(t.to_string());
                }
            }

            // Flat string as first element
            if let Some(t) = first.as_str() {
                return Some(t.to_string());
            }
        }
    }

    None
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
                                last_err = Some(format!(
                                    "JSON parse error: {} | raw: {}",
                                    e,
                                    &resp_text[..resp_text.len().min(200)]
                                ));
                                continue;
                            }
                        };

                        match extract_translation(&v) {
                            Some(translated) if !translated.is_empty() => {
                                return Ok(translated);
                            }
                            _ => {
                                tracing::warn!(
                                    sl = sl,
                                    tl = tl,
                                    "Google Translate: could not extract translation. Response: {}",
                                    &resp_text[..resp_text.len().min(300)]
                                );
                                last_err = Some(format!(
                                    "Empty translation returned. Response snippet: {}",
                                    &resp_text[..resp_text.len().min(200)]
                                ));
                            }
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
