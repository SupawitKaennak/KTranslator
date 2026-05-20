#[cfg(target_os = "windows")]
use windows::core::HSTRING;
#[cfg(target_os = "windows")]
use windows::Data::Text::WordsSegmenter;

/// Uses Windows.Globalization.WordsSegmenter to break Thai text into words.
/// This ensures proper wrapping in the UI for Thai language which doesn't use spaces.
pub fn segment_thai(
    text: &str,
    mode: crate::infrastructure::settings::ThaiSegmentationMode,
) -> String {
    use crate::infrastructure::settings::ThaiSegmentationMode;

    if !text
        .chars()
        .any(|c| (c as u32) >= 0x0E01 && (c as u32) <= 0x0E5B)
    {
        return text.to_string();
    }

    match mode {
        ThaiSegmentationMode::Standard => {
            if text.contains(' ') {
                return text.to_string();
            }
        }
        ThaiSegmentationMode::SyllableLevel => {
            return syllable_segment_thai(text);
        }
        ThaiSegmentationMode::DictionaryAssisted => {}
    }

    #[cfg(target_os = "windows")]
    {
        let segmenter = WordsSegmenter::CreateWithLanguage(&HSTRING::from("th-TH"));
        if let Ok(segmenter) = segmenter {
            let tokens = segmenter.GetTokens(&HSTRING::from(text));
            if let Ok(tokens) = tokens {
                let mut result = String::with_capacity(text.len() + 10);
                for token in tokens {
                    if let Ok(word_text) = token.Text() {
                        result.push_str(&word_text.to_string());
                        result.push(' ');
                    }
                }
                return result.trim().to_string();
            }
        }
    }
    text.to_string()
}

/// Syllable-level breaks using Thai combining-mark clusters (no dictionary).
fn syllable_segment_thai(text: &str) -> String {
    let mut out = String::with_capacity(text.len() * 2);
    let mut prev_base = false;
    for c in text.chars() {
        let u = c as u32;
        let is_mark = (0x0E30..=0x0E3A).contains(&u)
            || (0x0E40..=0x0E44).contains(&u)
            || (0x0E47..=0x0E4E).contains(&u);
        let is_base = (0x0E01..=0x0E2E).contains(&u);
        if is_base && prev_base {
            out.push('\u{200B}');
        }
        out.push(c);
        prev_base = is_base && !is_mark;
    }
    out
}
