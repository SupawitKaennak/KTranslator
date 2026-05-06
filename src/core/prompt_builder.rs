use crate::core::types::LanguageTag;

// ---------------------------------------------------------------------------
// Shared language code → human-readable name mapping
// ---------------------------------------------------------------------------

/// Convert a BCP-47-ish language tag to a human-readable name for AI prompts.
pub fn lang_name(tag: &LanguageTag) -> &str {
    match tag.0.as_str() {
        "th" => "Thai",
        "en" => "English",
        "ja" => "Japanese",
        "zh-Hans" => "Chinese Simplified",
        "zh-Hant" => "Chinese Traditional",
        "ko" => "Korean",
        "vi" => "Vietnamese",
        "id" => "Indonesian",
        "ru" => "Russian",
        "es" => "Spanish",
        "fr" => "French",
        "de" => "German",
        "pt" => "Portuguese",
        "it" => "Italian",
        "ar" => "Arabic",
        "hi" => "Hindi",
        other => other,
    }
}

/// Convert an optional source language tag to a name, defaulting to "auto-detect".
pub fn lang_name_or_auto(tag: Option<&LanguageTag>) -> &str {
    match tag {
        Some(t) => lang_name(t),
        None => "auto-detect",
    }
}

// ---------------------------------------------------------------------------
// Structured prompt builder for line-aligned batch translation
// ---------------------------------------------------------------------------

/// The separator used between lines in the batch translation protocol.
/// Three pipe characters are extremely rare in natural text, making them
/// a reliable delimiter that AI models can reproduce accurately.
pub const LINE_SEPARATOR: &str = "|||";

/// A ready-to-send prompt pair (system + user message).
pub struct TranslationPrompt {
    pub system: String,
    pub user: String,
    /// Number of input lines. 0 means single-line mode (no separator protocol).
    #[allow(dead_code)] // kept for future validation use by callers
    pub line_count: usize,
}

/// Build a translation prompt from OCR text lines.
///
/// - **Single line:** Simple "translate to X" prompt.
/// - **Multi-line:** Uses the `|||` separator protocol for reliable line alignment.
pub fn build_translation_prompt(
    lines: &[&str],
    source: Option<&LanguageTag>,
    target: &LanguageTag,
) -> TranslationPrompt {
    let target_name = lang_name(target);
    let source_name = lang_name_or_auto(source);

    if lines.len() <= 1 {
        // ── Single-line mode ─────────────────────────────────────────────
        let system = format!(
            "You are a professional manga/game translator. \
             Translate the text to {target_name}. \
             Output ONLY the translated text, no explanations, no quotes."
        );
        let user = if source.is_some() {
            format!("Translate from {source_name} to {target_name}:\n\n{}", lines.first().unwrap_or(&""))
        } else {
            format!("Translate to {target_name}:\n\n{}", lines.first().unwrap_or(&""))
        };
        TranslationPrompt { system, user, line_count: lines.len() }
    } else {
        // ── Multi-line batch mode (||| separator protocol) ───────────────
        let joined_input = lines.join(&format!(" {LINE_SEPARATOR} "));

        let system = format!(
            "You are an expert manga/game translator.\n\
             The input contains {count} text segments separated by \"{sep}\".\n\
             Translate EACH segment to {target_name} and output the translations separated by \"{sep}\".\n\
             \n\
             CRITICAL RULES:\n\
             - Output EXACTLY {count} segments, separated by \"{sep}\".\n\
             - Do NOT add line numbers, bullet points, or explanations.\n\
             - Do NOT merge or split segments.\n\
             - If a segment is empty or untranslatable, output it unchanged.\n\
             - Maintain context across segments (they belong to the same scene).\n\
             - Output ONLY in {target_name}.",
            count = lines.len(),
            sep = LINE_SEPARATOR,
            target_name = target_name,
        );

        let user = if source.is_some() {
            format!("Translate from {source_name} to {target_name}:\n\n{joined_input}")
        } else {
            format!("Translate to {target_name}:\n\n{joined_input}")
        };

        TranslationPrompt { system, user, line_count: lines.len() }
    }
}

// ---------------------------------------------------------------------------
// Response parser — multi-strategy alignment
// ---------------------------------------------------------------------------

