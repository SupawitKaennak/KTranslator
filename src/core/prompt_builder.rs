use std::sync::LazyLock;
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
        let extra_rules = if target.0 == "th" {
            " IMPORTANT: Add spaces between words to ensure correct word wrapping (e.g. วันนี้ จะ ไป)."
        } else {
            ""
        };
        let system = format!(
            "You are a professional manga/game translator. \
             Translate the text to {target_name}. \
             Maintain professional grammar, correct capitalization, and proper punctuation. \
             Output ONLY the translated text, no explanations, no quotes.{extra_rules}"
        );
        let user = if source.is_some() {
            format!("Translate from {source_name} to {target_name}:\n\n{}", lines.first().unwrap_or(&""))
        } else {
            format!("Translate to {target_name}:\n\n{}", lines.first().unwrap_or(&""))
        };
        TranslationPrompt { system, user, line_count: lines.len() }
    } else {
        // ── Multi-line batch mode (Numbered List protocol) ───────────────
        // Use numbered lines which is much more robust for Llama/OpenAI models.
        let mut joined_input = String::new();
        for (i, line) in lines.iter().enumerate() {
            joined_input.push_str(&format!("{}. {}\n", i + 1, line));
        }

        let mut extra_rules = String::new();
        if target.0 == "th" {
            extra_rules.push_str("             8. IMPORTANT: Add spaces between words in Thai to allow proper line wrapping (e.g., 'วัน นี้ ผม ไป ตลาด').\n");
        }

        let system = format!(
            "You are an expert professional manga/game translator.\n\
             Input: A numbered list of {count} text segments.\n\
             Task: Translate EACH segment to {target_name}.\n\
             \n\
             STRICT RULES:\n\
             1. Output EXACTLY {count} translated lines.\n\
             2. Use the same numbering format (1. Translation).\n\
             3. Do NOT merge segments or summarize the content.\n\
             4. Do NOT add any notes, explanations, or meta-talk.\n\
             5. If a segment is empty, output the number and an empty translation (e.g. \"5. \").\n\
             6. Prevent hallucinations: translate ONLY what is written.\n\
             7. Maintain professional grammar, correct capitalization, and punctuation.\n\
             8. Output ONLY the numbered list in {target_name}.\n\
{extra_rules}",
            count = lines.len(),
            target_name = target_name,
            extra_rules = extra_rules,
        );

        let user = if source.is_some() {
            format!("Translate these {count} segments from {source_name} to {target_name}:\n\n{joined_input}", count = lines.len())
        } else {
            format!("Translate these {count} segments to {target_name}:\n\n{joined_input}", count = lines.len())
        };

        TranslationPrompt { system, user, line_count: lines.len() }
    }
}

