use anyhow::{Result, Context};
use crate::core::ports::Translator;
use crate::core::types::LanguageTag;
use reqwest::blocking::Client;
use serde_json::Value;

pub struct GoogleTranslator {
    client: Client,
}

impl GoogleTranslator {
    pub fn new() -> Result<Self> {
        let client = Client::builder()
            .timeout(std::time::Duration::from_secs(10))
            .build()?;
        Ok(Self { client })
    }
}

impl Translator for GoogleTranslator {
    fn translate(&self, text: &str, source: Option<&LanguageTag>, target: &LanguageTag) -> Result<String> {
        let sl = source.map(|s| s.as_str()).unwrap_or("auto");
        let tl = target.as_str();

        let resp = self.client.get("https://translate.googleapis.com/translate_a/single")
            .query(&[
                ("client", "gtx"),
                ("sl", sl),
                ("tl", tl),
                ("dt", "t"),
                ("q", text),
            ])
            .send()
            .context("Failed to send request to Google Translate")?
            .text()
            .context("Failed to read response from Google Translate")?;

        let v: Value = serde_json::from_str(&resp)
            .context("Failed to parse Google Translate response")?;

        // Format is [[[translated, source, ...], ...], ...]
        let mut translated = String::new();
        if let Some(outer) = v.get(0).and_then(|v| v.as_array()) {
            for inner in outer {
                if let Some(t) = inner.get(0).and_then(|v| v.as_str()) {
                    translated.push_str(t);
                }
            }
        }

        if translated.is_empty() {
            anyhow::bail!("Google Translate returned empty result");
        }

        Ok(translated)
    }
}
