use crate::infrastructure::settings::Settings;
use parking_lot::Mutex;
use std::sync::Arc;

/// Controller managing transient state for the Settings UI.
/// Encapsulates API models fetching status and active configuration drafts.
#[derive(Clone)]
pub struct SettingsController {
    pub settings_edit: Option<Arc<Mutex<Settings>>>,
    pub gemini_models: Arc<Mutex<Vec<String>>>,
    pub gemini_fetching: Arc<Mutex<bool>>,
    pub groq_models: Arc<Mutex<Vec<String>>>,
    pub groq_fetching: Arc<Mutex<bool>>,
    pub ollama_models: Arc<Mutex<Vec<String>>>,
    pub ollama_fetching: Arc<Mutex<bool>>,
    pub custom_openai_models: Arc<Mutex<Vec<String>>>,
    pub custom_openai_fetching: Arc<Mutex<bool>>,
    #[allow(dead_code)]
    pub custom_openai_error: Arc<Mutex<Option<String>>>,
}

impl SettingsController {
    pub fn new() -> Self {
        Self {
            settings_edit: None,
            gemini_models: Arc::new(Mutex::new(Vec::new())),
            gemini_fetching: Arc::new(Mutex::new(false)),
            groq_models: Arc::new(Mutex::new(Vec::new())),
            groq_fetching: Arc::new(Mutex::new(false)),
            ollama_models: Arc::new(Mutex::new(Vec::new())),
            ollama_fetching: Arc::new(Mutex::new(false)),
            custom_openai_models: Arc::new(Mutex::new(Vec::new())),
            custom_openai_fetching: Arc::new(Mutex::new(false)),
            custom_openai_error: Arc::new(Mutex::new(None)),
        }
    }

    /// Initializes or retrieves the current editing session draft.
    pub fn begin_edit(&mut self, current: &Settings) -> Arc<Mutex<Settings>> {
        if self.settings_edit.is_none() {
            self.settings_edit = Some(Arc::new(Mutex::new(current.clone())));
        }
        self.settings_edit.as_ref().unwrap().clone()
    }

    /// Ends the editing session.
    pub fn end_edit(&mut self) {
        self.settings_edit = None;
    }

    /// Clears fetched models so they will reload on next UI display.
    pub fn reset_models_cache(&self) {
        self.gemini_models.lock().clear();
        self.groq_models.lock().clear();
        self.ollama_models.lock().clear();
        self.custom_openai_models.lock().clear();
    }
}
