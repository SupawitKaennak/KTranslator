use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub enum RegexRuleType {
    PreTranslation,
    PostTranslation,
    Protected,
    Ignore,
    Replace,
    Split,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct RegexRule {
    pub enabled: bool,
    pub pattern: String,
    pub replacement: String,
    pub rule_type: RegexRuleType,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub enum GlossaryType {
    CharacterName,
    GameTerminology,
    ProtectedWord,
    PhraseOverride,
    SlangJargon,
    TranslationMemory,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct GlossaryEntry {
    pub enabled: bool,
    pub source: String,
    pub target: String,
    pub entry_type: GlossaryType,
    pub priority: i32, // Higher priority overrides lower ones
}
