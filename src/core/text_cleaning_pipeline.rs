use regex::Regex;
use std::sync::LazyLock;
use unicode_normalization::UnicodeNormalization;

use crate::core::chinese_convert::convert_chinese;
use crate::infrastructure::settings::TextProcessingSettings;

static FURIGANA_PAREN_RE: LazyLock<Regex> =
    LazyLock::new(|| Regex::new(r"[（(][\p{Hiragana}\p{Katakana}・ー\s]+[）)]").unwrap());

pub struct TextCleaner;

impl TextCleaner {
    /// Line-level filter applied directly to raw OCR results to discard backgrounds or dust recognized as letters.
    pub fn is_line_valid(line: &str, config: &TextProcessingSettings) -> bool {
        let trimmed = line.trim();
        if trimmed.is_empty() {
            return false;
        }

        // Allow standalone important question/exclamation marks regardless of length limit
        let is_standalone_symbol =
            trimmed == "?" || trimmed == "!" || trimmed == "？" || trimmed == "！";

        if !is_standalone_symbol && trimmed.chars().count() < config.min_text_length {
            return false;
        }

        if config.remove_garbage {
            let total_chars = trimmed.chars().count() as f32;
            let special_chars = trimmed.chars().filter(|c| !c.is_alphanumeric()).count() as f32;
            if total_chars > 0.0 && (special_chars / total_chars) > config.special_char_ratio_limit
            {
                return false;
            }
        }

        if config.consonant_spam_filter {
            // Filter pure repeated consonant strings like "wwwwww" or "zzzz"
            let is_all_w = trimmed.chars().all(|c| c == 'w' || c == 'W');
            let is_all_z = trimmed.chars().all(|c| c == 'z' || c == 'Z');
            if (is_all_w || is_all_z) && trimmed.len() > 2 {
                return false;
            }
        }

        if config.kana_spam_filter {
            // Filter repeating single kana artifacts from screentones e.g. "ののの"
            let first_char = trimmed.chars().next().unwrap_or(' ');
            let is_kana = (first_char as u32) >= 0x3040 && (first_char as u32) <= 0x30FF;
            if is_kana && trimmed.chars().count() > 2 && trimmed.chars().all(|c| c == first_char) {
                return false;
            }
        }

        true
    }

    /// Comprehensive cleaning pipeline incorporating dynamic Setting sensitivity.
    pub fn clean(text: &str, config: &TextProcessingSettings) -> String {
        if text.is_empty() {
            return String::new();
        }

        // 1. Unicode Normalization (NFKC) + optional kana width normalization
        let mut normalized: String = text.nfkc().collect();
        if config.jp_kana_normalization {
            normalized = Self::normalize_kana_width(&normalized);
        }
        if config.jp_remove_furigana {
            normalized = Self::strip_furigana(&normalized);
        }
        if config.cn_conversion != crate::infrastructure::settings::ChineseConversionMode::None {
            normalized = convert_chinese(&normalized, config.cn_conversion);
        }
        if config.th_zero_width_cleanup {
            normalized = Self::strip_zero_width(&normalized);
        }
        if config.ar_rtl_correction {
            normalized = Self::fix_arabic_rtl(&normalized);
        }

        // Punctuation Normalization
        if config.punctuation_normalization {
            normalized = normalized
                .replace(",,", ",")
                .replace("..", ".")
                .replace("。。", "。")
                .replace("！！", "！")
                .replace("？？", "？");
        }

        let mut lines: Vec<String> = normalized.lines().map(|l| l.trim().to_string()).collect();

        if config.remove_duplicates {
            lines = Self::dedupe_consecutive_lines(lines);
        }
        if config.merge_broken_lines {
            lines = Self::merge_broken_lines(lines);
        }
        if config.merge_subtitle_fragments {
            lines = Self::merge_subtitle_fragments(lines);
        }

        let lines: Vec<String> = lines
            .into_iter()
            .map(|l| Self::process_single_line(&l, config))
            .collect();

        lines.join("\n")
    }

