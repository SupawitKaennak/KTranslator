use super::{
    gemini::GeminiTranslator, google::GoogleTranslator, groq::GroqTranslator,
    ollama::OllamaTranslator, openai::OpenAiTranslator,
};
use crate::core::ports::Translator;
use crate::infrastructure::settings::{Settings, TranslationProvider};
use std::sync::Arc;

/// Factory unifying the instantiation of text-to-text translation providers.
pub struct TranslatorFactory;

impl TranslatorFactory {
    /// Creates a Translator implementation based on current settings.
    pub fn create(settings: &Settings) -> Option<Arc<dyn Translator + Send + Sync>> {
        match settings.provider {
            TranslationProvider::Google => GoogleTranslator::new()
                .ok()
                .map(|t| Arc::new(t) as Arc<dyn Translator + Send + Sync>),
            TranslationProvider::Gemini => GeminiTranslator::new(
                settings.gemini_api_key.clone(),
                settings.gemini_model.clone(),
                Some(settings.trans_behavior.clone()),
            )
            .ok()
            .map(|t| Arc::new(t) as Arc<dyn Translator + Send + Sync>),
            TranslationProvider::Groq => GroqTranslator::new(
                settings.groq_api_key.clone(),
                settings.groq_model.clone(),
                Some(settings.trans_behavior.clone()),
            )
            .ok()
            .map(|t| Arc::new(t) as Arc<dyn Translator + Send + Sync>),
            TranslationProvider::Ollama => OllamaTranslator::new(
                settings.ollama_url.clone(),
                settings.ollama_model.clone(),
                Some(settings.trans_behavior.clone()),
            )
            .ok()
            .map(|t| Arc::new(t) as Arc<dyn Translator + Send + Sync>),
            TranslationProvider::CustomOpenAI => OpenAiTranslator::new(
                settings.custom_openai_url.clone(),
                settings.custom_openai_api_key.clone(),
                settings.custom_openai_model.clone(),
                Some(settings.trans_behavior.clone()),
            )
            .ok()
            .map(|t| Arc::new(t) as Arc<dyn Translator + Send + Sync>),
        }
    }
}

/// Convenience standalone function mirroring the factory method.
pub fn create_translator(settings: &Settings) -> Option<Arc<dyn Translator + Send + Sync>> {
    TranslatorFactory::create(settings)
}
