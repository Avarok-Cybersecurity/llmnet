//! Memory and compute validation for edge device deployments
//!
//! This module provides validation hints to help users understand whether
//! their edge devices can run specific models effectively.
//!
//! # Supported Devices
//! - NVIDIA Jetson Orin Nano (8GB) - Small models with INT4 quantization
//! - NVIDIA Jetson Orin NX (16GB) - Medium models up to 13B
//! - NVIDIA Jetson AGX Orin (32/64GB) - Large models up to 34B
//! - Raspberry Pi 5 (8GB) - Very small models (1-3B) with llama.cpp

use std::collections::HashMap;

use serde::{Deserialize, Serialize};

use super::models::{ModelConfig, RunnerType};

/// Device capability profile
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct DeviceProfile {
    /// Device name for display
    pub name: String,
    /// Total available memory in GB
    pub memory_gb: f32,
    /// GPU compute capability (e.g., 8.7 for Orin)
    pub compute_capability: Option<f32>,
    /// Whether device has CUDA support
    pub cuda_support: bool,
    /// Whether device has TensorRT support
    pub tensorrt_support: bool,
    /// Maximum recommended model size in billions of parameters
    pub max_model_params_b: f32,
    /// Recommended quantization method
    pub recommended_quantization: String,
    /// Maximum context length recommendation
    pub max_context_length: u32,
}

/// Validation result with severity levels
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub enum ValidationSeverity {
    /// Informational hint
    Info,
    /// Warning - may work but suboptimal
    Warning,
    /// Error - likely to fail
    Error,
}

/// A single validation message
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ValidationMessage {
    pub severity: ValidationSeverity,
    pub code: String,
    pub message: String,
    pub suggestion: Option<String>,
}

/// Validation result containing all messages
#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct ValidationResult {
    pub messages: Vec<ValidationMessage>,
    pub passed: bool,
}

impl ValidationResult {
    pub fn new() -> Self {
        Self {
            messages: Vec::new(),
            passed: true,
        }
    }

    pub fn add(&mut self, msg: ValidationMessage) {
        if msg.severity == ValidationSeverity::Error {
            self.passed = false;
        }
        self.messages.push(msg);
    }

    pub fn info(mut self, code: &str, message: &str) -> Self {
        self.add(ValidationMessage {
            severity: ValidationSeverity::Info,
            code: code.to_string(),
            message: message.to_string(),
            suggestion: None,
        });
        self
    }

    pub fn warning(mut self, code: &str, message: &str, suggestion: Option<&str>) -> Self {
        self.add(ValidationMessage {
            severity: ValidationSeverity::Warning,
            code: code.to_string(),
            message: message.to_string(),
            suggestion: suggestion.map(String::from),
        });
        self
    }

    pub fn error(mut self, code: &str, message: &str, suggestion: Option<&str>) -> Self {
        self.add(ValidationMessage {
            severity: ValidationSeverity::Error,
            code: code.to_string(),
            message: message.to_string(),
            suggestion: suggestion.map(String::from),
        });
        self
    }

    pub fn has_errors(&self) -> bool {
        self.messages
            .iter()
            .any(|m| m.severity == ValidationSeverity::Error)
    }

    pub fn has_warnings(&self) -> bool {
        self.messages
            .iter()
            .any(|m| m.severity == ValidationSeverity::Warning)
    }
}

// ============================================================================
// SBIO: Pure validation logic (no I/O)
// ============================================================================

/// Known device profiles
pub fn known_devices() -> HashMap<String, DeviceProfile> {
    let mut devices = HashMap::new();

    devices.insert(
        "jetson-orin-nano".to_string(),
        DeviceProfile {
            name: "NVIDIA Jetson Orin Nano".to_string(),
            memory_gb: 8.0,
            compute_capability: Some(8.7),
            cuda_support: true,
            tensorrt_support: true,
            max_model_params_b: 7.0,
            recommended_quantization: "int4_awq".to_string(),
            max_context_length: 2048,
        },
    );

    devices.insert(
        "jetson-orin-nx".to_string(),
        DeviceProfile {
            name: "NVIDIA Jetson Orin NX".to_string(),
            memory_gb: 16.0,
            compute_capability: Some(8.7),
            cuda_support: true,
            tensorrt_support: true,
            max_model_params_b: 13.0,
            recommended_quantization: "int8".to_string(),
            max_context_length: 4096,
        },
    );

    devices.insert(
        "jetson-agx-orin".to_string(),
        DeviceProfile {
            name: "NVIDIA Jetson AGX Orin".to_string(),
            memory_gb: 64.0,
            compute_capability: Some(8.7),
            cuda_support: true,
            tensorrt_support: true,
            max_model_params_b: 34.0,
            recommended_quantization: "fp16".to_string(),
            max_context_length: 8192,
        },
    );

    devices.insert(
        "raspberry-pi-5".to_string(),
        DeviceProfile {
            name: "Raspberry Pi 5".to_string(),
            memory_gb: 8.0,
            compute_capability: None,
            cuda_support: false,
            tensorrt_support: false,
            max_model_params_b: 3.0,
            recommended_quantization: "q4_k_m".to_string(),
            max_context_length: 2048,
        },
    );

    devices
}

