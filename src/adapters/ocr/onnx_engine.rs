use anyhow::Result;
use ort::session::{Session, builder::SessionBuilder};
use ort::ep::DirectMLExecutionProvider;
use std::path::Path;

/// Generic ONNX engine execution wrapper initializing sessions with hardware acceleration.
pub struct OnnxEngine;

impl OnnxEngine {
    /// Creates an ORT Session configured with DirectML execution provider if available.
    pub fn create_session<P: AsRef<Path>>(model_path: P) -> Result<Session> {
        let session = SessionBuilder::new()
            .map_err(|e| anyhow::anyhow!("ORT Builder Error: {}", e))?
            .with_execution_providers([DirectMLExecutionProvider::default().build()])
            .map_err(|e| anyhow::anyhow!("ORT EP Error: {}", e))?
            .commit_from_file(model_path)
            .map_err(|e| anyhow::anyhow!("ORT Commit Error: {}", e))?;
        Ok(session)
    }
}
