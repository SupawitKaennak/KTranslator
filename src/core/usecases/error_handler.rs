use parking_lot::Mutex;
use std::collections::BTreeMap;
use std::sync::Arc;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
#[allow(dead_code)]
pub enum ErrorSeverity {
    Warning,
    Error,
    Critical,
}

#[derive(Debug, Clone)]
pub struct AppError {
    #[allow(dead_code)]
    pub id: usize,
    #[allow(dead_code)]
    pub severity: ErrorSeverity,
    pub message: String,
}

/// Centralized mechanism responsible for collecting, cataloging,
/// and dispatching application errors to the user interface.
#[derive(Clone)]
pub struct ErrorHandler {
    errors: Arc<Mutex<BTreeMap<usize, AppError>>>,
    next_id: Arc<Mutex<usize>>,
}

impl ErrorHandler {
    pub fn new() -> Self {
        Self {
            errors: Arc::new(Mutex::new(BTreeMap::new())),
            next_id: Arc::new(Mutex::new(1)),
        }
    }

    /// Logs a new error with standard severity and auto-incrementing ID.
    pub fn report(&self, severity: ErrorSeverity, message: impl Into<String>) -> usize {
        let mut next = self.next_id.lock();
        let id = *next;
        *next += 1;

        let msg = message.into();

        // Structured logging integration
        match severity {
            ErrorSeverity::Warning => tracing::warn!("[Error #{id}] {msg}"),
            ErrorSeverity::Error => tracing::error!("[Error #{id}] {msg}"),
            ErrorSeverity::Critical => tracing::error!("[CRITICAL #{id}] {msg}"),
        }

        let err = AppError {
            id,
            severity,
            message: msg,
        };

        self.errors.lock().insert(id, err);
        id
    }

    /// Quickly logs a standard error.
    pub fn report_simple(&self, message: impl Into<String>) -> usize {
        self.report(ErrorSeverity::Error, message)
    }

    /// Dismisses a specific error by ID.
    pub fn dismiss(&self, id: usize) {
        self.errors.lock().remove(&id);
    }

    /// Clears all cataloged errors.
    pub fn clear_all(&self) {
        self.errors.lock().clear();
    }

    /// Retrieves a snapshot of all current active errors.
    pub fn get_all_errors(&self) -> Vec<AppError> {
        self.errors.lock().values().cloned().collect()
    }

    /// Checks if any errors are actively unaddressed.
    pub fn has_errors(&self) -> bool {
        !self.errors.lock().is_empty()
    }
}