/// Estimate model size from source name (heuristic)
pub fn estimate_model_size(source: &str) -> Option<f32> {
    let source_lower = source.to_lowercase();

    // Common patterns: "7b", "13b", "70b", "3b", etc.
    let patterns = [
        ("405b", 405.0),
        ("180b", 180.0),
        ("70b", 70.0),
        ("65b", 65.0),
        ("34b", 34.0),
        ("33b", 33.0),
        ("32b", 32.0),
        ("30b", 30.0),
        ("27b", 27.0),
        ("14b", 14.0),
        ("13b", 13.0),
        ("11b", 11.0),
        ("9b", 9.0),
        ("8b", 8.0),
        ("7b", 7.0),
        ("6b", 6.0),
        ("3b", 3.0),
        ("2b", 2.0),
        ("1.5b", 1.5),
        ("1b", 1.0),
        ("0.5b", 0.5),
        ("500m", 0.5),
    ];

    for (pattern, size) in patterns {
        if source_lower.contains(pattern) {
            return Some(size);
        }
    }

    None
}

/// Estimate memory requirement based on model size and quantization
pub fn estimate_memory_requirement(params_billions: f32, quantization: &str) -> f32 {
    let bytes_per_param = match quantization.to_lowercase().as_str() {
        "int4" | "int4_awq" | "int4_gptq" | "q4_0" | "q4_k_m" | "q4_k_s" => 0.5,
        "int8" | "int8_sq" | "q8_0" => 1.0,
        "fp8" => 1.0,
        "fp16" | "float16" | "bfloat16" | "q6_k" => 2.0,
        "fp32" | "float32" => 4.0,
        _ => 2.0, // Default to FP16
    };

    // Model weights + overhead for KV cache and activations (~30%)
    params_billions * bytes_per_param * 1.3
}

/// Get quantization from model config parameters
pub fn get_quantization(config: &ModelConfig) -> String {
    config
        .parameters
        .get("quantization")
        .and_then(|v| v.as_str())
        .map(String::from)
        .unwrap_or_else(|| "fp16".to_string())
}

