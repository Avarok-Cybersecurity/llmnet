//! TensorRT-LLM configuration and CLI argument generation
//!
//! This module provides functionality to generate TensorRT-LLM server configurations
//! for NVIDIA Jetson and GPU-accelerated edge devices. TensorRT-LLM provides
//! optimized inference performance through NVIDIA's TensorRT optimization toolkit.
//!
//! # Supported Platforms
//! - NVIDIA Jetson Orin Nano (8GB) - Up to 7B models with INT4 quantization
//! - NVIDIA Jetson Orin NX (16GB) - Up to 13B models
//! - NVIDIA Jetson AGX Orin (32/64GB) - Full model support
//! - Desktop/Server GPUs with TensorRT-LLM installed
//!
//! # Example Configuration
//! ```json
//! {
//!     "runner": "tensorrt-llm",
//!     "source": "meta-llama/Llama-3.2-3B-Instruct",
//!     "parameters": {
//!         "max_batch_size": 8,
//!         "max_input_len": 2048,
//!         "max_output_len": 512,
//!         "quantization": "int4_awq",
//!         "tp_size": 1
//!     }
//! }
//! ```

use std::collections::HashMap;
use std::process::Command as StdCommand;

use serde_json::Value;

// ============================================================================
// SBIO: Pure business logic (no I/O)
// ============================================================================

