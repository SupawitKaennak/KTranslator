use crate::infrastructure::settings::GpuBackend;
use anyhow::Result;
use ort::ep::{CUDAExecutionProvider, DirectMLExecutionProvider, TensorRTExecutionProvider};
use ort::session::{builder::SessionBuilder, Session};
use std::path::Path;

/// Generic ONNX engine execution wrapper initializing sessions with hardware acceleration.
pub struct OnnxEngine;

impl OnnxEngine {
    /// Creates an ORT Session configured with the specified GPU backend.
    /// `vram_limit_mb`: maximum GPU memory in MB (0 = unlimited, CUDA only).
    pub fn create_session<P: AsRef<Path>>(
        model_path: P,
        gpu_backend: GpuBackend,
        vram_limit_mb: u32,
    ) -> Result<Session> {
        let builder =
            SessionBuilder::new().map_err(|e| anyhow::anyhow!("ORT Builder Error: {}", e))?;

        // Build CUDA provider with optional VRAM cap
        let cuda_ep = if vram_limit_mb > 0 {
            let limit_bytes = (vram_limit_mb as usize) * 1024 * 1024;
            CUDAExecutionProvider::default()
                .with_memory_limit(limit_bytes)
                .build()
        } else {
            CUDAExecutionProvider::default().build()
        };

        // Select execution providers based on user's GPU backend setting
        let execution_providers = match gpu_backend {
            GpuBackend::Auto => vec![
                TensorRTExecutionProvider::default().build(),
                cuda_ep,
                DirectMLExecutionProvider::default().build(),
            ],
            GpuBackend::Cuda => vec![cuda_ep],
            GpuBackend::DirectMl => vec![DirectMLExecutionProvider::default().build()],
            GpuBackend::TensorRt => vec![TensorRTExecutionProvider::default().build()],
            GpuBackend::Cpu => vec![],
        };

        let mut builder = builder
            .with_execution_providers(execution_providers)
            .map_err(|e| anyhow::anyhow!("ORT EP Error: {}", e))?;

        // Log which GPU backend is being used for verification
        tracing::info!(
            "Creating ONNX session for {:?} with GPU backend: {:?}, VRAM limit: {}MB",
            model_path.as_ref(),
            gpu_backend,
            if vram_limit_mb == 0 { "unlimited".to_string() } else { vram_limit_mb.to_string() },
        );

        let session = builder
            .commit_from_file(model_path)
            .map_err(|e| anyhow::anyhow!("ORT Commit Error: {}", e))?;

        Ok(session)
    }
}