/// Validate a model configuration against a device profile
pub fn validate_model_for_device(config: &ModelConfig, device: &DeviceProfile) -> ValidationResult {
    let mut result = ValidationResult::new();

    // Check runner compatibility
    match config.runner {
        RunnerType::TensorRtLlm if !device.tensorrt_support => {
            result = result.error(
                "RUNNER_UNSUPPORTED",
                &format!("TensorRT-LLM is not supported on {}", device.name),
                Some("Use llama-cpp runner instead for CPU-based inference"),
            );
        }
        RunnerType::Vllm if !device.cuda_support => {
            result = result.error(
                "RUNNER_UNSUPPORTED",
                &format!(
                    "vLLM requires CUDA which is not available on {}",
                    device.name
                ),
                Some("Use llama-cpp runner for CPU-based inference"),
            );
        }
        _ => {}
    }

    // Estimate model size if source is provided
    if let Some(source) = &config.source {
        if let Some(model_size) = estimate_model_size(source) {
            let quantization = get_quantization(config);
            let memory_required = estimate_memory_requirement(model_size, &quantization);

            // Check model size against device limits
            if model_size > device.max_model_params_b {
                result = result.error(
                    "MODEL_TOO_LARGE",
                    &format!(
                        "Model ({:.1}B params) exceeds device limit ({:.1}B)",
                        model_size, device.max_model_params_b
                    ),
                    Some(&format!(
                        "Use a smaller model or apply {} quantization",
                        device.recommended_quantization
                    )),
                );
            } else if model_size > device.max_model_params_b * 0.8 {
                result = result.warning(
                    "MODEL_NEAR_LIMIT",
                    &format!(
                        "Model ({:.1}B params) is close to device limit ({:.1}B)",
                        model_size, device.max_model_params_b
                    ),
                    Some("Consider reducing batch size or context length"),
                );
            }

            // Check memory requirements
            if memory_required > device.memory_gb {
                result = result.error(
                    "INSUFFICIENT_MEMORY",
                    &format!(
                        "Estimated memory requirement ({:.1}GB) exceeds device memory ({:.1}GB)",
                        memory_required, device.memory_gb
                    ),
                    Some(&format!(
                        "Use {} quantization to reduce memory usage",
                        device.recommended_quantization
                    )),
                );
            } else if memory_required > device.memory_gb * 0.85 {
                result = result.warning(
                    "MEMORY_PRESSURE",
                    &format!(
                        "Model will use {:.0}% of available memory",
                        (memory_required / device.memory_gb) * 100.0
                    ),
                    Some("Reduce batch size or context length for stable operation"),
                );
            }

            // Quantization recommendations
            if quantization != device.recommended_quantization {
                let quant_bytes = match quantization.to_lowercase().as_str() {
                    "int4" | "int4_awq" | "q4_0" | "q4_k_m" => 0.5,
                    "int8" | "q8_0" => 1.0,
                    "fp16" | "bfloat16" => 2.0,
                    _ => 2.0,
                };
                let rec_bytes = match device.recommended_quantization.as_str() {
                    "int4_awq" | "q4_k_m" => 0.5,
                    "int8" => 1.0,
                    "fp16" => 2.0,
                    _ => 2.0,
                };

                if quant_bytes > rec_bytes && memory_required > device.memory_gb * 0.7 {
                    result = result.warning(
                        "SUBOPTIMAL_QUANTIZATION",
                        &format!(
                            "Using {} quantization; {} recommended for {}",
                            quantization, device.recommended_quantization, device.name
                        ),
                        Some(&format!(
                            "Consider {} for better memory efficiency",
                            device.recommended_quantization
                        )),
                    );
                }
            }
        } else {
            result = result.info(
                "MODEL_SIZE_UNKNOWN",
                "Could not estimate model size from source name",
            );
        }
    }

    // Check context length
    if let Some(max_input) = config.parameters.get("max_input_len") {
        if let Some(ctx) = max_input.as_u64() {
            if ctx as u32 > device.max_context_length {
                result = result.warning(
                    "CONTEXT_TOO_LONG",
                    &format!(
                        "Context length {} exceeds recommended {} for {}",
                        ctx, device.max_context_length, device.name
                    ),
                    Some(&format!(
                        "Reduce max_input_len to {} or lower",
                        device.max_context_length
                    )),
                );
            }
        }
    }

    // Check batch size for constrained devices
    if device.memory_gb <= 8.0 {
        if let Some(batch) = config.parameters.get("max_batch_size") {
            if let Some(b) = batch.as_u64() {
                if b > 4 {
                    result = result.warning(
                        "BATCH_SIZE_HIGH",
                        &format!(
                            "Batch size {} may cause memory issues on {}",
                            b, device.name
                        ),
                        Some("Consider reducing max_batch_size to 4 or lower"),
                    );
                }
            }
        }
    }

    result
}

/// Validate all models in a configuration against available device profiles
pub fn validate_models(
    models: &HashMap<String, ModelConfig>,
    device_name: Option<&str>,
) -> HashMap<String, ValidationResult> {
    let devices = known_devices();
    let device = device_name
        .and_then(|name| devices.get(name))
        .or_else(|| detect_device_profile(&devices));

    let mut results = HashMap::new();

    for (name, config) in models {
        let result = if let Some(dev) = device {
            validate_model_for_device(config, dev)
        } else {
            ValidationResult::new().info(
                "NO_DEVICE_PROFILE",
                "No device profile available for validation",
            )
        };
        results.insert(name.clone(), result);
    }

    results
}

/// Try to detect the current device profile
fn detect_device_profile(devices: &HashMap<String, DeviceProfile>) -> Option<&DeviceProfile> {
    // Check for Jetson
    if std::path::Path::new("/etc/nv_tegra_release").exists() {
        // Read device tree model to determine specific Jetson
        if let Ok(model) = std::fs::read_to_string("/proc/device-tree/model") {
            let model = model.to_lowercase();
            if model.contains("orin nano") {
                return devices.get("jetson-orin-nano");
            } else if model.contains("orin nx") {
                return devices.get("jetson-orin-nx");
            } else if model.contains("agx orin") {
                return devices.get("jetson-agx-orin");
            }
        }
    }

    // Check for Raspberry Pi
    if std::path::Path::new("/proc/device-tree/model").exists() {
        if let Ok(model) = std::fs::read_to_string("/proc/device-tree/model") {
            if model.contains("Raspberry Pi 5") {
                return devices.get("raspberry-pi-5");
            }
        }
    }

    None
}

