//! Docker container management for model runners
//!
//! This module provides functionality to run model runners (vLLM, etc.) inside
//! Docker containers with full configuration support including custom registries,
//! GPU passthrough, and environment variable mapping.

use std::collections::HashMap;

use serde::{Deserialize, Serialize};
use serde_json::Value;
use thiserror::Error;

/// Errors that can occur during Docker operations
#[derive(Error, Debug)]
pub enum DockerError {
    #[error("Docker not available: {0}")]
    NotAvailable(String),

    #[error("Invalid configuration: {0}")]
    ConfigError(String),

    #[error("Registry authentication failed: {0}")]
    AuthError(String),

    #[error("Image pull failed: {0}")]
    PullError(String),

    #[error("Container start failed: {0}")]
    StartError(String),

    #[error("Dockerfile fetch failed: {0}")]
    FetchError(String),

    #[error("Build failed: {0}")]
    BuildError(String),
}

/// Docker registry configuration
#[derive(Debug, Clone, Default, PartialEq, Deserialize, Serialize)]
pub struct RegistryConfig {
    /// Registry URL (e.g., "registry.example.com", "ghcr.io")
    /// If not specified, uses Docker Hub
    #[serde(skip_serializing_if = "Option::is_none")]
    pub url: Option<String>,

    /// Authentication token or password
    /// Supports env var expansion: "${DOCKER_PAT}"
    #[serde(skip_serializing_if = "Option::is_none")]
    pub token: Option<String>,

    /// Username for registry authentication
    #[serde(skip_serializing_if = "Option::is_none")]
    pub username: Option<String>,
}

/// Docker container configuration
#[derive(Debug, Clone, Default, PartialEq, Deserialize, Serialize)]
pub struct DockerConfig {
    /// Pre-built image name (e.g., "dgx-vllm:cutlass-nvfp4")
    /// Mutually exclusive with `dockerfile`
    #[serde(skip_serializing_if = "Option::is_none")]
    pub image: Option<String>,

    /// Path to Dockerfile (local or remote URL)
    /// Uses fetch_file() for retrieval
    /// Mutually exclusive with `image`
    #[serde(skip_serializing_if = "Option::is_none")]
    pub dockerfile: Option<String>,

    /// Build context directory (required if using dockerfile)
    #[serde(skip_serializing_if = "Option::is_none")]
    pub context: Option<String>,

    /// Custom registry configuration
    #[serde(skip_serializing_if = "Option::is_none")]
    pub registry: Option<RegistryConfig>,

    /// Container name prefix (auto-generated if not specified)
    #[serde(skip_serializing_if = "Option::is_none")]
    pub name: Option<String>,

    /// Network mode: "host", "bridge", or custom network name
    #[serde(default = "default_network")]
    pub network: String,

    /// GPU configuration: "all", specific IDs "0,1", or count "2"
    #[serde(skip_serializing_if = "Option::is_none")]
    pub gpus: Option<String>,

    /// IPC mode: "host", "private", "shareable"
    #[serde(skip_serializing_if = "Option::is_none")]
    pub ipc: Option<String>,

    /// Shared memory size (e.g., "16g")
    #[serde(skip_serializing_if = "Option::is_none")]
    pub shm_size: Option<String>,

    /// Volume mounts: ["host:container", ...]
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub volumes: Vec<String>,

    /// Additional environment variables (beyond parameter mapping)
    #[serde(default, skip_serializing_if = "HashMap::is_empty")]
    pub env: HashMap<String, String>,

    /// Extra arguments to pass to the container command
    /// For vLLM: "--swap-space 32 --tool-call-parser hermes"
    #[serde(skip_serializing_if = "Option::is_none")]
    pub extra_args: Option<String>,

    /// Run container in detached mode (default: true)
    #[serde(default = "default_detached")]
    pub detached: bool,

    /// Automatically remove container when stopped
    #[serde(default)]
    pub auto_remove: bool,