pub fn build_translation_prompt_with_behavior(
    lines: &[&str],
    source: Option<&LanguageTag>,
    target: &LanguageTag,
    behavior: Option<&crate::infrastructure::settings::TranslationBehaviorSettings>,
) -> TranslationPrompt {
    let mut base = build_translation_prompt(lines, source, target);
    
    if let Some(beh) = behavior {
        // Apply Prompt Customization Overrides if enabled
        if beh.custom_prompts.enabled {
            let target_name = lang_name(target);
            let source_name = lang_name_or_auto(source);
            
            base.system = beh.custom_prompts.system_prompt
                .replace("{source_lang}", source_name)
                .replace("{target_lang}", target_name);
                
            if lines.len() <= 1 {
                let txt = lines.first().unwrap_or(&"");
                base.user = beh.custom_prompts.single_line_user_prompt
                    .replace("{source_lang}", source_name)
                    .replace("{target_lang}", target_name)
                    .replace("{text}", txt);
            } else {
                let mut joined_input = String::new();
                for (i, line) in lines.iter().enumerate() {
                    joined_input.push_str(&format!("{}. {}\n", i + 1, line));
                }
                base.user = beh.custom_prompts.multi_line_user_prompt
                    .replace("{source_lang}", source_name)
                    .replace("{target_lang}", target_name)
                    .replace("{count}", &lines.len().to_string())
                    .replace("{numbered_lines}", &joined_input);
            }
        }

        let mut custom_guidance = String::new();
        
        // 1. Literal vs Natural slider
        if beh.literal_natural_slider < 0.35 {
            custom_guidance.push_str(" - Focus on highly literal accuracy, maintaining source sentence structure and idioms directly.\n");
        } else if beh.literal_natural_slider > 0.65 {
            custom_guidance.push_str(" - Focus on highly natural localization, seamlessly rewriting idioms for professional native flow.\n");
        } else {
            custom_guidance.push_str(" - Balance literal semantic fidelity with smooth, natural readability.\n");
        }
        
        // 2. Preservations
        if beh.preserve_honorifics {
            custom_guidance.push_str(" - STRONGLY PRESERVE character honorifics (e.g. -san, -sama, senpai, sensei) as-is in the translated text.\n");
        }
        if beh.preserve_emojis {
            custom_guidance.push_str(" - Retain all original emojis, kaomojis, and expressive punctuation icons.\n");
        }
        if beh.profanity_filter {
            custom_guidance.push_str(" - STRICT PROFANITY FILTER: Mask or replace offensive language with professional mild expressions.\n");
        }
        
        // 3. Tone
        match beh.tone {
            crate::infrastructure::settings::TranslationTone::Formal => custom_guidance.push_str(" - TONE: Maintain a formal, respectable, and polite voice.\n"),
            crate::infrastructure::settings::TranslationTone::Casual => custom_guidance.push_str(" - TONE: Maintain a highly informal, casual, and lively conversational voice.\n"),
            crate::infrastructure::settings::TranslationTone::Polite => custom_guidance.push_str(" - TONE: Use standard polite forms suitable for respectful public communication.\n"),
            _ => {}
        }
        
        // 4. Presets
        match beh.preset {
            crate::infrastructure::settings::TranslationStylePreset::JrpgMode => custom_guidance.push_str(" - STYLE PRESET: JRPG Mode. Use epic fantasy jargon for skills, clear crisp terms for items, and dramatic dialogue styling.\n"),
            crate::infrastructure::settings::TranslationStylePreset::AnimeSubtitle => custom_guidance.push_str(" - STYLE PRESET: Anime Subtitle Mode. Prioritize punchy, highly dynamic, fast-paced dialog subtitles.\n"),
            crate::infrastructure::settings::TranslationStylePreset::VisualNovel => custom_guidance.push_str(" - STYLE PRESET: Visual Novel Mode. Capture rich subtext, emotional depth, and immersive descriptive text accurately.\n"),
            crate::infrastructure::settings::TranslationStylePreset::StreamerMode => custom_guidance.push_str(" - STYLE PRESET: Streamer Overlay Mode. Keep translations concise, readable at a glance, and strictly safe-for-work.\n"),
            _ => {}
        }
        
        if !custom_guidance.is_empty() {
            base.system.push_str(&format!("\n\nBEHAVIOR & STYLE OVERRIDES:\n{}", custom_guidance));
        }
    }
    
    base
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

    // ── Strategy 2: Numbered list (Fallback) ─────────────────────────────
    // Regex matches "1. text", "**1.** text", "- 1. text", "[1] text", etc.
    static RE_NUMBERED: LazyLock<regex::Regex> = LazyLock::new(|| {
        regex::Regex::new(r"(?m)^[\s\*\-]*[\[\(]?\s*(\d+)\s*[\]\)]?[\s\.\:\->\*]+\s*(.*)$").unwrap()
    });
    
    let mut numbered_result = vec![String::new(); expected_count];
    let mut matched_indices = std::collections::HashSet::new();

    for caps in RE_NUMBERED.captures_iter(trimmed) {
        if let Ok(num) = caps[1].parse::<usize>() {
            if num > 0 && num <= expected_count {
                let content = caps[2].trim().to_string();
                if numbered_result[num - 1].len() < content.len() {
                    numbered_result[num - 1] = content;
                    matched_indices.insert(num - 1);
                }
            }
        }
    }

    if matched_indices.len() >= (expected_count + 1) / 2 {
        return numbered_result;
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