    fn normalize_kana_width(s: &str) -> String {
        s.chars()
            .map(|c| {
                let u = c as u32;
                // Half-width katakana → full-width
                if (0xFF61..=0xFF9F).contains(&u) {
                    char::from_u32(u - 0xFF61 + 0x30A1).unwrap_or(c)
                } else {
                    c
                }
            })
            .collect()
    }

    fn strip_furigana(s: &str) -> String {
        FURIGANA_PAREN_RE.replace_all(s, "").into_owned()
    }

    fn strip_zero_width(s: &str) -> String {
        s.chars()
            .filter(|c| {
                !matches!(
                    c,
                    '\u{200B}' | '\u{200C}' | '\u{200D}' | '\u{FEFF}' | '\u{2060}'
                )
            })
            .collect()
    }

    fn is_arabic_char(c: char) -> bool {
        matches!(c as u32, 0x0600..=0x06FF)
    }

    fn fix_arabic_rtl(s: &str) -> String {
        if !s.chars().any(Self::is_arabic_char) {
            return s.to_string();
        }
        // Wrap Arabic runs with RLM to reduce OCR left-to-right ordering artifacts.
        let mut out = String::with_capacity(s.len() + 4);
        let mut in_arabic = false;
        for c in s.chars() {
            let is_ar = Self::is_arabic_char(c);
            if is_ar && !in_arabic {
                out.push('\u{200F}');
                in_arabic = true;
            } else if !is_ar && in_arabic {
                out.push('\u{200F}');
                in_arabic = false;
            }
            out.push(c);
        }
        if in_arabic {
            out.push('\u{200F}');
        }
        out
    }

    fn dedupe_consecutive_lines(lines: Vec<String>) -> Vec<String> {
        let mut out = Vec::with_capacity(lines.len());
        let mut prev: Option<String> = None;
        for line in lines {
            if line.is_empty() {
                out.push(line);
                prev = None;
                continue;
            }
            if prev.as_ref() == Some(&line) {
                continue;
            }
            prev = Some(line.clone());
            out.push(line);
        }
        out
    }

    fn merge_broken_lines(lines: Vec<String>) -> Vec<String> {
        if lines.len() < 2 {
            return lines;
        }
        let mut out = Vec::new();
        let mut buf = String::new();
        for line in lines {
            if line.is_empty() {
                if !buf.is_empty() {
                    out.push(buf.clone());
                    buf.clear();
                }
                out.push(String::new());
                continue;
            }
            if buf.is_empty() {
                buf = line;
                continue;
            }
            let continues = !buf.ends_with(['.', '!', '?', '。', '！', '？', '…', ':', '：'])
                && line
                    .chars()
                    .next()
                    .is_some_and(|c| !c.is_uppercase() || line.len() <= 2);
            if continues {
                let latin_join = buf.chars().last().is_some_and(|c| c.is_ascii_alphabetic())
                    && line.chars().next().is_some_and(|c| c.is_ascii_alphabetic());
                if latin_join && !buf.ends_with('-') {
                    buf.push(' ');
                }
                buf.push_str(&line);
            } else {
                out.push(buf.clone());
                buf = line;
            }
        }
        if !buf.is_empty() {
            out.push(buf);
        }
        out
    }

    fn merge_subtitle_fragments(lines: Vec<String>) -> Vec<String> {
        if lines.len() < 2 {
            return lines;
        }
        let mut out = Vec::new();
        let mut buf = String::new();
        for line in lines {
            if line.is_empty() {
                if !buf.is_empty() {
                    out.push(buf.clone());
                    buf.clear();
                }
                out.push(String::new());
                continue;
            }
            let is_fragment = line.chars().count() <= 4;
            if buf.is_empty() {
                buf = line;
            } else if is_fragment || buf.chars().count() <= 4 {
                buf.push(' ');
                buf.push_str(&line);
            } else {
                out.push(buf.clone());
                buf = line;
            }
        }
        if !buf.is_empty() {
            out.push(buf);
        }
        out
    }