    /// Container restart policy: "no", "always", "unless-stopped", "on-failure"
    #[serde(skip_serializing_if = "Option::is_none")]
    pub restart: Option<String>,
}

fn default_network() -> String {
    "host".to_string()
}

fn default_detached() -> bool {
    true
}

impl DockerConfig {
    /// Validate the configuration
    pub fn validate(&self) -> Result<(), DockerError> {
        // Must have either image or dockerfile, but not both
        match (&self.image, &self.dockerfile) {
            (None, None) => {
                return Err(DockerError::ConfigError(
                    "Either 'image' or 'dockerfile' must be specified".to_string(),
                ));
            }
            (Some(_), Some(_)) => {
                return Err(DockerError::ConfigError(
                    "'image' and 'dockerfile' are mutually exclusive".to_string(),
                ));
            }
            _ => {}
        }

        Ok(())
    }

    /// Check if this config uses a Dockerfile (needs build)
    pub fn needs_build(&self) -> bool {
        self.dockerfile.is_some()
    }

    /// Get the effective image name
    pub fn effective_image(&self, model_name: &str) -> String {
        if let Some(image) = &self.image {
            image.clone()
        } else {
            // Generate image name from model name for Dockerfile builds
            format!("llmnet-{}", model_name.replace('/', "-").to_lowercase())
        }
    }
}

// ============================================================================
// SBIO: Pure business logic (no I/O)
// ============================================================================

/// Convert a parameter key to environment variable format
/// e.g., "tensor_parallel_size" -> "TENSOR_PARALLEL_SIZE"
pub fn param_to_env_var(key: &str) -> String {
    key.to_uppercase()
}

/// Map common parameter names to their Docker env var equivalents
pub fn map_param_name(key: &str) -> &str {
    match key {
        "gpu_memory_utilization" => "GPU_MEMORY_UTIL",
        _ => key,
    }
}

/// Generate Docker run arguments
///
/// Creates a complete `docker run` command with all options.
pub fn generate_run_args(
    config: &DockerConfig,
    model_source: &str,
    port: u16,
    parameters: &HashMap<String, Value>,
    container_name: &str,
) -> Vec<String> {
    let mut args = vec!["run".to_string()];

    // Detached mode
    if config.detached {
        args.push("-d".to_string());
    }

    // Container name
    args.push("--name".to_string());
    args.push(container_name.to_string());

    // Network mode
    args.push("--network".to_string());
    args.push(config.network.clone());

    // GPU configuration
    if let Some(gpus) = &config.gpus {
        args.push("--gpus".to_string());
        args.push(gpus.clone());
    }

    // IPC mode
    if let Some(ipc) = &config.ipc {
        args.push("--ipc".to_string());
        args.push(ipc.clone());
    }

    // Shared memory size
    if let Some(shm) = &config.shm_size {
        args.push("--shm-size".to_string());
        args.push(shm.clone());
    }

    // Auto remove
    if config.auto_remove {
        args.push("--rm".to_string());
    }

    // Restart policy
    if let Some(restart) = &config.restart {
        args.push("--restart".to_string());
        args.push(restart.clone());
    }

    // Environment variables from parameters
    // MODEL is the source
    args.push("-e".to_string());
    args.push(format!("MODEL={}", model_source));

    // PORT
    args.push("-e".to_string());
    args.push(format!("PORT={}", port));

    // Map all parameters to env vars
    for (key, value) in parameters {
        let env_name = param_to_env_var(map_param_name(key));
        let env_value = match value {
            Value::Bool(b) => b.to_string(),
            Value::Number(n) => n.to_string(),
            Value::String(s) => s.clone(),
            _ => continue,
        };
        args.push("-e".to_string());
        args.push(format!("{}={}", env_name, env_value));
    }

    // Extra vLLM args
    if let Some(extra) = &config.extra_args {
        args.push("-e".to_string());
        args.push(format!("VLLM_EXTRA_ARGS={}", extra));
    }

    // Additional env vars from config
    for (key, value) in &config.env {
        args.push("-e".to_string());
        args.push(format!("{}={}", key, expand_env_vars(value)));
    }

    // Volume mounts
    for vol in &config.volumes {
        args.push("-v".to_string());
        args.push(expand_env_vars(vol));
    }

    // Image name
    args.push(config.effective_image(container_name));

    // Container command (typically "serve" for vLLM)
    args.push("serve".to_string());

    args
}

