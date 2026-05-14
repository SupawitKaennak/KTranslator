use unicode_normalization::UnicodeNormalization;

use crate::infrastructure::settings::TextProcessingSettings;

pub struct TextCleaner;

impl TextCleaner {
    /// Line-level filter applied directly to raw OCR results to discard backgrounds or dust recognized as letters.
    pub fn is_line_valid(line: &str, config: &TextProcessingSettings) -> bool {
        let trimmed = line.trim();
        if trimmed.is_empty() {
            return false;
        }

        // Allow standalone important question/exclamation marks regardless of length limit
        let is_standalone_symbol = trimmed == "?" || trimmed == "!" || trimmed == "？" || trimmed == "！";
        
        if !is_standalone_symbol && trimmed.chars().count() < config.min_text_length {
            return false;
        }

        if config.remove_garbage {
            let total_chars = trimmed.chars().count() as f32;
            let special_chars = trimmed.chars().filter(|c| !c.is_alphanumeric()).count() as f32;
            if total_chars > 0.0 && (special_chars / total_chars) > config.special_char_ratio_limit {
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

        // 1. Unicode Normalization (NFKC)
        let mut normalized: String = text.nfkc().collect();

        // Punctuation Normalization
        if config.punctuation_normalization {
            normalized = normalized.replace(",,", ",")
                                   .replace("..", ".")
                                   .replace("。。", "。")
                                   .replace("！！", "！")
                                   .replace("？？", "？");
        }

        // Process each line individually, PRESERVING line count for bounding box coordinates.
        let lines: Vec<String> = normalized
            .lines()
            .map(|l| Self::process_single_line(l.trim(), config))
            .collect();

        lines.join("\n")
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
        // e.g., "ARTHURIA" → "ARTHUR I A", "TOUYA" → "TO U YA", "MILDA" → "MILD A"
        // Merge single uppercase letters back into the previous word when both are uppercase.
        if config.enable_wordninja {
            let tokens: Vec<&str> = s.split_whitespace().collect();
            if tokens.len() > 1 {
                let mut merged: Vec<String> = Vec::new();
                for token in &tokens {
                    let is_short_upper = token.len() <= 2
                        && token.chars().all(|c| c.is_ascii_uppercase());

                    if is_short_upper {
                        if let Some(prev) = merged.last_mut() {
                            // Merge into previous if previous is also uppercase
                            let prev_is_upper = prev.len() >= 2
                                && prev.chars().all(|c| c.is_ascii_uppercase() || !c.is_alphabetic());
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
                // Split any token ≥7 chars that is mostly English letters using dictionary lookup.
                // Wordninja's dictionary is lowercase-only, so we normalize before splitting.
                let alpha_count = token.chars().filter(|c| c.is_ascii_alphabetic()).count();
                if token.len() >= 7 && alpha_count >= 5 {
                    let is_all_upper = token.chars().filter(|c| c.is_ascii_alphabetic()).all(|c| c.is_ascii_uppercase());
                    let lower = token.to_lowercase();
                    let parts = wordninja::DEFAULT_MODEL.split(&lower);
                    // Only accept if the dictionary split it AND all parts are ≥2 chars
                    // (reject splits that produce single-letter fragments like "a", "i")
                    let all_parts_valid = parts.len() > 1
                        && parts.iter().all(|p| p.len() >= 2);
                    if all_parts_valid {
                        for p in parts {
                            if is_all_upper {
                                segmented_words.push(p.to_uppercase());
                            } else {
                                segmented_words.push(p.to_string());
                            }
                        }
                    } else {
                        segmented_words.push(token.to_string());
                    }
                } else {
                    segmented_words.push(token.to_string());
                }
            }
            s = segmented_words.join(" ");
        }

        s
    } 

    fn collapse_repeated_chars(s: &str) -> String {
        if s.len() < 2 { return s.to_string(); }
        let chars: Vec<char> = s.chars().collect();
        let mut result = String::with_capacity(s.len());
        
        let mut i = 0;
        while i < chars.len() {
            let c = chars[i];
            let mut count = 1;
            while i + count < chars.len() && chars[i + count] == c {
                count += 1;
            }

            let limit = if c == '.' || c == '!' || c == '?' || c == '。' || c == '！' || c == '？' || c == '…' {
                3
            } else if c.is_alphanumeric() {
                if count >= 3 { 1 } else { count } 
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
        if s.len() < 4 { return s.to_string(); }
        
        let result = s.to_string();
        let chars: Vec<char> = result.chars().collect();
        let len = chars.len();
        
        for win_size in 2..=(len / 2) {
            let chunk1 = &chars[0..win_size];
            let chunk2 = &chars[win_size..win_size*2];
            
            if chunk1 == chunk2 {
                let mut matches = 2;
                while (matches + 1) * win_size <= len {
                    let next_chunk = &chars[matches * win_size .. (matches + 1) * win_size];
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
                let next = words[i+1];
                let c_clean = current.trim_end_matches('-');
                if current.ends_with('-') && !c_clean.is_empty() && next.to_lowercase().starts_with(&c_clean.to_lowercase()) {
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

    #[test]
    fn test_char_collapse() {
        let cfg = TextProcessingSettings::default();
        assert_eq!(TextCleaner::clean("AAAAABBB", &cfg), "AB");
        assert_eq!(TextCleaner::clean("Hellooooo", &cfg), "Hello");
        assert_eq!(TextCleaner::clean("Wait!!!!!!", &cfg), "Wait!!!");
    }

    #[test]
    fn test_cycle_collapse() {
        let cfg = TextProcessingSettings::default();
        assert_eq!(TextCleaner::clean("ABCABCABC", &cfg), "ABC");
        assert_eq!(TextCleaner::clean("ในที่สุดในที่สุด", &cfg), "ในที่สุด");
    }

    #[test]
    fn test_wordninja_uppercase_split() {
        let mut cfg = TextProcessingSettings::default();
        cfg.enable_wordninja = true;
        cfg.repeated_char_collapse = false;
        cfg.recurring_suppression = false;
        // These are real OCR outputs from manga pages (≥7 chars)
        let result = TextCleaner::process_single_line("IFTHATIS", &cfg);
        assert!(result.contains(" "), "Expected 'IFTHATIS' to be split, got: {}", result);
        let result2 = TextCleaner::process_single_line("THATYOUWOULD", &cfg);
        assert!(result2.contains(" "), "Expected 'THATYOUWOULD' to be split, got: {}", result2);
    }

    #[test]
    fn test_fragment_merger() {
        let mut cfg = TextProcessingSettings::default();
        cfg.enable_wordninja = true;
        cfg.repeated_char_collapse = false;
        cfg.recurring_suppression = false;
        // OCR often splits manga words: "ARTHURIA" → "ARTHUR I A", "MILDA" → "MILD A"
        let result = TextCleaner::process_single_line("ARTHUR I AVON MILD A", &cfg);
        assert!(result.contains("ARTHURI"), "Expected 'ARTHUR I' to merge, got: {}", result);
        assert!(result.contains("MILDA"), "Expected 'MILD A' to merge, got: {}", result);
    }

}
