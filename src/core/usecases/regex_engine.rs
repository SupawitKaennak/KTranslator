use std::collections::HashMap;
use regex::Regex;
use parking_lot::Mutex;
use crate::infrastructure::settings::{RegexRule, RegexRuleType};

/// Thread-safe cache for compiled regex patterns.
/// Avoids recompiling the same pattern string on every OCR frame (~20-60fps).
static REGEX_CACHE: Mutex<Option<HashMap<String, Regex>>> = Mutex::new(None);

fn get_compiled(pattern: &str) -> Option<Regex> {
    let mut guard = REGEX_CACHE.lock();
    let cache = guard.get_or_insert_with(HashMap::new);
    if let Some(re) = cache.get(pattern) {
        return Some(re.clone());
    }
    match Regex::new(pattern) {
        Ok(re) => {
            cache.insert(pattern.to_string(), re.clone());
            Some(re)
        }
        Err(e) => {
            tracing::warn!("Invalid regex pattern '{}': {}", pattern, e);
            None
        }
    }
}

pub struct RegexEngine;

impl RegexEngine {
    /// Applies PreTranslation, Protected, Ignore, Replace, and Split rules.
    /// Returns the transformed text along with a map of protected string placeholders.
    pub fn apply_pre_rules(text: &str, rules: &[RegexRule]) -> (String, HashMap<String, String>) {
        let mut current_text = text.to_string();
        let mut protected_map = HashMap::new();
        let mut p_counter = 0;

        for rule in rules {
            if !rule.enabled || rule.pattern.trim().is_empty() {
                continue;
            }

            if let Some(re) = get_compiled(&rule.pattern) {
                match rule.rule_type {
                    RegexRuleType::Ignore => {
                        // Strip matched patterns entirely
                        current_text = re.replace_all(&current_text, "").to_string();
                    }
                    RegexRuleType::Replace | RegexRuleType::PreTranslation => {
                        // General pre-translation replacement
                        current_text = re.replace_all(&current_text, &rule.replacement).to_string();
                    }
                    RegexRuleType::Split => {
                        // Replace matches with newlines
                        current_text = re.replace_all(&current_text, "\n").to_string();
                    }
                    RegexRuleType::Protected => {
                        // Extract matched sub-strings, encode as [[PROT_X]], save original to decode post-translation
                        let mut temp = String::new();
                        let mut last_end = 0;
                        for mat in re.find_iter(&current_text.clone()) {
                            temp.push_str(&current_text[last_end..mat.start()]);
                            let placeholder = format!("[[PROT_{}]]", p_counter);
                            protected_map.insert(placeholder.clone(), mat.as_str().to_string());
                            temp.push_str(&placeholder);
                            p_counter += 1;
                            last_end = mat.end();
                        }
                        temp.push_str(&current_text[last_end..]);
                        current_text = temp;
                    }
                    RegexRuleType::PostTranslation => {
                        // Handled in apply_post_rules
                    }
                }
            }
        }

        (current_text, protected_map)
    }

    /// Applies PostTranslation rules and decodes Protected placeholders.
    pub fn apply_post_rules(text: &str, rules: &[RegexRule], protected_map: &HashMap<String, String>) -> String {
        let mut current_text = text.to_string();

        // 1. First apply user post-translation regex rules
        for rule in rules {
            if !rule.enabled || rule.pattern.trim().is_empty() {
                continue;
            }

            if rule.rule_type == RegexRuleType::PostTranslation {
                if let Some(re) = get_compiled(&rule.pattern) {
                    current_text = re.replace_all(&current_text, &rule.replacement).to_string();
                }
            }
        }

        // 2. Decode Protected string placeholders back to their pristine original form
        for (placeholder, original) in protected_map {
            current_text = current_text.replace(placeholder, original);
        }

        current_text
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::infrastructure::settings::{RegexRule, RegexRuleType};

    fn rule(rule_type: RegexRuleType, pattern: &str, replacement: &str) -> RegexRule {
        RegexRule {
            enabled: true,
            rule_type,
            pattern: pattern.to_string(),
            replacement: replacement.to_string(),
        }
    }

    #[test]
    fn ignore_rule_strips_matches() {
        let rules = vec![rule(RegexRuleType::Ignore, r"\d+", "")];
        let (result, _) = RegexEngine::apply_pre_rules("abc123def456", &rules);
        assert_eq!(result, "abcdef");
    }

    #[test]
    fn replace_rule_substitutes() {
        let rules = vec![rule(RegexRuleType::Replace, r"hello", "world")];
        let (result, _) = RegexEngine::apply_pre_rules("hello there", &rules);
        assert_eq!(result, "world there");
    }

    #[test]
    fn split_rule_inserts_newlines() {
        let rules = vec![rule(RegexRuleType::Split, r"\|", "")];
        let (result, _) = RegexEngine::apply_pre_rules("A|B|C", &rules);
        assert_eq!(result, "A\nB\nC");
    }

    #[test]
    fn protected_rule_creates_placeholders() {
        let rules = vec![rule(RegexRuleType::Protected, r"\b[A-Z]+\b", "")];
        let (result, protected) = RegexEngine::apply_pre_rules("Hello WORLD there", &rules);
        assert!(result.contains("[[PROT_0]]"));
        assert_eq!(protected.get("[[PROT_0]]").unwrap(), "WORLD");
    }

    #[test]
    fn post_rules_decode_protected() {
        let mut protected = std::collections::HashMap::new();
        protected.insert("[[PROT_0]]".to_string(), "ORIGINAL".to_string());
        let result = RegexEngine::apply_post_rules("translated [[PROT_0]] text", &[], &protected);
        assert_eq!(result, "translated ORIGINAL text");
    }

    #[test]
    fn post_translation_rule_applies_after() {
        let rules = vec![rule(RegexRuleType::PostTranslation, r"foo", "bar")];
        let result = RegexEngine::apply_post_rules("foo baz foo", &rules, &std::collections::HashMap::new());
        assert_eq!(result, "bar baz bar");
    }

    #[test]
    fn disabled_rule_skipped() {
        let mut r = rule(RegexRuleType::Ignore, r"\d+", "");
        r.enabled = false;
        let (result, _) = RegexEngine::apply_pre_rules("abc123", &[r]);
        assert_eq!(result, "abc123");
    }

    #[test]
    fn empty_pattern_skipped() {
        let r = rule(RegexRuleType::Ignore, "   ", "");
        let (result, _) = RegexEngine::apply_pre_rules("abc123", &[r]);
        assert_eq!(result, "abc123");
    }
}