/// Generate CLI arguments for TensorRT-LLM server
///
/// Creates arguments for `python -m tensorrt_llm.serve`
///
/// # Supported Parameters
/// - `max_batch_size`: Maximum batch size for inference
/// - `max_input_len`: Maximum input sequence length
/// - `max_output_len`: Maximum output sequence length
/// - `max_beam_width`: Beam search width (1 = greedy)
/// - `tp_size`: Tensor parallelism size (number of GPUs)
/// - `pp_size`: Pipeline parallelism size
/// - `quantization`: Quantization method (int8, int4_awq, fp8)
/// - `use_custom_all_reduce`: Enable custom all-reduce for multi-GPU
/// - `enable_chunked_context`: Enable chunked context for long sequences
/// - `kv_cache_free_gpu_mem_fraction`: Fraction of GPU memory for KV cache
pub fn generate_args(
    model: &str,
    host: &str,
    port: u16,
    params: &HashMap<String, Value>,
) -> Vec<String> {
    let mut args = vec![
        "-m".to_string(),
        "tensorrt_llm.serve".to_string(),
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

/// Generate a command line string for TensorRT-LLM server
pub fn generate_command(
    model: &str,
    host: &str,
    port: u16,
    params: &HashMap<String, Value>,
) -> String {
    let args = generate_args(model, host, port, params);
    format!("python {}", args.join(" "))
}

/// Generate Docker run arguments for TensorRT-LLM container
///
/// For Jetson devices, uses the L4T TensorRT container from NVIDIA NGC.
/// For desktop/server GPUs, uses the standard TensorRT-LLM container.
pub fn generate_docker_args(
    model: &str,
    host: &str,
    port: u16,
    params: &HashMap<String, Value>,
    is_jetson: bool,
) -> Vec<String> {
    let image = if is_jetson {
        // NVIDIA L4T TensorRT container for Jetson
        "nvcr.io/nvidia/l4t-tensorrt:r36.4.0-runtime"
    } else {
        // Standard TensorRT-LLM container
        "nvcr.io/nvidia/tritonserver:24.12-trtllm-python-py3"
    };

    let mut args = vec![
        "run".to_string(),
        "-d".to_string(),
        "--gpus".to_string(),
        "all".to_string(),
        "--network".to_string(),
        "host".to_string(),
        "--ipc".to_string(),
        "host".to_string(),
        "--shm-size".to_string(),
        "4g".to_string(),
        "-v".to_string(),
        format!(
            "{}/.cache/huggingface:/root/.cache/huggingface",
            std::env::var("HOME").unwrap_or_else(|_| "/root".to_string())
        ),
        image.to_string(),
    ];

    // Add the serve command arguments
    args.extend(generate_args(model, host, port, params));

    args
}

/// Get the default port for TensorRT-LLM
pub const fn default_port() -> u16 {
    8000
}

/// Generate the endpoint URL for a TensorRT-LLM instance
pub fn endpoint_url(host: &str, port: u16) -> String {
    format!("http://{}:{}/v1", host, port)
}

/// Common TensorRT-LLM parameters with their default values
pub fn default_params() -> HashMap<String, Value> {
    use serde_json::Number;
    let mut params = HashMap::new();
    params.insert("max_batch_size".to_string(), Value::Number(Number::from(8)));
    params.insert(
        "max_input_len".to_string(),
        Value::Number(Number::from(2048)),
    );
    params.insert(
        "max_output_len".to_string(),
        Value::Number(Number::from(512)),
    );
    params.insert("max_beam_width".to_string(), Value::Number(Number::from(1)));
    params
}

/// Default parameters optimized for Jetson Orin Nano (8GB)
pub fn jetson_nano_params() -> HashMap<String, Value> {
    use serde_json::Number;
    let mut params = HashMap::new();
    // Reduced batch size for memory constraints
    params.insert("max_batch_size".to_string(), Value::Number(Number::from(4)));
    // Shorter context for memory efficiency
    params.insert(
        "max_input_len".to_string(),
        Value::Number(Number::from(1024)),
    );
    params.insert(
        "max_output_len".to_string(),
        Value::Number(Number::from(256)),
    );
    params.insert("max_beam_width".to_string(), Value::Number(Number::from(1)));
    // INT4 quantization for memory efficiency
    params.insert(
        "quantization".to_string(),
        Value::String("int4_awq".to_string()),
    );
    // Use most of available memory for KV cache
    params.insert(
        "kv_cache_free_gpu_mem_fraction".to_string(),
        Value::Number(Number::from_f64(0.85).unwrap()),
    );
    params
}

/// Default parameters optimized for Jetson Orin NX (16GB)
pub fn jetson_nx_params() -> HashMap<String, Value> {
    use serde_json::Number;
    let mut params = HashMap::new();
    params.insert("max_batch_size".to_string(), Value::Number(Number::from(8)));
    params.insert(
        "max_input_len".to_string(),
        Value::Number(Number::from(2048)),
    );
    params.insert(
        "max_output_len".to_string(),
        Value::Number(Number::from(512)),
    );
    params.insert("max_beam_width".to_string(), Value::Number(Number::from(1)));
    // INT8 quantization as a balance
    params.insert(
        "quantization".to_string(),
        Value::String("int8".to_string()),
    );
    params.insert(
        "kv_cache_free_gpu_mem_fraction".to_string(),
        Value::Number(Number::from_f64(0.80).unwrap()),
    );
    params
}

/// Recommended maximum model sizes by device
pub struct DeviceModelLimits {
    pub max_params_billions: f32,
    pub recommended_quantization: &'static str,
    pub max_context_length: u32,
}

/// Get model limits for common Jetson devices
pub fn get_device_limits(device: &str) -> Option<DeviceModelLimits> {
    match device.to_lowercase().as_str() {
        "jetson-orin-nano" | "orin-nano" => Some(DeviceModelLimits {
            max_params_billions: 7.0, // With INT4 quantization
            recommended_quantization: "int4_awq",
            max_context_length: 2048,
        }),
        "jetson-orin-nx" | "orin-nx" => Some(DeviceModelLimits {
            max_params_billions: 13.0,
            recommended_quantization: "int8",
            max_context_length: 4096,
        }),
        "jetson-agx-orin" | "agx-orin" => Some(DeviceModelLimits {
            max_params_billions: 34.0,
            recommended_quantization: "fp16",
            max_context_length: 8192,
        }),
        _ => None,
    }
}

/// Estimate memory requirements for a model
pub fn estimate_memory_gb(params_billions: f32, quantization: &str) -> f32 {
    let bytes_per_param = match quantization {
        "int4" | "int4_awq" | "int4_gptq" => 0.5,
        "int8" | "int8_sq" => 1.0,
        "fp8" => 1.0,
        "fp16" | "float16" | "bfloat16" => 2.0,
        "fp32" | "float32" => 4.0,
        _ => 2.0, // Default to FP16
    };

    // Model weights + ~20% overhead for activations/KV cache
    params_billions * bytes_per_param * 1.2
}

// ============================================================================
// I/O: System checks
// ============================================================================

/// Check if TensorRT-LLM is installed
pub fn is_tensorrt_llm_installed() -> bool {
    StdCommand::new("python")
        .args(["-c", "import tensorrt_llm"])
        .output()
        .map(|o| o.status.success())
        .unwrap_or(false)
}

/// Check if running on a Jetson device
pub fn is_jetson_device() -> bool {
    // Check for Jetson-specific file
    std::path::Path::new("/etc/nv_tegra_release").exists() || std::env::var("JETSON_FAMILY").is_ok()
}

/// Get Jetson device type from tegra release file
pub fn get_jetson_device_type() -> Option<String> {
    if !is_jetson_device() {
        return None;
    }

    // Try to read device tree model
    std::fs::read_to_string("/proc/device-tree/model")
        .ok()
        .map(|s| s.trim_matches('\0').trim().to_string())
}

/// Check if TensorRT is available
pub fn is_tensorrt_available() -> bool {
    StdCommand::new("python")
        .args(["-c", "import tensorrt"])
        .output()
        .map(|o| o.status.success())
        .unwrap_or(false)
}

/// Get available GPU memory in GB (for Jetson unified memory)
pub fn get_available_memory_gb() -> Option<f32> {
    // Try nvidia-smi first (for discrete GPUs)
    let nvidia_smi = StdCommand::new("nvidia-smi")
        .args(["--query-gpu=memory.total", "--format=csv,noheader,nounits"])
        .output()
        .ok()
        .and_then(|o| String::from_utf8(o.stdout).ok())
        .and_then(|s| s.trim().parse::<f32>().ok())
        .map(|mb| mb / 1024.0);

    if nvidia_smi.is_some() {
        return nvidia_smi;
    }

    // For Jetson, check tegrastats or meminfo
    // Jetson uses unified memory, so we check total system memory
    std::fs::read_to_string("/proc/meminfo")
        .ok()
        .and_then(|content| {
            content
                .lines()
                .find(|line| line.starts_with("MemTotal:"))
                .and_then(|line| {
                    line.split_whitespace()
                        .nth(1)
                        .and_then(|kb| kb.parse::<f64>().ok())
                        .map(|kb| (kb / 1024.0 / 1024.0) as f32)
                })
        })
}

/// Generate environment variables for TensorRT-LLM process
pub fn generate_env_vars(hf_token: Option<&str>) -> HashMap<String, String> {
    let mut env = HashMap::new();

    if let Some(token) = hf_token {
        env.insert("HF_TOKEN".to_string(), token.to_string());
        env.insert("HUGGING_FACE_HUB_TOKEN".to_string(), token.to_string());
    }

    // TensorRT-LLM specific environment variables
    env.insert("TRT_LLM_LOG_LEVEL".to_string(), "WARNING".to_string());

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
        let args = generate_args("meta-llama/Llama-3.2-3B", "0.0.0.0", 8000, &params);

        assert!(args.contains(&"-m".to_string()));
        assert!(args.contains(&"tensorrt_llm.serve".to_string()));
        assert!(args.contains(&"--model".to_string()));
        assert!(args.contains(&"meta-llama/Llama-3.2-3B".to_string()));
    }

    #[test]
    fn test_generate_args_with_params() {
        let mut params = HashMap::new();
        params.insert("max_batch_size".to_string(), Value::Number(4.into()));
        params.insert("max_input_len".to_string(), Value::Number(1024.into()));
        params.insert(
            "quantization".to_string(),
            Value::String("int4_awq".to_string()),
        );

        let args = generate_args("model", "localhost", 8000, &params);

        assert!(args.contains(&"--max-batch-size".to_string()));
        assert!(args.contains(&"4".to_string()));
        assert!(args.contains(&"--max-input-len".to_string()));
        assert!(args.contains(&"--quantization".to_string()));
        assert!(args.contains(&"int4_awq".to_string()));
    }

    #[test]
    fn test_generate_command() {
        let params = HashMap::new();
        let cmd = generate_command("llama3", "0.0.0.0", 8000, &params);

        assert!(cmd.starts_with("python -m tensorrt_llm.serve"));
        assert!(cmd.contains("--model llama3"));
    }

    #[test]
    fn test_endpoint_url() {
        assert_eq!(endpoint_url("localhost", 8000), "http://localhost:8000/v1");
    }

    #[test]
    fn test_default_port() {
        assert_eq!(default_port(), 8000);
    }

    #[test]
    fn test_default_params() {
        let params = default_params();
        assert!(params.contains_key("max_batch_size"));
        assert!(params.contains_key("max_input_len"));
        assert!(params.contains_key("max_output_len"));
    }

    #[test]
    fn test_jetson_nano_params() {
        let params = jetson_nano_params();
        assert_eq!(
            params.get("quantization"),
            Some(&Value::String("int4_awq".to_string()))
        );
        // Should have reduced batch size
        assert_eq!(params.get("max_batch_size"), Some(&Value::Number(4.into())));
    }

    #[test]
    fn test_device_limits() {
        let nano = get_device_limits("jetson-orin-nano").unwrap();
        assert_eq!(nano.max_params_billions, 7.0);
        assert_eq!(nano.recommended_quantization, "int4_awq");

        let nx = get_device_limits("orin-nx").unwrap();
        assert_eq!(nx.max_params_billions, 13.0);

        assert!(get_device_limits("unknown-device").is_none());
    }

    #[test]
    fn test_estimate_memory() {
        // 7B model with INT4 should be ~4.2GB
        let int4_mem = estimate_memory_gb(7.0, "int4_awq");
        assert!(int4_mem > 3.0 && int4_mem < 5.0);

        // 7B model with FP16 should be ~16.8GB
        let fp16_mem = estimate_memory_gb(7.0, "fp16");
        assert!(fp16_mem > 15.0 && fp16_mem < 20.0);
    }

    #[test]
    fn test_generate_env_vars() {
        let env = generate_env_vars(Some("hf_test"));
        assert_eq!(env.get("HF_TOKEN"), Some(&"hf_test".to_string()));
        assert_eq!(env.get("TRT_LLM_LOG_LEVEL"), Some(&"WARNING".to_string()));
    }
}