/// Generate Docker build arguments
pub fn generate_build_args(
    dockerfile_path: &str,
    context_path: &str,
    image_name: &str,
) -> Vec<String> {
    vec![
        "build".to_string(),
        "-f".to_string(),
        dockerfile_path.to_string(),
        "-t".to_string(),
        image_name.to_string(),
        context_path.to_string(),
    ]
}

/// Generate Docker login arguments
/// Returns (args, password) where password should be piped to stdin
pub fn generate_login_args(registry: &RegistryConfig) -> Option<(Vec<String>, String)> {
    let token = registry.token.as_ref()?;
    let password = expand_env_vars(token);

    let mut args = vec!["login".to_string()];

    if let Some(username) = &registry.username {
        args.push("-u".to_string());
        args.push(username.clone());
    }

    args.push("--password-stdin".to_string());

    if let Some(url) = &registry.url {
        args.push(url.clone());
    }

    Some((args, password))
}

/// Generate Docker pull arguments
pub fn generate_pull_args(image: &str, registry: &Option<RegistryConfig>) -> Vec<String> {
    let full_image = if let Some(reg) = registry {
        if let Some(url) = &reg.url {
            format!("{}/{}", url, image)
        } else {
            image.to_string()
        }
    } else {
        image.to_string()
    };

    vec!["pull".to_string(), full_image]
}

/// Generate Docker stop arguments
pub fn generate_stop_args(container_name: &str) -> Vec<String> {
    vec!["stop".to_string(), container_name.to_string()]
}

/// Generate Docker rm arguments
pub fn generate_rm_args(container_name: &str) -> Vec<String> {
    vec!["rm".to_string(), "-f".to_string(), container_name.to_string()]
}

/// Expand environment variables in a string
/// Supports ${VAR} and $VAR syntax
pub fn expand_env_vars(input: &str) -> String {
    let mut result = input.to_string();

    // Handle ${VAR} syntax
    while let Some(start) = result.find("${") {
        if let Some(end) = result[start..].find('}') {
            let var_name = &result[start + 2..start + end];
            let var_value = std::env::var(var_name).unwrap_or_default();
            result = format!("{}{}{}", &result[..start], var_value, &result[start + end + 1..]);
        } else {
            break;
        }
    }

    result
}

