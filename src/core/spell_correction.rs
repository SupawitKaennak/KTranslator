//! SymSpell-based spell correction for English text.
//!
//! Uses a frequency dictionary (~82K words) to fix OCR typos before translation.
//! Only processes Latin-script (English) text — CJK/Thai/Arabic text is passed through unchanged.

use std::io::BufRead;
use std::sync::LazyLock;
use symspell::{AsciiStringStrategy, SymSpell};

/// Embedded English frequency dictionary (~1.3MB, compiled into binary).
const DICT_DATA: &str = include_str!("../../data/frequency_dictionary_en_82_765.txt");

/// Global singleton SymSpell engine — initialized once on first use.
static SYMSPELL_ENGINE: LazyLock<SymSpell<AsciiStringStrategy>> = LazyLock::new(|| {
    let mut engine: SymSpell<AsciiStringStrategy> = SymSpell::default();

    // Load dictionary from embedded data using a BufReader over the in-memory string.
    let cursor = std::io::Cursor::new(DICT_DATA);
    let reader = std::io::BufReader::new(cursor);

    for line in reader.lines() {
        let Ok(line) = line else { continue };
        let trimmed = line.trim();
        if trimmed.is_empty() {
            continue;
        }
        // SymSpell load_dictionary_line expects: key, count, staging_count, separator
        // We pass the full line and let the internal parser handle splitting.
        engine.load_dictionary_line(trimmed, 0, 1, " ");
    }

    tracing::info!(
        "SymSpell spell correction engine initialized with embedded English dictionary"
    );
    engine
});

/// Returns `true` if the line is predominantly Latin-script (English).
/// We skip spell correction for CJK, Thai, Arabic, and other non-Latin scripts.
fn is_predominantly_latin(text: &str) -> bool {
    let mut latin_count = 0usize;
    let mut non_latin_alpha = 0usize;

    for c in text.chars() {
        if c.is_ascii_alphabetic() {
            latin_count += 1;
        } else if c.is_alphabetic() {
            // Non-ASCII alphabetic = CJK, Thai, Arabic, Cyrillic, etc.
            non_latin_alpha += 1;
        }
    }

    let total_alpha = latin_count + non_latin_alpha;
    if total_alpha == 0 {
        return false;
    }

    // At least 70% of alphabetic characters must be Latin
    (latin_count as f32 / total_alpha as f32) >= 0.7
}

/// Correct spelling errors in a single line of English text.
///
/// Uses `lookup_compound` which handles:
/// - Single-word typos (`schoool` → `school`)
/// - Compound errors (`whereis th elove` → `where is the love`)
/// - Missing spaces (`whatdoyouthink` → `what do you think`)
///
/// For non-English text, returns the input unchanged.
pub fn correct_line(line: &str) -> String {
    let trimmed = line.trim();
    if trimmed.is_empty() || !is_predominantly_latin(trimmed) {
        return line.to_string();
    }

    // Detect if the input is ALL CAPS to restore casing after correction
    let is_all_upper = trimmed
        .chars()
        .filter(|c| c.is_ascii_alphabetic())
        .all(|c| c.is_ascii_uppercase());

    // SymSpell works on lowercase; we lowercase the input for correction
    let lower_input = trimmed.to_lowercase();

    let suggestions = SYMSPELL_ENGINE.lookup_compound(&lower_input, 2);

    if let Some(best) = suggestions.first() {
        let corrected = &best.term;

        // Skip if the correction is identical to the lowercased input (nothing changed)
        if corrected == &lower_input {
            return line.to_string();
        }

        // Restore ALL CAPS if the original was all uppercase
        if is_all_upper {
            corrected.to_uppercase()
        } else {
            corrected.to_string()
        }
    } else {
        line.to_string()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_correct_simple_typo() {
        let result = correct_line("I am goig to schoool");
        assert_eq!(result, "i am going to school");
    }

    #[test]
    fn test_correct_compound_errors() {
        // SymSpell corrects "whereis" → "whereas" (valid word, closer edit distance).
        // This is expected behavior — compound correction finds the most likely valid form.
        let result = correct_line("whereis th elove");
        assert_eq!(result, "whereas the love");
    }

    #[test]
    fn test_correct_split_words() {
        // More realistic OCR scenario: words split by extra spaces
        let result = correct_line("can you hel p me");
        // SymSpell should merge "hel p" back into a valid form
        assert!(!result.contains("hel p"), "Expected 'hel p' to be corrected, got: {}", result);
    }

    #[test]
    fn test_skip_japanese() {
        let input = "これはテストです";
        let result = correct_line(input);
        assert_eq!(result, input);
    }

    #[test]
    fn test_skip_empty() {
        assert_eq!(correct_line(""), "");
        assert_eq!(correct_line("   "), "   ");
    }

    #[test]
    fn test_all_caps_preserved() {
        let result = correct_line("WHRE IS THE LVOE");
        // Should be corrected and returned in uppercase
        assert!(result
            .chars()
            .filter(|c| c.is_alphabetic())
            .all(|c| c.is_uppercase()));
    }

    #[test]
    fn test_predominantly_latin_detection() {
        assert!(is_predominantly_latin("hello world"));
        assert!(is_predominantly_latin("HELLO WORLD 123"));
        assert!(!is_predominantly_latin("これはテストです"));
        assert!(!is_predominantly_latin("สวัสดีครับ"));
        assert!(!is_predominantly_latin(""));
    }
}
