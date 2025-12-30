use std::collections::HashMap;

use serde::{Deserialize, Serialize};
use serde_json::Value;

use crate::runtime::docker::DockerConfig;

/// Runner type for model execution
#[derive(Debug, Clone, PartialEq, Deserialize, Serialize, Default)]
#[serde(rename_all = "kebab-case")]
pub enum RunnerType {
    /// External API endpoint (e.g., OpenAI, Anthropic, self-hosted)
    #[default]
    External,
    /// Ollama local runner
    Ollama,
    /// vLLM local runner
    Vllm,
    /// llama.cpp local runner
    LlamaCpp,
    /// Docker-based runner
    Docker,
}

impl RunnerType {
    /// Get the default port for this runner type
    pub fn default_port(&self) -> Option<u16> {
        match self {
            RunnerType::External => None,
            RunnerType::Ollama => Some(11434),
            RunnerType::Vllm => Some(8000),
            RunnerType::LlamaCpp => Some(8080),
            RunnerType::Docker => None,
        }
    }

    /// Check if this runner needs to be spawned as a subprocess
    pub fn is_local_runner(&self) -> bool {
        matches!(self, RunnerType::Ollama | RunnerType::Vllm | RunnerType::LlamaCpp)
    }
}

/// Unified model configuration
///
/// This structure supports all model types through a common interface:
/// - `runner`: The execution backend (external, ollama, vllm, llama-cpp, docker)
/// - `interface`: The API protocol (openai-api)
/// - `source`: Model file, URL, HuggingFace repo, or model name
/// - `endpoint`: Explicit endpoint URL (for external runners)
/// - `parameters`: Runner-specific parameters
#[derive(Debug, Clone, PartialEq, Deserialize, Serialize)]
pub struct ModelConfig {
    /// Runner type: external, ollama, vllm, llama-cpp, docker
    #[serde(default)]
    pub runner: RunnerType,

    /// API interface: openai-api (default)
    #[serde(default = "default_interface")]
    pub interface: String,

    /// Model source: URL, local path, HF repo, or model name
    /// - External: not used (use endpoint instead)
    /// - Ollama: model name (e.g., "tinyllama:1.1b") or Modelfile path
    /// - vLLM: HuggingFace repo (e.g., "meta-llama/Llama-2-7b-hf")
    /// - llama.cpp: GGUF file path or URL
    #[serde(skip_serializing_if = "Option::is_none")]
    pub source: Option<String>,

    /// Endpoint URL (required for external, auto-generated for local runners)
    #[serde(skip_serializing_if = "Option::is_none")]
    pub endpoint: Option<String>,

    /// API key for authentication
    #[serde(rename = "api-key", skip_serializing_if = "Option::is_none")]
    pub api_key: Option<String>,

    /// Runner-specific parameters
    #[serde(default, skip_serializing_if = "HashMap::is_empty")]
    pub parameters: HashMap<String, Value>,

    /// Docker configuration (for runner: "docker")
    #[serde(skip_serializing_if = "Option::is_none")]
    pub docker: Option<DockerConfig>,
}

fn default_interface() -> String {
    "openai-api".to_string()
}

impl Default for ModelConfig {
    fn default() -> Self {
        Self {
            runner: RunnerType::External,
            interface: default_interface(),
            source: None,
            endpoint: None,
            api_key: None,
            parameters: HashMap::new(),
            docker: None,
        }
    }
}

impl ModelConfig {
    /// Create a new external model configuration
    pub fn external(endpoint: impl Into<String>) -> Self {
        Self {
            runner: RunnerType::External,
            endpoint: Some(endpoint.into()),
            ..Default::default()
        }
    }

    /// Create a new Ollama model configuration
    pub fn ollama(source: impl Into<String>) -> Self {
        Self {
            runner: RunnerType::Ollama,
            source: Some(source.into()),
            ..Default::default()
        }
    }

    /// Create a new vLLM model configuration
    pub fn vllm(source: impl Into<String>) -> Self {
        Self {
            runner: RunnerType::Vllm,
            source: Some(source.into()),
            ..Default::default()
        }
    }

    /// Create a new llama.cpp model configuration
    pub fn llamacpp(source: impl Into<String>) -> Self {
        Self {
            runner: RunnerType::LlamaCpp,
            source: Some(source.into()),
            ..Default::default()
        }
    }