/// Generate a unique container name
pub fn generate_container_name(prefix: &str, model_name: &str) -> String {
    let sanitized = model_name
        .replace('/', "-")
        .replace(':', "-")
        .to_lowercase();
    format!("{}-{}", prefix, sanitized)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_validate_image_config() {
        let config = DockerConfig {
            image: Some("dgx-vllm:latest".to_string()),
            ..Default::default()
        };
        assert!(config.validate().is_ok());
    }

    #[test]
    fn test_validate_dockerfile_config() {
        let config = DockerConfig {
            dockerfile: Some("./Dockerfile".to_string()),
            ..Default::default()
        };
        assert!(config.validate().is_ok());
    }

    #[test]
    fn test_validate_neither() {
        let config = DockerConfig::default();
        assert!(config.validate().is_err());
    }

    #[test]
    fn test_validate_both() {
        let config = DockerConfig {
            image: Some("image".to_string()),
            dockerfile: Some("./Dockerfile".to_string()),
            ..Default::default()
        };
        assert!(config.validate().is_err());
    }

    #[test]
    fn test_param_to_env_var() {
        assert_eq!(param_to_env_var("tensor_parallel_size"), "TENSOR_PARALLEL_SIZE");
        assert_eq!(param_to_env_var("max_model_len"), "MAX_MODEL_LEN");
    }

    #[test]
    fn test_map_param_name() {
        assert_eq!(map_param_name("gpu_memory_utilization"), "GPU_MEMORY_UTIL");
        assert_eq!(map_param_name("tensor_parallel_size"), "tensor_parallel_size");
    }

    #[test]
    fn test_generate_run_args() {
        let config = DockerConfig {
            image: Some("dgx-vllm:cutlass-nvfp4".to_string()),
            network: "host".to_string(),
            gpus: Some("all".to_string()),
            ipc: Some("host".to_string()),
            volumes: vec!["${HOME}/.cache/huggingface:/root/.cache/huggingface".to_string()],
            extra_args: Some("--swap-space 32".to_string()),
            detached: true,
            ..Default::default()
        };

        let mut params = HashMap::new();
        params.insert("tensor_parallel_size".to_string(), Value::Number(1.into()));
        params.insert("max_model_len".to_string(), Value::Number(131072.into()));

        let args = generate_run_args(
            &config,
            "RESMP-DEV/Qwen3-Next-80B",
            8888,
            &params,
            "test-container",
        );

        assert!(args.contains(&"run".to_string()));
        assert!(args.contains(&"-d".to_string()));
        assert!(args.contains(&"--network".to_string()));
        assert!(args.contains(&"host".to_string()));
        assert!(args.contains(&"--gpus".to_string()));
        assert!(args.contains(&"all".to_string()));
        assert!(args.contains(&"--ipc".to_string()));
        assert!(args.contains(&"-e".to_string()));
        assert!(args.iter().any(|a| a.contains("MODEL=RESMP-DEV/Qwen3-Next-80B")));
        assert!(args.iter().any(|a| a.contains("PORT=8888")));
        assert!(args.iter().any(|a| a.contains("TENSOR_PARALLEL_SIZE=1")));
        assert!(args.iter().any(|a| a.contains("MAX_MODEL_LEN=131072")));
        assert!(args.iter().any(|a| a.contains("VLLM_EXTRA_ARGS=--swap-space 32")));
    }

    #[test]
    fn test_generate_build_args() {
        let args = generate_build_args("./Dockerfile", ".", "my-image:latest");
        assert_eq!(args, vec!["build", "-f", "./Dockerfile", "-t", "my-image:latest", "."]);
    }

    #[test]
    fn test_generate_container_name() {
        assert_eq!(
            generate_container_name("llmnet", "meta-llama/Llama-2-7b"),
            "llmnet-meta-llama-llama-2-7b"
        );
    }

    #[test]
    fn test_expand_env_vars() {
        std::env::set_var("TEST_VAR", "test_value");
        assert_eq!(expand_env_vars("prefix-${TEST_VAR}-suffix"), "prefix-test_value-suffix");
        std::env::remove_var("TEST_VAR");
    }

    #[test]
    fn test_effective_image() {
        let config = DockerConfig {
            image: Some("my-image:v1".to_string()),
            ..Default::default()
        };
        assert_eq!(config.effective_image("test"), "my-image:v1");

        let config2 = DockerConfig {
            dockerfile: Some("./Dockerfile".to_string()),
            ..Default::default()
        };
        assert_eq!(config2.effective_image("my-model"), "llmnet-my-model");
    }

    #[test]
    fn test_generate_stop_args() {
        let args = generate_stop_args("my-container");
        assert_eq!(args, vec!["stop", "my-container"]);
    }

    #[test]
    fn test_generate_rm_args() {
        let args = generate_rm_args("my-container");
        assert_eq!(args, vec!["rm", "-f", "my-container"]);
    }
}
