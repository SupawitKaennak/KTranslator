use std::collections::HashMap;
use crate::infrastructure::settings::{GlossaryEntry, GlossaryType};

pub struct GlossaryEngine;

impl GlossaryEngine {
    /// Checks if input text perfectly matches any stored TranslationMemory entry.
    pub fn apply_translation_memory(text: &str, entries: &[GlossaryEntry]) -> Option<String> {
        let trimmed = text.trim();
        for entry in entries {
            if entry.enabled && entry.entry_type == GlossaryType::TranslationMemory {
                if entry.source.trim().eq_ignore_ascii_case(trimmed) {
                    return Some(entry.target.clone());
                }
            }
        }
        None
    }

    /// Filters glossary entries that appear in the text, sorted by priority (descending).
    pub fn filter_active_entries(text: &str, entries: &[GlossaryEntry]) -> Vec<GlossaryEntry> {
        let mut active = Vec::new();
        let lower_text = text.to_lowercase();

        for entry in entries {
            if !entry.enabled || entry.source.trim().is_empty() {
                continue;
            }

            // TranslationMemory and Overrides are handled separately
            if entry.entry_type == GlossaryType::TranslationMemory || entry.entry_type == GlossaryType::PhraseOverride || entry.entry_type == GlossaryType::ProtectedWord {
                continue;
            }

            if lower_text.contains(&entry.source.to_lowercase()) {
                active.push(entry.clone());
            }
        }

        active.sort_by(|a, b| b.priority.cmp(&a.priority));
        active
    }

    /// Formats active glossary rules into clear instructions for injection into LLM System prompts.
    pub fn build_glossary_guidance(active_entries: &[GlossaryEntry]) -> String {
        if active_entries.is_empty() {
            return String::new();
        }

        let mut lines = Vec::new();
        for entry in active_entries {
            let label = match entry.entry_type {
                GlossaryType::CharacterName => "Character Name",
                GlossaryType::GameTerminology => "Game Terminology",
                GlossaryType::SlangJargon => "Slang / Jargon",
                _ => "Custom Term",
            };
            lines.push(format!("- \"{}\" -> \"{}\" ({})", entry.source, entry.target, label));
        }

        lines.join("\n")
    }

    /// Applies immediate hard string replacements for PhraseOverride and masks ProtectedWords.
    pub fn apply_pre_override(text: &str, entries: &[GlossaryEntry]) -> (String, HashMap<String, String>) {
        let mut current_text = text.to_string();
        let mut protected_map = HashMap::new();
        let mut g_counter = 0;

        // Apply PhraseOverride directly
        for entry in entries {
            if entry.enabled && entry.entry_type == GlossaryType::PhraseOverride && !entry.source.is_empty() {
                // Case-insensitive or sensitive replacement; simple native replace
                current_text = current_text.replace(&entry.source, &entry.target);
            }
        }

        // Mask ProtectedWords to string placeholders e.g. [[GPROT_X]]
        for entry in entries {
            if entry.enabled && entry.entry_type == GlossaryType::ProtectedWord && !entry.source.is_empty() {
                let mut temp = String::new();
                let mut start = 0;
                let target_lower = entry.source.to_lowercase();
                let text_lower = current_text.to_lowercase();

                while let Some(pos) = text_lower[start..].find(&target_lower) {
                    let actual_start = start + pos;
                    let actual_end = actual_start + entry.source.len();
                    temp.push_str(&current_text[start..actual_start]);

                    let placeholder = format!("[[GPROT_{}]]", g_counter);
                    // Use target as override if specified, otherwise keep original
                    let override_val = if !entry.target.is_empty() { entry.target.clone() } else { current_text[actual_start..actual_end].to_string() };
                    protected_map.insert(placeholder.clone(), override_val);
                    temp.push_str(&placeholder);

                    g_counter += 1;
                    start = actual_end;
                }
                temp.push_str(&current_text[start..]);
                current_text = temp;
            }
        }

        (current_text, protected_map)
    }

    /// Decodes ProtectedWord placeholders back into their mapped target strings.
    pub fn apply_post_override(text: &str, protected_map: &HashMap<String, String>) -> String {
        let mut current_text = text.to_string();
        for (placeholder, original) in protected_map {
            current_text = current_text.replace(placeholder, original);
        }
        current_text
    }
}