    /// Create a new Docker-based model configuration
    pub fn docker(source: impl Into<String>, docker_config: DockerConfig) -> Self {
        Self {
            runner: RunnerType::Docker,
            source: Some(source.into()),
            docker: Some(docker_config),
            ..Default::default()
        }
    }

    /// Add Docker configuration
    pub fn with_docker(mut self, docker_config: DockerConfig) -> Self {
        self.docker = Some(docker_config);
        self
    }

    /// Add an API key
    pub fn with_api_key(mut self, key: impl Into<String>) -> Self {
        self.api_key = Some(key.into());
        self
    }

    /// Add parameters
    pub fn with_parameters(mut self, params: HashMap<String, Value>) -> Self {
        self.parameters = params;
        self
    }

    /// Add a single parameter
    pub fn with_parameter(mut self, key: impl Into<String>, value: Value) -> Self {
        self.parameters.insert(key.into(), value);
        self
    }

    /// Get the effective endpoint URL
    ///
    /// For external runners, returns the configured endpoint.
    /// For local runners, generates based on runner defaults.
    pub fn effective_endpoint(&self, host: &str, port: Option<u16>) -> Option<String> {
        if let Some(endpoint) = &self.endpoint {
            return Some(endpoint.clone());
        }

        let port = port.or_else(|| self.runner.default_port())?;

        Some(match self.runner {
            RunnerType::External => return None,
            RunnerType::Ollama => format!("http://{}:{}/v1", host, port),
            RunnerType::Vllm => format!("http://{}:{}/v1", host, port),
            RunnerType::LlamaCpp => format!("http://{}:{}/v1", host, port),
            RunnerType::Docker => return None,
        })
    }

    /// Get the runner type name as a string
    pub fn type_name(&self) -> &'static str {
        match self.runner {
            RunnerType::External => "external",
            RunnerType::Ollama => "ollama",
            RunnerType::Vllm => "vllm",
            RunnerType::LlamaCpp => "llama-cpp",
            RunnerType::Docker => "docker",
        }
    }
}

// ============================================================================
// Legacy support: Map old ModelDefinition to new ModelConfig
// ============================================================================

/// Model definition types (legacy format for backward compatibility)
#[derive(Debug, Clone, PartialEq, Deserialize, Serialize)]
#[serde(tag = "type", rename_all = "lowercase")]
pub enum ModelDefinition {
    External(ExternalModel),
    Docker(DockerModel),
    Huggingface(HuggingfaceModel),
    /// New unified format
    #[serde(untagged)]
    Unified(ModelConfig),
}

/// External OpenAI-compatible API endpoint (legacy)
#[derive(Debug, Clone, PartialEq, Deserialize, Serialize)]
pub struct ExternalModel {
    pub interface: String,
    pub url: String,
    #[serde(rename = "api-key")]
    pub api_key: Option<String>,
}

/// Docker-based model runner (legacy)
#[derive(Debug, Clone, PartialEq, Deserialize, Serialize)]
pub struct DockerModel {
    pub image: String,
    pub pat: Option<String>,
    pub registry_url: Option<String>,
    pub params: Option<String>,
}

/// HuggingFace model with runner specification (legacy)
#[derive(Debug, Clone, PartialEq, Deserialize, Serialize)]
pub struct HuggingfaceModel {
    pub url: String,
    pub hf_pat: Option<String>,
    pub runner: String,
}

