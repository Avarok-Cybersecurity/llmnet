//! vLLM configuration and CLI argument generation
//!
//! This module provides functionality to generate vLLM server configurations
//! and CLI arguments for launching vLLM OpenAI-compatible API servers.

use std::collections::HashMap;

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
        assert_eq!(endpoint_url("192.168.1.100", 8080), "http://192.168.1.100:8080/v1");
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
}
