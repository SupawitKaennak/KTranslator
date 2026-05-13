use std::collections::HashMap;
use regex::Regex;
use crate::infrastructure::settings::{RegexRule, RegexRuleType};

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

            if let Ok(re) = Regex::new(&rule.pattern) {
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
                if let Ok(re) = Regex::new(&rule.pattern) {
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