    fn process_single_line(line: &str, config: &TextProcessingSettings) -> String {
        let mut s = line.to_string();

        if config.repeated_char_collapse {
            s = Self::collapse_repeated_chars(&s);
        }

        if config.recurring_suppression {
            s = Self::collapse_repeated_phrases(&s);
        }

        // Stuttering Filter
        s = Self::filter_stuttering(&s);

        // --- OCR Fragment Merger ---
        // Windows OCR on manga fonts often splits words into isolated characters.
        // Merge single uppercase letters back into the previous word when both are uppercase.
        if config.enable_ocr_merge {
            let tokens: Vec<&str> = s.split_whitespace().collect();
            if tokens.len() > 1 {
                let mut merged: Vec<String> = Vec::new();
                for token in &tokens {
                    let is_short_upper =
                        token.len() <= 2 && token.chars().all(|c| c.is_ascii_uppercase());

                    if is_short_upper {
                        if let Some(prev) = merged.last_mut() {
                            // Merge into previous if previous is also uppercase
                            let prev_is_upper = prev.len() >= 2
                                && prev
                                    .chars()
                                    .all(|c| c.is_ascii_uppercase() || !c.is_alphabetic());
                            if prev_is_upper {
                                prev.push_str(token);
                                continue;
                            }
                        }
                    }
                    merged.push(token.to_string());
                }
                s = merged.join(" ");
            }
        }

        // --- Wordninja Dictionary Splitting ---
        if config.enable_wordninja {
            let mut segmented_words = Vec::new();
            for token in s.split_whitespace() {
                let alpha_count = token.chars().filter(|c| c.is_ascii_alphabetic()).count();
                // Lower threshold to 5 chars to catch words like WHATDO (6) or CANYOU (6)
                if token.len() >= 5 && alpha_count >= 4 {
                    let first_alpha = token.find(|c: char| c.is_ascii_alphabetic());
                    let last_alpha = token.rfind(|c: char| c.is_ascii_alphabetic());
                    
                    if let (Some(start), Some(end)) = (first_alpha, last_alpha) {
                        let prefix = &token[..start];
                        let suffix = &token[end + 1..];
                        let core = &token[start..=end];
                        
                        let has_internal_punct = core.chars().any(|c| !c.is_ascii_alphabetic() && c != '\'');
                        
                        if !has_internal_punct {
                            let is_all_upper = core.chars().filter(|c| c.is_ascii_alphabetic()).all(|c| c.is_ascii_uppercase());
                            let lower = core.to_lowercase();
                            let parts = wordninja::DEFAULT_MODEL.split(&lower);
                            
                            // Allow valid 1-letter English words like "a", "i", "o" and common contractions
                            let all_parts_valid = parts.len() > 1 && parts.iter().all(|p| {
                                p.len() >= 2 || matches!(p.as_ref(), "a" | "i" | "o" | "s" | "t" | "m" | "d")
                            });
                            
                            if all_parts_valid {
                                let mut joined = String::new();
                                for p in parts {
                                    if !joined.is_empty() { joined.push(' '); }
                                    if is_all_upper {
                                        joined.push_str(&p.to_uppercase());
                                    } else {
                                        let mut c = p.chars();
                                        if let Some(first) = c.next() {
                                            joined.push(first); // Kept simple, true case restoration is hard
                                            joined.push_str(c.as_str());
                                        }
                                    }
                                }
                                segmented_words.push(format!("{}{}{}", prefix, joined, suffix));
                                continue;
                            }
                        }
                    }
                }
                
                // Fallback for missing spaces after punctuation (e.g., "C.I.D.FOR?")
                let mut fixed_token = String::new();
                let chars: Vec<char> = token.chars().collect();
                for i in 0..chars.len() {
                    fixed_token.push(chars[i]);
                    if i + 1 < chars.len() {
                        let c1 = chars[i];
                        let c2 = chars[i + 1];
                        if (c1 == '.' || c1 == ',' || c1 == '?' || c1 == '!') && c2.is_ascii_alphabetic() {
                            // Check if it's an acronym like C.I.D. (where the next letter is followed by another dot)
                            let is_acronym = c1 == '.' && i + 2 < chars.len() && chars[i + 2] == '.';
                            if !is_acronym {
                                fixed_token.push(' ');
                            }
                        }
                    }
                }
                
                segmented_words.push(fixed_token);
            }
            s = segmented_words.join(" ");
        }

        // --- SymSpell Spell Correction ---
        // Runs after wordninja to fix remaining OCR typos before translation.
        // Only processes Latin-script (English) text.
        if config.enable_spell_correction {
            s = crate::core::spell_correction::correct_line(&s);
        }

        s
    }

