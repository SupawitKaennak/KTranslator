use anyhow::Result;

use crate::core::ports::Translator;
use crate::core::types::LanguageTag;

/// Compact prior segments for contextual translation (minimal token use).
pub fn build_context_hint(segments: &[String], window: u32) -> Option<String> {
    if window == 0 || segments.is_empty() {
        return None;
    }
    let n = (window as usize).min(segments.len());
    let hint: String = segments
        .iter()
        .rev()
        .take(n)
        .rev()
        .map(|s| {
            let one_line = s.replace('\n', " ");
            if one_line.chars().count() > 72 {
                format!("{}…", one_line.chars().take(72).collect::<String>())
            } else {
                one_line
            }
        })
        .collect::<Vec<_>>()
        .join(" | ");
    Some(hint)
}

/// Runs translation with optional per-line mode when batching is disabled.
pub fn translate_text(
    translator: &dyn Translator,
    text: &str,
    source: Option<&LanguageTag>,
    target: &LanguageTag,
    enable_batching: bool,
    context_hint: Option<&str>,
) -> Result<String> {
    let non_empty: Vec<&str> = text.lines().filter(|l| !l.trim().is_empty()).collect();
    if enable_batching || non_empty.len() <= 1 {
        return translator.translate(text, source, target, context_hint);
    }

    let mut out = Vec::with_capacity(non_empty.len());
    for line in non_empty {
        out.push(translator.translate(line.trim(), source, target, None)?);
    }
    Ok(out.join("\n"))
}
