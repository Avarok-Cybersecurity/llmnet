pub mod architecture;
pub mod composition;
pub mod functions;
pub mod models;
pub mod secrets;

pub use architecture::{
    ArchitectureNode, FailureAction, HookConfig, HookMode, NodeHooks, OutputTarget,
};
pub use composition::{
    parse_composition, strip_jsonc_comments, validate_composition, Composition, CompositionError,
};
pub use functions::{FunctionError, FunctionExecutor, FunctionResult, FunctionType, HttpMethod};
pub use models::{DockerModel, ExternalModel, HuggingfaceModel, ModelDefinition};
pub use secrets::{SecretError, SecretSource, SecretsManager};

use std::path::Path;
use thiserror::Error;

/// Errors for file I/O operations (separate from pure parsing errors)
#[derive(Error, Debug)]
pub enum ConfigError {
    #[error("Failed to read file: {0}")]
    IoError(#[from] std::io::Error),

    #[error("Composition error: {0}")]
    CompositionError(#[from] CompositionError),
}

// ============================================================================
// SBIO: I/O wrapper - thin layer over pure functions
// ============================================================================

/// Load and parse a composition file from disk.
/// This is the I/O boundary - it reads the file and delegates to pure parsing functions.
pub fn load_composition_file(path: &Path) -> Result<Composition, ConfigError> {
    let content = std::fs::read_to_string(path)?;
    let composition = Composition::from_str(&content)?;
    Ok(composition)
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::io::Write;
    use tempfile::NamedTempFile;

    fn create_temp_file(content: &str) -> NamedTempFile {
        let mut file = NamedTempFile::new().unwrap();
        file.write_all(content.as_bytes()).unwrap();
        file
    }

    #[test]
    fn test_load_composition_file() {
        let content = r#"{
            "models": {},
            "architecture": [
                {"name": "router", "layer": 0, "adapter": "openai-api"},
                {"name": "output", "adapter": "output"}
            ]
        }"#;

        let file = create_temp_file(content);
        let result = load_composition_file(file.path());
        assert!(result.is_ok());
    }

    #[test]
    fn test_load_nonexistent_file() {
        let result = load_composition_file(Path::new("/nonexistent/file.json"));
        assert!(matches!(result, Err(ConfigError::IoError(_))));
    }
}
