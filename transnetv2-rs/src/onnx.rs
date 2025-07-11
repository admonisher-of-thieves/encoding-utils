// #![allow(unused_imports)]
use eyre::{Result, eyre};
use ort::{
    self,
    execution_providers::ExecutionProviderDispatch,
    session::{Session, builder::GraphOptimizationLevel},
};

#[cfg(target_os = "macos")]
use ort::execution_providers::CoreMLExecutionProvider;

#[cfg(windows)]
use ort::execution_providers::{
    CUDAExecutionProvider, DirectMLExecutionProvider, TensorRTExecutionProvider,
};

#[cfg(all(unix, not(target_os = "macos")))]
use ort::execution_providers::{CUDAExecutionProvider, ROCmExecutionProvider};

use std::path::Path;

#[derive(Debug)]
pub struct TransNetSession {
    pub session: Session,
}

impl TransNetSession {
    pub fn new(model_path: Option<impl AsRef<Path>>, use_cpu: bool) -> Result<Self> {
        let providers = if use_cpu {
            vec![]
        } else {
            Self::preferred_execution_providers()
        };
        let session = match model_path {
            Some(path) => Self::init_session_from_file(path.as_ref(), &providers)?,
            None => Self::init_session_from_embedded(&providers)?,
        };

        Ok(Self { session })
    }

    pub fn init_session_from_file(
        model_path: &Path,
        execution_providers: &[ExecutionProviderDispatch],
    ) -> Result<Session> {
        Session::builder()?
            .with_optimization_level(GraphOptimizationLevel::Level3)?
            .with_intra_threads(4)?
            .with_execution_providers(execution_providers)?
            .commit_from_file(model_path)
            .map_err(|e| eyre!("Failed to load model from file: {}", e))
    }

    pub fn init_session_from_embedded(
        execution_providers: &[ExecutionProviderDispatch],
    ) -> Result<Session> {
        // Embedded model bytes (compile-time included)
        const MODEL_BYTES: &[u8] = include_bytes!("../models/transnetv2.onnx");

        // Create a temporary file to hold the model bytes
        let temp_dir = tempfile::tempdir()?;
        let model_path = temp_dir.path().join("transnetv2.onnx");
        std::fs::write(&model_path, MODEL_BYTES)?;

        // Create the session from the temp file
        let session = Session::builder()?
            .with_optimization_level(GraphOptimizationLevel::Level3)?
            .with_intra_threads(4)?
            .with_execution_providers(execution_providers)?
            .commit_from_file(&model_path)
            .map_err(|e| eyre!("Failed to load embedded model: {}", e))?;

        // Keep the temp directory alive for the session's lifetime
        std::mem::forget(temp_dir);

        Ok(session)
    }

    pub fn preferred_execution_providers() -> Vec<ExecutionProviderDispatch> {
        let mut providers = Vec::new();

        #[cfg(target_os = "macos")]
        {
            providers.push(CoreMLExecutionProvider::default().build());
        }

        #[cfg(windows)]
        {
            providers.push(CUDAExecutionProvider::default().build());
            providers.push(TensorRTExecutionProvider::default().build());
            providers.push(DirectMLExecutionProvider::default().build());
        }

        #[cfg(all(unix, not(target_os = "macos")))]
        {
            providers.push(ROCmExecutionProvider::default().build());
            providers.push(CUDAExecutionProvider::default().build());
        }

        providers
    }
}