    fn collapse_repeated_chars(s: &str) -> String {
        if s.len() < 2 {
            return s.to_string();
        }
        let chars: Vec<char> = s.chars().collect();
        let mut result = String::with_capacity(s.len());

        let mut i = 0;
        while i < chars.len() {
            let c = chars[i];
            let mut count = 1;
            while i + count < chars.len() && chars[i + count] == c {
                count += 1;
            }

            let limit = if c == '.'
                || c == '!'
                || c == '?'
                || c == '。'
                || c == '！'
                || c == '？'
                || c == '…'
            {
                3
            } else if c.is_alphanumeric() {
                if count >= 3 {
                    1
                } else {
                    count
                }
            } else {
                1
            };

            for _ in 0..count.min(limit) {
                result.push(c);
            }
            i += count;
        }
        result
    }

    fn collapse_repeated_phrases(s: &str) -> String {
        if s.len() < 4 {
            return s.to_string();
        }

        let result = s.to_string();
        let chars: Vec<char> = result.chars().collect();
        let len = chars.len();

        for win_size in 2..=(len / 2) {
            let chunk1 = &chars[0..win_size];
            let chunk2 = &chars[win_size..win_size * 2];

            if chunk1 == chunk2 {
                let mut matches = 2;
                while (matches + 1) * win_size <= len {
                    let next_chunk = &chars[matches * win_size..(matches + 1) * win_size];
                    if next_chunk == chunk1 {
                        matches += 1;
                    } else {
                        break;
                    }
                }

                if matches >= 2 && matches * win_size >= len - 1 {
                    return chunk1.iter().collect();
                }
            }
        }

        result
    }