/// Parse a translation response back into individual lines, aligned to the
/// expected OCR line count. Uses multiple fallback strategies:
///
/// 1. `|||` separator (preferred — matches our prompt protocol)
/// 2. Numbered list (`1. text`, `1) text`, `1: text`, `[1] text`)
/// 3. Plain newline split
/// 4. Best-effort pad/truncate
pub fn parse_translation_response(raw: &str, expected_count: usize) -> Vec<String> {
    if expected_count == 0 {
        return vec![];
    }

    let trimmed = raw.trim();

    // ── Strategy 1: ||| separator ────────────────────────────────────────
    if trimmed.contains(LINE_SEPARATOR) {
        let parts: Vec<String> = trimmed
            .split(LINE_SEPARATOR)
            .map(|s| s.trim().to_string())
            .collect();
        if parts.len() == expected_count {
            return parts;
        }
        // If close enough (off by 1-2), try to align
        if parts.len() >= expected_count {
            return parts[..expected_count].to_vec();
        }
    }

    // ── Strategy 2: Numbered list ────────────────────────────────────────
    let re_numbered = regex::Regex::new(r"^\s*\[?\s*(\d+)\s*\]?\s*[\.)\:\->\s]+\s*(.*)$").unwrap();
    let mut numbered_result = vec![String::new(); expected_count];
    let mut matched_any = false;

    for line in trimmed.lines() {
        let line = line.trim();
        if line.is_empty() { continue; }
        if let Some(caps) = re_numbered.captures(line) {
            if let Ok(num) = caps[1].parse::<usize>() {
                if num > 0 && num <= expected_count {
                    let content = caps[2].trim().to_string();
                    if numbered_result[num - 1].is_empty() || numbered_result[num - 1].len() < content.len() {
                        numbered_result[num - 1] = content;
                        matched_any = true;
                    }
                }
            }
        }
    }

    // Accept numbered result if we matched at least half the expected lines
    if matched_any {
        let filled = numbered_result.iter().filter(|s| !s.is_empty()).count();
        if filled >= (expected_count + 1) / 2 {
            return numbered_result;
        }
    }

    // ── Strategy 3: Plain newline split ──────────────────────────────────
    let lines: Vec<String> = trimmed
        .lines()
        .map(|l| l.trim().to_string())
        .filter(|l| !l.is_empty())
        .collect();

    if lines.len() == expected_count {
        return lines;
    }

    // ── Strategy 4: Best-effort pad/truncate ─────────────────────────────
    let mut result = if lines.len() > expected_count {
        lines[..expected_count].to_vec()
    } else {
        lines
    };

    // Pad with empty strings to reach expected count
    while result.len() < expected_count {
        result.push(String::new());
    }

    result
}

// ---------------------------------------------------------------------------
// Tests
// ---------------------------------------------------------------------------

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_lang_name() {
        assert_eq!(lang_name(&LanguageTag("th".to_string())), "Thai");
        assert_eq!(lang_name(&LanguageTag("ja".to_string())), "Japanese");
        assert_eq!(lang_name(&LanguageTag("unknown".to_string())), "unknown");
    }

    #[test]
    fn test_parse_separator() {
        let raw = "สวัสดี ||| สบายดีไหม ||| ขอบคุณ";
        let result = parse_translation_response(raw, 3);
        assert_eq!(result, vec!["สวัสดี", "สบายดีไหม", "ขอบคุณ"]);
    }

    #[test]
    fn test_parse_numbered() {
        let raw = "1. สวัสดี\n2. สบายดีไหม\n3. ขอบคุณ";
        let result = parse_translation_response(raw, 3);
        assert_eq!(result, vec!["สวัสดี", "สบายดีไหม", "ขอบคุณ"]);
    }

    #[test]
    fn test_parse_newline_fallback() {
        let raw = "Hello\nWorld\nFoo";
        let result = parse_translation_response(raw, 3);
        assert_eq!(result, vec!["Hello", "World", "Foo"]);
    }

    #[test]
    fn test_parse_pad_short() {
        let raw = "Hello\nWorld";
        let result = parse_translation_response(raw, 4);
        assert_eq!(result, vec!["Hello", "World", "", ""]);
    }

    #[test]
    fn test_parse_truncate_long() {
        let raw = "A\nB\nC\nD\nE";
        let result = parse_translation_response(raw, 3);
        assert_eq!(result, vec!["A", "B", "C"]);
    }

    #[test]
    fn test_single_line_prompt() {
        let target = LanguageTag("th".to_string());
        let prompt = build_translation_prompt(&["Hello"], None, &target);
        assert_eq!(prompt.line_count, 1);
        assert!(!prompt.system.contains("|||"));
    }

    #[test]
    fn test_multi_line_prompt() {
        let target = LanguageTag("th".to_string());
        let prompt = build_translation_prompt(&["Hello", "World"], None, &target);
        assert_eq!(prompt.line_count, 2);
        assert!(prompt.system.contains("|||"));
        assert!(prompt.user.contains("|||"));
    }
}