impl ModelDefinition {
    /// Get the model type as a string for display
    pub fn type_name(&self) -> &'static str {
        match self {
            ModelDefinition::External(_) => "external",
            ModelDefinition::Docker(_) => "docker",
            ModelDefinition::Huggingface(_) => "huggingface",
            ModelDefinition::Unified(config) => config.type_name(),
        }
    }

    /// Convert to unified ModelConfig
    pub fn to_config(&self) -> ModelConfig {
        match self {
            ModelDefinition::External(ext) => ModelConfig {
                runner: RunnerType::External,
                interface: ext.interface.clone(),
                endpoint: Some(ext.url.clone()),
                api_key: ext.api_key.clone(),
                source: None,
                parameters: HashMap::new(),
                docker: None,
            },
            ModelDefinition::Docker(docker_legacy) => ModelConfig {
                runner: RunnerType::Docker,
                interface: "openai-api".to_string(),
                source: Some(docker_legacy.image.clone()),
                endpoint: None,
                api_key: None,
                parameters: HashMap::new(),
                docker: None, // Legacy format doesn't have full Docker config
            },
            ModelDefinition::Huggingface(hf) => {
                let runner = match hf.runner.as_str() {
                    "ollama" => RunnerType::Ollama,
                    "vllm" => RunnerType::Vllm,
                    "llama-cpp" | "llamacpp" => RunnerType::LlamaCpp,
                    _ => RunnerType::External,
                };
                ModelConfig {
                    runner,
                    interface: "openai-api".to_string(),
                    source: Some(hf.url.clone()),
                    endpoint: None,
                    api_key: None,
                    parameters: HashMap::new(),
                    docker: None,
                }
            }
            ModelDefinition::Unified(config) => config.clone(),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_parse_external_model_legacy() {
        let json = r#"{
            "type": "external",
            "interface": "openai-api",
            "url": "https://api.example.com",
            "api-key": "sk-test"
        }"#;

        let model: ModelDefinition = serde_json::from_str(json).unwrap();
        match model {
            ModelDefinition::External(ext) => {
                assert_eq!(ext.interface, "openai-api");
                assert_eq!(ext.url, "https://api.example.com");
                assert_eq!(ext.api_key, Some("sk-test".to_string()));
            }
            _ => panic!("Expected External model"),
        }
    }

    #[test]
    fn test_parse_unified_model() {
        let json = r#"{
            "runner": "ollama",
            "interface": "openai-api",
            "source": "tinyllama:1.1b",
            "parameters": {
                "temperature": 0.7,
                "num_ctx": 2048
            }
        }"#;

        let config: ModelConfig = serde_json::from_str(json).unwrap();
        assert_eq!(config.runner, RunnerType::Ollama);
        assert_eq!(config.source, Some("tinyllama:1.1b".to_string()));
        assert!(config.parameters.contains_key("temperature"));
    }

    #[test]
    fn test_parse_vllm_model() {
        let json = r#"{
            "runner": "vllm",
            "source": "meta-llama/Llama-2-7b-hf",
            "parameters": {
                "tensor_parallel_size": 2
            }
        }"#;

        let config: ModelConfig = serde_json::from_str(json).unwrap();
        assert_eq!(config.runner, RunnerType::Vllm);
        assert_eq!(config.interface, "openai-api");
    }

    #[test]
    fn test_parse_llamacpp_model() {
        let json = r#"{
            "runner": "llama-cpp",
            "source": "/path/to/model.gguf",
            "parameters": {
                "n_ctx": 4096,
                "n_gpu_layers": 35
            }
        }"#;

        let config: ModelConfig = serde_json::from_str(json).unwrap();
        assert_eq!(config.runner, RunnerType::LlamaCpp);
    }

    #[test]
    fn test_runner_default_ports() {
        assert_eq!(RunnerType::External.default_port(), None);
        assert_eq!(RunnerType::Ollama.default_port(), Some(11434));
        assert_eq!(RunnerType::Vllm.default_port(), Some(8000));
        assert_eq!(RunnerType::LlamaCpp.default_port(), Some(8080));
    }

    #[test]
    fn test_effective_endpoint() {
        let config = ModelConfig::ollama("tinyllama:1.1b");
        assert_eq!(
            config.effective_endpoint("localhost", None),
            Some("http://localhost:11434/v1".to_string())
        );

        let config = ModelConfig::vllm("llama2");
        assert_eq!(
            config.effective_endpoint("0.0.0.0", Some(9000)),
            Some("http://0.0.0.0:9000/v1".to_string())
        );
    }

    #[test]
    fn test_model_builders() {
        use serde_json::Number;
        let ollama = ModelConfig::ollama("tinyllama:1.1b")
            .with_parameter("temperature".to_string(), Value::Number(Number::from_f64(0.7).unwrap()));
        assert_eq!(ollama.runner, RunnerType::Ollama);
        assert!(ollama.parameters.contains_key("temperature"));

        let external = ModelConfig::external("http://api.example.com")
            .with_api_key("sk-test");
        assert_eq!(external.runner, RunnerType::External);
        assert_eq!(external.api_key, Some("sk-test".to_string()));
    }

    #[test]
    fn test_legacy_to_config() {
        let legacy = ModelDefinition::External(ExternalModel {
            interface: "openai-api".to_string(),
            url: "http://test".to_string(),
            api_key: Some("key".to_string()),
        });

        let config = legacy.to_config();
        assert_eq!(config.runner, RunnerType::External);
        assert_eq!(config.endpoint, Some("http://test".to_string()));
    }

    #[test]
    fn test_is_local_runner() {
        assert!(!RunnerType::External.is_local_runner());
        assert!(RunnerType::Ollama.is_local_runner());
        assert!(RunnerType::Vllm.is_local_runner());
        assert!(RunnerType::LlamaCpp.is_local_runner());
        assert!(!RunnerType::Docker.is_local_runner());
    }

    #[test]
    fn test_parse_docker_model() {
        let json = r#"{
            "runner": "docker",
            "source": "RESMP-DEV/Qwen3-Next-80B-A3B-Instruct-NVFP4",
            "docker": {
                "image": "dgx-vllm:cutlass-nvfp4",
                "network": "host",
                "gpus": "all",
                "ipc": "host",
                "volumes": ["${HOME}/.cache/huggingface:/root/.cache/huggingface"],
                "extra_args": {
                    "swap-space": 32,
                    "tool-call-parser": "hermes",
                    "enable-auto-tool-choice": true
                }
            },
            "parameters": {
                "tensor_parallel_size": 1,
                "max_model_len": 131072,
                "gpu_memory_utilization": 0.80,
                "max_num_seqs": 128
            }
        }"#;

        let config: ModelConfig = serde_json::from_str(json).unwrap();
        assert_eq!(config.runner, RunnerType::Docker);
        assert_eq!(config.source, Some("RESMP-DEV/Qwen3-Next-80B-A3B-Instruct-NVFP4".to_string()));

        let docker = config.docker.as_ref().unwrap();
        assert_eq!(docker.image, Some("dgx-vllm:cutlass-nvfp4".to_string()));
        assert_eq!(docker.network, "host");
        assert_eq!(docker.gpus, Some("all".to_string()));
        assert_eq!(docker.ipc, Some("host".to_string()));
        assert_eq!(docker.volumes.len(), 1);
        assert!(docker.extra_args.contains_key("swap-space"));
        assert!(docker.extra_args.contains_key("tool-call-parser"));
        assert!(docker.extra_args.contains_key("enable-auto-tool-choice"));

        assert!(config.parameters.contains_key("tensor_parallel_size"));
        assert!(config.parameters.contains_key("max_model_len"));
    }

    #[test]
    fn test_parse_docker_with_registry() {
        let json = r#"{
            "runner": "docker",
            "source": "meta-llama/Llama-2-7b",
            "docker": {
                "image": "vllm-openai:latest",
                "registry": {
                    "url": "ghcr.io",
                    "username": "myuser",
                    "token": "${GHCR_TOKEN}"
                },
                "gpus": "0,1"
            }
        }"#;

        let config: ModelConfig = serde_json::from_str(json).unwrap();
        let docker = config.docker.as_ref().unwrap();
        let registry = docker.registry.as_ref().unwrap();

        assert_eq!(registry.url, Some("ghcr.io".to_string()));
        assert_eq!(registry.username, Some("myuser".to_string()));
        assert_eq!(registry.token, Some("${GHCR_TOKEN}".to_string()));
    }

    #[test]
    fn test_parse_docker_with_dockerfile() {
        let json = r#"{
            "runner": "docker",
            "source": "my-model",
            "docker": {
                "dockerfile": "./docker/Dockerfile.vllm",
                "context": "./docker"
            }
        }"#;

        let config: ModelConfig = serde_json::from_str(json).unwrap();
        let docker = config.docker.as_ref().unwrap();

        assert!(docker.image.is_none());
        assert_eq!(docker.dockerfile, Some("./docker/Dockerfile.vllm".to_string()));
        assert_eq!(docker.context, Some("./docker".to_string()));
    }

    #[test]
    fn test_docker_builder() {
        let docker_config = DockerConfig {
            image: Some("my-image:v1".to_string()),
            network: "host".to_string(),
            gpus: Some("all".to_string()),
            ..Default::default()
        };

        let config = ModelConfig::docker("my-model", docker_config);
        assert_eq!(config.runner, RunnerType::Docker);
        assert_eq!(config.source, Some("my-model".to_string()));
        assert!(config.docker.is_some());
    }
}
