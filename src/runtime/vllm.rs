//! vLLM configuration and CLI argument generation
//!
//! This module provides functionality to generate vLLM server configurations
//! and CLI arguments for launching vLLM OpenAI-compatible API servers.

use std::collections::HashMap;
use std::process::Command as StdCommand;

use serde_json::Value;

// ============================================================================
// SBIO: Pure business logic (no I/O)
// ============================================================================

/// Generate CLI arguments for vLLM server
///
/// Creates arguments for `python -m vllm.entrypoints.openai.api_server`
///
/// # Supported Parameters
/// - `tensor_parallel_size`: Number of GPUs for tensor parallelism
/// - `pipeline_parallel_size`: Number of GPUs for pipeline parallelism
/// - `max_model_len`: Maximum sequence length
/// - `gpu_memory_utilization`: Fraction of GPU memory to use (0-1)
/// - `dtype`: Data type (auto, float16, bfloat16, float32)
/// - `quantization`: Quantization method (awq, gptq, squeezellm)
/// - `enforce_eager`: Disable CUDA graph optimization
/// - `trust_remote_code`: Trust remote code from HuggingFace
/// - `max_num_seqs`: Maximum concurrent sequences
/// - `max_num_batched_tokens`: Maximum batched tokens
pub fn generate_args(
    model: &str,
    host: &str,
    port: u16,
    params: &HashMap<String, Value>,
) -> Vec<String> {
    let mut args = vec![
        "-m".to_string(),
        "vllm.entrypoints.openai.api_server".to_string(),
        "--model".to_string(),
        model.to_string(),
        "--host".to_string(),
        host.to_string(),
        "--port".to_string(),
        port.to_string(),
    ];

    for (key, value) in params {
        let arg_name = format!("--{}", key.replace('_', "-"));

        match value {
            Value::Bool(b) => {
                if *b {
                    args.push(arg_name);
                }
            }
            Value::Number(n) => {
                args.push(arg_name);
                args.push(n.to_string());
            }
            Value::String(s) => {
                args.push(arg_name);
                args.push(s.clone());
            }
            _ => {}
        }
    }

    args
}

/// Generate a command line string for vLLM server
pub fn generate_command(
    model: &str,
    host: &str,
    port: u16,
    params: &HashMap<String, Value>,
) -> String {
    let args = generate_args(model, host, port, params);
    format!("python {}", args.join(" "))
}

/// Get the default port for vLLM
pub const fn default_port() -> u16 {
    8000
}

/// Generate the endpoint URL for a vLLM instance
pub fn endpoint_url(host: &str, port: u16) -> String {
    format!("http://{}:{}/v1", host, port)
}

/// Common vLLM parameters with their default values
pub fn default_params() -> HashMap<String, Value> {
    use serde_json::Number;
    let mut params = HashMap::new();
    params.insert(
        "gpu_memory_utilization".to_string(),
        Value::Number(Number::from_f64(0.9).unwrap()),
    );
    params.insert("dtype".to_string(), Value::String("auto".to_string()));
    params
}

// ============================================================================
// I/O: System checks
// ============================================================================

/// Check if vLLM is installed
pub fn is_vllm_installed() -> bool {
    StdCommand::new("python")
        .args(["-c", "import vllm"])
        .output()
        .map(|o| o.status.success())
        .unwrap_or(false)
}

/// Check if CUDA is available
pub fn is_cuda_available() -> bool {
    StdCommand::new("python")
        .args(["-c", "import torch; assert torch.cuda.is_available()"])
        .output()
        .map(|o| o.status.success())
        .unwrap_or(false)
}

/// Get the number of available GPUs
pub fn gpu_count() -> usize {
    StdCommand::new("python")
        .args(["-c", "import torch; print(torch.cuda.device_count())"])
        .output()
        .ok()
        .and_then(|o| String::from_utf8(o.stdout).ok())
        .and_then(|s| s.trim().parse().ok())
        .unwrap_or(0)
}

/// Get HuggingFace token from environment or config
pub fn get_hf_token() -> Option<String> {
    // Try HF_TOKEN first (newer standard)
    if let Ok(token) = std::env::var("HF_TOKEN") {
        if !token.is_empty() {
            return Some(token);
        }
    }
    // Fall back to HUGGING_FACE_HUB_TOKEN
    if let Ok(token) = std::env::var("HUGGING_FACE_HUB_TOKEN") {
        if !token.is_empty() {
            return Some(token);
        }
    }
    None
}