/// Format validation results for display
pub fn format_validation_results(results: &HashMap<String, ValidationResult>) -> String {
    let mut output = String::new();

    for (model_name, result) in results {
        if result.messages.is_empty() {
            continue;
        }

        output.push_str(&format!("\n[{}]\n", model_name));

        for msg in &result.messages {
            let prefix = match msg.severity {
                ValidationSeverity::Info => "INFO",
                ValidationSeverity::Warning => "WARN",
                ValidationSeverity::Error => "ERROR",
            };

            output.push_str(&format!("  {} [{}]: {}\n", prefix, msg.code, msg.message));

            if let Some(suggestion) = &msg.suggestion {
                output.push_str(&format!("    -> {}\n", suggestion));
            }
        }
    }

    output
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::Value;

    #[test]
    fn test_estimate_model_size() {
        assert_eq!(estimate_model_size("meta-llama/Llama-3.2-7B"), Some(7.0));
        assert_eq!(estimate_model_size("Qwen2.5-3B-Instruct"), Some(3.0));
        assert_eq!(estimate_model_size("mistral-70b"), Some(70.0));
        assert_eq!(estimate_model_size("phi-3-mini"), None);
    }

    #[test]
    fn test_estimate_memory_requirement() {
        // 7B with INT4 should be ~4.55GB (7 * 0.5 * 1.3)
        let mem = estimate_memory_requirement(7.0, "int4_awq");
        assert!(mem > 4.0 && mem < 5.0);

        // 7B with FP16 should be ~18.2GB (7 * 2 * 1.3)
        let mem = estimate_memory_requirement(7.0, "fp16");
        assert!(mem > 17.0 && mem < 20.0);
    }

    #[test]
    fn test_known_devices() {
        let devices = known_devices();
        assert!(devices.contains_key("jetson-orin-nano"));
        assert!(devices.contains_key("raspberry-pi-5"));

        let nano = &devices["jetson-orin-nano"];
        assert_eq!(nano.memory_gb, 8.0);
        assert!(nano.tensorrt_support);
    }

    #[test]
    fn test_validate_model_too_large() {
        let config = ModelConfig::tensorrt_llm("meta-llama/Llama-3.1-70B");
        let device = &known_devices()["jetson-orin-nano"];

        let result = validate_model_for_device(&config, device);

        assert!(result.has_errors());
        assert!(result.messages.iter().any(|m| m.code == "MODEL_TOO_LARGE"));
    }

    #[test]
    fn test_validate_model_fits() {
        let config = ModelConfig::tensorrt_llm("meta-llama/Llama-3.2-3B").with_parameter(
            "quantization".to_string(),
            Value::String("int4_awq".to_string()),
        );
        let device = &known_devices()["jetson-orin-nano"];

        let result = validate_model_for_device(&config, device);

        assert!(!result.has_errors());
    }

    #[test]
    fn test_validate_runner_incompatible() {
        let config = ModelConfig::tensorrt_llm("llama-3b");
        let device = &known_devices()["raspberry-pi-5"];

        let result = validate_model_for_device(&config, device);

        assert!(result.has_errors());
        assert!(result
            .messages
            .iter()
            .any(|m| m.code == "RUNNER_UNSUPPORTED"));
    }

    #[test]
    fn test_validation_result_builder() {
        let result = ValidationResult::new()
            .info("TEST_INFO", "Info message")
            .warning("TEST_WARN", "Warning message", Some("Do this"))
            .error("TEST_ERR", "Error message", None);

        assert_eq!(result.messages.len(), 3);
        assert!(result.has_errors());
        assert!(result.has_warnings());
        assert!(!result.passed);
    }

    #[test]
    fn test_format_validation_results() {
        let mut results = HashMap::new();
        results.insert(
            "test-model".to_string(),
            ValidationResult::new().warning(
                "TEST_WARN",
                "This is a warning",
                Some("Fix it this way"),
            ),
        );

        let output = format_validation_results(&results);
        assert!(output.contains("[test-model]"));
        assert!(output.contains("WARN"));
        assert!(output.contains("Fix it this way"));
    }
}