    fn filter_stuttering(s: &str) -> String {
        let words: Vec<&str> = s.split_whitespace().collect();
        let mut result_words = Vec::new();

        let mut i = 0;
        while i < words.len() {
            let current = words[i];
            if i + 1 < words.len() {
                let next = words[i + 1];
                let c_clean = current.trim_end_matches('-');
                if current.ends_with('-')
                    && !c_clean.is_empty()
                    && next.to_lowercase().starts_with(&c_clean.to_lowercase())
                {
                    i += 1;
                    continue;
                }
            }
            result_words.push(current);
            i += 1;
        }

        result_words.join(" ")
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::infrastructure::settings::TextProcessingSettings;

    fn default_config() -> TextProcessingSettings {
        TextProcessingSettings::default()
    }

    // ===== is_line_valid =====

    #[test]
    fn valid_line_passes() {
        let cfg = default_config();
        assert!(TextCleaner::is_line_valid("Hello World", &cfg));
    }

    #[test]
    fn empty_line_rejected() {
        let cfg = default_config();
        assert!(!TextCleaner::is_line_valid("", &cfg));
        assert!(!TextCleaner::is_line_valid("   ", &cfg));
    }

    #[test]
    fn standalone_symbols_pass() {
        let mut cfg = default_config();
        // Disable garbage filter to isolate the standalone symbol length-bypass behavior.
        // The garbage filter would reject "?" (100% special chars) — that's separate logic.
        cfg.remove_garbage = false;
        assert!(TextCleaner::is_line_valid("?", &cfg));
        assert!(TextCleaner::is_line_valid("!", &cfg));
        assert!(TextCleaner::is_line_valid("？", &cfg));
        assert!(TextCleaner::is_line_valid("！", &cfg));
    }

    #[test]
    fn garbage_ratio_filtering() {
        let mut cfg = default_config();
        cfg.remove_garbage = true;
        cfg.special_char_ratio_limit = 0.5;
        // All special chars should fail
        assert!(!TextCleaner::is_line_valid("###!@$%", &cfg));
        // Mostly text should pass
        assert!(TextCleaner::is_line_valid("Hello World!", &cfg));
    }

    #[test]
    fn consonant_spam_filter() {
        let mut cfg = default_config();
        cfg.consonant_spam_filter = true;
        assert!(!TextCleaner::is_line_valid("wwww", &cfg));
        assert!(!TextCleaner::is_line_valid("ZZZZ", &cfg));
        assert!(TextCleaner::is_line_valid("ww", &cfg)); // Too short to be spam
    }

    #[test]
    fn kana_spam_filter() {
        let mut cfg = default_config();
        cfg.kana_spam_filter = true;
        assert!(!TextCleaner::is_line_valid("ののの", &cfg));
        assert!(TextCleaner::is_line_valid("のの", &cfg)); // Too short
        assert!(TextCleaner::is_line_valid("のだ", &cfg)); // Mixed = valid
    }

    // ===== clean =====

    #[test]
    fn clean_empty() {
        let cfg = default_config();
        assert_eq!(TextCleaner::clean("", &cfg), "");
    }

    #[test]
    fn clean_preserves_normal_text() {
        let cfg = default_config();
        let result = TextCleaner::clean("Hello World", &cfg);
        assert_eq!(result, "Hello World");
    }

    // ===== collapse_repeated_chars =====

    #[test]
    fn collapse_repeated_chars_basic() {
        assert_eq!(TextCleaner::collapse_repeated_chars("aaaa"), "a");
        assert_eq!(TextCleaner::collapse_repeated_chars("ab"), "ab");
    }

    #[test]
    fn collapse_preserves_punctuation_up_to_3() {
        assert_eq!(TextCleaner::collapse_repeated_chars("!!!!"), "!!!");
        assert_eq!(TextCleaner::collapse_repeated_chars("..."), "...");
        assert_eq!(TextCleaner::collapse_repeated_chars("......"), "...");
    }

    // ===== collapse_repeated_phrases =====

    #[test]
    fn collapse_repeated_phrases() {
        assert_eq!(TextCleaner::collapse_repeated_phrases("abab"), "ab");
        assert_eq!(TextCleaner::collapse_repeated_phrases("abcabc"), "abc");
    }

    #[test]
    fn collapse_no_repeat() {
        assert_eq!(TextCleaner::collapse_repeated_phrases("hello"), "hello");
    }

    // ===== filter_stuttering =====

    #[test]
    fn filter_stuttering_removes_stutter() {
        assert_eq!(
            TextCleaner::filter_stuttering("I- I love you"),
            "I love you"
        );
    }

    #[test]
    fn filter_stuttering_preserves_non_stutter() {
        assert_eq!(TextCleaner::filter_stuttering("hello world"), "hello world");
    }

    // ===== dedupe_consecutive_lines =====

    #[test]
    fn dedupe_consecutive() {
        let lines = vec!["A".to_string(), "A".to_string(), "B".to_string()];
        let result = TextCleaner::dedupe_consecutive_lines(lines);
        assert_eq!(result, vec!["A", "B"]);
    }

    #[test]
    fn dedupe_preserves_non_consecutive() {
        let lines = vec!["A".to_string(), "B".to_string(), "A".to_string()];
        let result = TextCleaner::dedupe_consecutive_lines(lines);
        assert_eq!(result, vec!["A", "B", "A"]);
    }

    // ===== OCR-merge & Wordninja independent toggles =====

    #[test]
    fn test_ocr_merge_only() {
        let mut cfg = default_config();
        cfg.enable_ocr_merge = true;
        cfg.enable_wordninja = false;

        // "HELLO W O R L D" should merge when enable_ocr_merge is true
        assert_eq!(TextCleaner::clean("HELLO W O R L D", &cfg), "HELLOWORLD");

        // Compound word "gamestart" should NOT split when wordninja is false
        assert_eq!(TextCleaner::clean("gamestart", &cfg), "gamestart");
    }

    #[test]
    fn test_wordninja_only() {
        let mut cfg = default_config();
        cfg.enable_ocr_merge = false;
        cfg.enable_wordninja = true;

        // "HELLO W O R L D" should NOT merge when enable_ocr_merge is false
        assert_eq!(
            TextCleaner::clean("HELLO W O R L D", &cfg),
            "HELLO W O R L D"
        );

        // Compound word "gamestart" should split when wordninja is true
        assert_eq!(TextCleaner::clean("gamestart", &cfg), "game start");
    }
}
