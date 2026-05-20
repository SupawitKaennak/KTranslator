use crate::infrastructure::settings::GpuBackend;
use anyhow::Result;
use ort::ep::{CUDAExecutionProvider, DirectMLExecutionProvider, TensorRTExecutionProvider};
use ort::session::{builder::SessionBuilder, Session};
use std::path::Path;

/// Generic ONNX engine execution wrapper initializing sessions with hardware acceleration.
pub struct OnnxEngine;

impl OnnxEngine {
    /// Creates an ORT Session configured with the specified GPU backend.
    /// Respects user's performance settings for GPU acceleration selection.
    pub fn create_session<P: AsRef<Path>>(
        model_path: P,
        gpu_backend: GpuBackend,
    ) -> Result<Session> {
        let builder =
            SessionBuilder::new().map_err(|e| anyhow::anyhow!("ORT Builder Error: {}", e))?;

        // Select execution providers based on user's GPU backend setting
        let execution_providers = match gpu_backend {
            GpuBackend::Auto => vec![
                TensorRTExecutionProvider::default().build(),
                CUDAExecutionProvider::default().build(),
                DirectMLExecutionProvider::default().build(),
            ],
            GpuBackend::Cuda => vec![CUDAExecutionProvider::default().build()],
            GpuBackend::DirectMl => vec![DirectMLExecutionProvider::default().build()],
            GpuBackend::TensorRt => vec![TensorRTExecutionProvider::default().build()],
            GpuBackend::Cpu => vec![],
        };

        let mut builder = builder
            .with_execution_providers(execution_providers)
            .map_err(|e| anyhow::anyhow!("ORT EP Error: {}", e))?;

        // Log which GPU backend is being used for verification
        tracing::info!(
            "Creating ONNX session for {:?} with GPU backend: {:?}",
            model_path.as_ref(),
            gpu_backend
        );

        let session = builder
            .commit_from_file(model_path)
            .map_err(|e| anyhow::anyhow!("ORT Commit Error: {}", e))?;

        Ok(session)
    }
}