/// Check if a model requires authentication (gated models)
pub fn model_requires_auth(model: &str) -> bool {
    // Common gated model prefixes
    let gated_prefixes = ["meta-llama/", "mistralai/Mistral", "google/gemma", "Qwen/"];

    gated_prefixes
        .iter()
        .any(|prefix| model.starts_with(prefix))
}

/// Generate environment variables for vLLM process
pub fn generate_env_vars(hf_token: Option<&str>) -> HashMap<String, String> {
    let mut env = HashMap::new();

    if let Some(token) = hf_token {
        env.insert("HF_TOKEN".to_string(), token.to_string());
        env.insert("HUGGING_FACE_HUB_TOKEN".to_string(), token.to_string());
    }

    // Disable tokenizer parallelism warning
    env.insert("TOKENIZERS_PARALLELISM".to_string(), "false".to_string());

    env
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_generate_args_basic() {
        let params = HashMap::new();
        let args = generate_args("meta-llama/Llama-2-7b-hf", "0.0.0.0", 8000, &params);

        assert!(args.contains(&"-m".to_string()));
        assert!(args.contains(&"vllm.entrypoints.openai.api_server".to_string()));
        assert!(args.contains(&"--model".to_string()));
        assert!(args.contains(&"meta-llama/Llama-2-7b-hf".to_string()));
        assert!(args.contains(&"--host".to_string()));
        assert!(args.contains(&"0.0.0.0".to_string()));
        assert!(args.contains(&"--port".to_string()));
        assert!(args.contains(&"8000".to_string()));
    }

    #[test]
    fn test_generate_args_with_params() {
        let mut params = HashMap::new();
        params.insert("tensor_parallel_size".to_string(), Value::Number(2.into()));
        params.insert("max_model_len".to_string(), Value::Number(4096.into()));
        params.insert("trust_remote_code".to_string(), Value::Bool(true));
        params.insert("dtype".to_string(), Value::String("float16".to_string()));

        let args = generate_args("model", "localhost", 8000, &params);

        assert!(args.contains(&"--tensor-parallel-size".to_string()));
        assert!(args.contains(&"2".to_string()));
        assert!(args.contains(&"--max-model-len".to_string()));
        assert!(args.contains(&"4096".to_string()));
        assert!(args.contains(&"--trust-remote-code".to_string()));
        assert!(args.contains(&"--dtype".to_string()));
        assert!(args.contains(&"float16".to_string()));
    }

    #[test]
    fn test_generate_command() {
        let params = HashMap::new();
        let cmd = generate_command("llama2", "0.0.0.0", 8000, &params);

        assert!(cmd.starts_with("python -m vllm.entrypoints.openai.api_server"));
        assert!(cmd.contains("--model llama2"));
    }

    #[test]
    fn test_endpoint_url() {
        assert_eq!(endpoint_url("localhost", 8000), "http://localhost:8000/v1");
        assert_eq!(
            endpoint_url("192.168.1.100", 8080),
            "http://192.168.1.100:8080/v1"
        );
    }

    #[test]
    fn test_default_port() {
        assert_eq!(default_port(), 8000);
    }

    #[test]
    fn test_default_params() {
        let params = default_params();
        assert!(params.contains_key("gpu_memory_utilization"));
        assert!(params.contains_key("dtype"));
    }

    #[test]
    fn test_model_requires_auth() {
        assert!(model_requires_auth("meta-llama/Llama-2-7b-hf"));
        assert!(model_requires_auth("mistralai/Mistral-7B-v0.1"));
        assert!(model_requires_auth("google/gemma-7b"));
        assert!(model_requires_auth("Qwen/Qwen-7B"));
        assert!(!model_requires_auth("facebook/opt-125m"));
        assert!(!model_requires_auth("gpt2"));
    }

    #[test]
    fn test_generate_env_vars_with_token() {
        let env = generate_env_vars(Some("hf_test_token"));
        assert_eq!(env.get("HF_TOKEN"), Some(&"hf_test_token".to_string()));
        assert_eq!(
            env.get("HUGGING_FACE_HUB_TOKEN"),
            Some(&"hf_test_token".to_string())
        );
        assert_eq!(
            env.get("TOKENIZERS_PARALLELISM"),
            Some(&"false".to_string())
        );
    }

    #[test]
    fn test_generate_env_vars_without_token() {
        let env = generate_env_vars(None);
        assert!(env.get("HF_TOKEN").is_none());
        assert_eq!(
            env.get("TOKENIZERS_PARALLELISM"),
            Some(&"false".to_string())
        );
    }
}
