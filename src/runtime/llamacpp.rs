//! llama.cpp server configuration and CLI argument generation
//!
//! This module provides functionality to generate CLI arguments for
//! llama.cpp's `llama-server` (OpenAI-compatible API server).

use std::collections::HashMap;

use serde_json::Value;

// ============================================================================
// SBIO: Pure business logic (no I/O)
// ============================================================================

/// Generate CLI arguments for llama-server
///
/// Creates arguments for `llama-server` (llama.cpp OpenAI-compatible server)
///
/// # Supported Parameters
/// - `n_ctx` / `ctx_size`: Context size (default: 2048)
/// - `n_gpu_layers` / `ngl`: Number of layers to offload to GPU
/// - `n_threads` / `threads`: Number of threads
/// - `n_batch`: Batch size for prompt processing
/// - `rope_freq_base`: RoPE base frequency
/// - `rope_freq_scale`: RoPE frequency scaling
/// - `flash_attn` / `fa`: Enable flash attention
/// - `mlock`: Lock model in memory
/// - `no_mmap`: Don't use memory mapping
/// - `embedding`: Enable embedding mode
/// - `cont_batching`: Enable continuous batching
pub fn generate_args(
    model: &str,
    host: &str,
    port: u16,
    params: &HashMap<String, Value>,
) -> Vec<String> {
    let mut args = vec![
        "--model".to_string(),
        model.to_string(),
        "--host".to_string(),
        host.to_string(),
        "--port".to_string(),
        port.to_string(),
    ];

    for (key, value) in params {
        // Normalize parameter names
        let arg_name = match key.as_str() {
            "n_ctx" | "ctx_size" => "--ctx-size".to_string(),
            "n_gpu_layers" | "ngl" => "--n-gpu-layers".to_string(),
            "n_threads" | "threads" => "--threads".to_string(),
            "n_batch" => "--batch-size".to_string(),
            "flash_attn" | "fa" => "--flash-attn".to_string(),
            _ => format!("--{}", key.replace('_', "-")),
        };

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

/// Generate a command line string for llama-server
pub fn generate_command(
    model: &str,
    host: &str,
    port: u16,
    params: &HashMap<String, Value>,
) -> String {
    let args = generate_args(model, host, port, params);
    format!("llama-server {}", args.join(" "))
}

/// Get the default port for llama-server
pub const fn default_port() -> u16 {
    8080
}

/// Generate the endpoint URL for a llama-server instance
pub fn endpoint_url(host: &str, port: u16) -> String {
    format!("http://{}:{}/v1", host, port)
}

/// Common llama-server parameters with reasonable defaults
pub fn default_params() -> HashMap<String, Value> {
    let mut params = HashMap::new();
    params.insert("n_ctx".to_string(), Value::Number(2048.into()));
    params.insert("n_gpu_layers".to_string(), Value::Number((-1).into())); // All layers on GPU
    params.insert("cont_batching".to_string(), Value::Bool(true));
    params
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_generate_args_basic() {
        let params = HashMap::new();
        let args = generate_args("/path/to/model.gguf", "0.0.0.0", 8080, &params);

        assert!(args.contains(&"--model".to_string()));
        assert!(args.contains(&"/path/to/model.gguf".to_string()));
        assert!(args.contains(&"--host".to_string()));
        assert!(args.contains(&"0.0.0.0".to_string()));
        assert!(args.contains(&"--port".to_string()));
        assert!(args.contains(&"8080".to_string()));
    }

    #[test]
    fn test_generate_args_with_params() {
        let mut params = HashMap::new();
        params.insert("n_ctx".to_string(), Value::Number(4096.into()));
        params.insert("n_gpu_layers".to_string(), Value::Number(35.into()));
        params.insert("flash_attn".to_string(), Value::Bool(true));
        params.insert("n_threads".to_string(), Value::Number(8.into()));

        let args = generate_args("/model.gguf", "localhost", 8080, &params);

        assert!(args.contains(&"--ctx-size".to_string()));
        assert!(args.contains(&"4096".to_string()));
        assert!(args.contains(&"--n-gpu-layers".to_string()));
        assert!(args.contains(&"35".to_string()));
        assert!(args.contains(&"--flash-attn".to_string()));
        assert!(args.contains(&"--threads".to_string()));
        assert!(args.contains(&"8".to_string()));
    }

    #[test]
    fn test_parameter_aliases() {
        let mut params = HashMap::new();
        params.insert("ctx_size".to_string(), Value::Number(2048.into()));
        params.insert("ngl".to_string(), Value::Number(20.into()));
        params.insert("threads".to_string(), Value::Number(4.into()));
        params.insert("fa".to_string(), Value::Bool(true));

        let args = generate_args("model.gguf", "localhost", 8080, &params);

        // Aliases should be normalized
        assert!(args.contains(&"--ctx-size".to_string()));
        assert!(args.contains(&"--n-gpu-layers".to_string()));
        assert!(args.contains(&"--threads".to_string()));
        assert!(args.contains(&"--flash-attn".to_string()));
    }

    #[test]
    fn test_generate_command() {
        let params = HashMap::new();
        let cmd = generate_command("/model.gguf", "0.0.0.0", 8080, &params);

        assert!(cmd.starts_with("llama-server"));
        assert!(cmd.contains("--model /model.gguf"));
    }

    #[test]
    fn test_endpoint_url() {
        assert_eq!(endpoint_url("localhost", 8080), "http://localhost:8080/v1");
        assert_eq!(endpoint_url("192.168.1.100", 9000), "http://192.168.1.100:9000/v1");
    }

    #[test]
    fn test_default_port() {
        assert_eq!(default_port(), 8080);
    }

    #[test]
    fn test_default_params() {
        let params = default_params();
        assert!(params.contains_key("n_ctx"));
        assert!(params.contains_key("n_gpu_layers"));
        assert!(params.contains_key("cont_batching"));
    }
}
