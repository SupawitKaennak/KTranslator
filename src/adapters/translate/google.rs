use anyhow::Result;
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
            .timeout(std::time::Duration::from_secs(15))
            .tcp_keepalive(std::time::Duration::from_secs(60))
            .pool_idle_timeout(std::time::Duration::from_secs(120))
            .build()?;
        Ok(Self { client })
    }
}

impl Translator for GoogleTranslator {
    fn translate(&self, text: &str, source: Option<&LanguageTag>, target: &LanguageTag) -> Result<String> {
        let sl = source.map(|s| s.as_str()).unwrap_or("auto");
        let tl = target.as_str();

        let mut last_err = None;
        for attempt in 0..3 {
            let req = self.client.get("https://translate.googleapis.com/translate_a/single")
                .query(&[
                    ("client", "gtx"),
                    ("sl", sl),
                    ("tl", tl),
                    ("dt", "t"),
                    ("q", text),
                ]);

            match req.send() {
                Ok(resp) => {
                    if resp.status().is_success() {
                        let resp_text = match resp.text() {
                            Ok(t) => t,
                            Err(e) => {
                                last_err = Some(anyhow::anyhow!("Read text error: {}", e));
                                continue;
                            }
                        };

                        let v: Value = match serde_json::from_str(&resp_text) {
                            Ok(val) => val,
                            Err(e) => {
                                last_err = Some(anyhow::anyhow!("JSON parse error: {}", e));
                                continue;
                            }
                        };

                        let mut translated = String::new();
                        if let Some(outer) = v.get(0).and_then(|v| v.as_array()) {
                            for inner in outer {
                                if let Some(t) = inner.get(0).and_then(|v| v.as_str()) {
                                    translated.push_str(t);
                                }
                            }
                        }

                        if !translated.is_empty() {
                            return Ok(translated);
                        } else {
                            last_err = Some(anyhow::anyhow!("Empty translation returned"));
                        }
                    } else if resp.status().as_u16() == 429 {
                        // Rate limit hit, backoff heavily
                        last_err = Some(anyhow::anyhow!("Rate limit (429)"));
                        std::thread::sleep(std::time::Duration::from_millis(1000 * (attempt + 1)));
                        continue;
                    } else {
                        last_err = Some(anyhow::anyhow!("HTTP error: {}", resp.status()));
                    }
                }
                Err(e) => {
                    last_err = Some(anyhow::anyhow!("Request send error: {}", e));
                }
            }

            // Simple backoff for general errors before retrying
            if attempt < 2 {
                std::thread::sleep(std::time::Duration::from_millis(300 * (attempt as u64 + 1)));
            }
        }

        anyhow::bail!("Google Translate failed after 3 attempts. Last error: {:?}", last_err)
    }
}
