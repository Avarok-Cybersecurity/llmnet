//! Ollama Modelfile parsing and generation
//!
//! This module provides functionality to parse, generate, and merge Ollama
//! Modelfile configurations. Supports FROM, PARAMETER, SYSTEM, TEMPLATE,
//! and ADAPTER directives.

use std::collections::HashMap;

use serde_json::Value;
use thiserror::Error;

/// Errors that can occur during Modelfile operations
#[derive(Error, Debug)]
pub enum ModelfileError {
    #[error("Missing required FROM directive")]
    MissingFrom,

    #[error("Invalid directive: {0}")]
    InvalidDirective(String),

    #[error("Parse error at line {0}: {1}")]
    ParseError(usize, String),
}

/// Ollama Modelfile structure
#[derive(Debug, Clone, Default)]
pub struct Modelfile {
    /// Base model (FROM directive)
    pub from: String,
    /// Key-value parameters (PARAMETER directive)
    pub parameters: Vec<(String, String)>,
    /// System prompt (SYSTEM directive)
    pub system: Option<String>,
    /// Prompt template (TEMPLATE directive)
    pub template: Option<String>,
    /// LoRA adapter paths (ADAPTER directive)
    pub adapters: Vec<String>,
    /// License information (LICENSE directive)
    pub license: Option<String>,
    /// Message examples (MESSAGE directive)
    pub messages: Vec<(String, String)>,
}

// ============================================================================
// SBIO: Pure business logic (no I/O)
// ============================================================================

/// Parse a Modelfile from string content
///
/// Supports directives: FROM, PARAMETER, SYSTEM, TEMPLATE, ADAPTER, LICENSE, MESSAGE
pub fn parse_modelfile(content: &str) -> Result<Modelfile, ModelfileError> {
    let mut modelfile = Modelfile::default();
    let mut current_directive: Option<&str> = None;
    let mut multiline_buffer = String::new();

    for (line_num, line) in content.lines().enumerate() {
        let trimmed = line.trim();

        // Skip empty lines and comments
        if trimmed.is_empty() || trimmed.starts_with('#') {
            continue;
        }

        // Check for multiline continuation (heredoc-style)
        if let Some(directive) = current_directive {
            if trimmed == "\"\"\"" {
                // End of multiline
                match directive {
                    "SYSTEM" => modelfile.system = Some(multiline_buffer.trim().to_string()),
                    "TEMPLATE" => modelfile.template = Some(multiline_buffer.trim().to_string()),
                    "LICENSE" => modelfile.license = Some(multiline_buffer.trim().to_string()),
                    _ => {}
                }
                current_directive = None;
                multiline_buffer.clear();
                continue;
            } else {
                multiline_buffer.push_str(line);
                multiline_buffer.push('\n');
                continue;
            }
        }

        // Parse directive
        let parts: Vec<&str> = trimmed.splitn(2, char::is_whitespace).collect();
        let directive = parts[0].to_uppercase();
        let value = parts.get(1).map(|s| s.trim()).unwrap_or("");

        match directive.as_str() {
            "FROM" => {
                if value.is_empty() {
                    return Err(ModelfileError::ParseError(
                        line_num + 1,
                        "FROM requires a model name".to_string(),
                    ));
                }
                modelfile.from = value.to_string();
            }
            "PARAMETER" => {
                let param_parts: Vec<&str> = value.splitn(2, char::is_whitespace).collect();
                if param_parts.len() < 2 {
                    return Err(ModelfileError::ParseError(
                        line_num + 1,
                        "PARAMETER requires key and value".to_string(),
                    ));
                }
                modelfile
                    .parameters
                    .push((param_parts[0].to_string(), param_parts[1].to_string()));
            }
            "SYSTEM" => {
                if value.starts_with("\"\"\"") {
                    // Start multiline
                    current_directive = Some("SYSTEM");
                    multiline_buffer.clear();
                    let after_quotes = value.trim_start_matches("\"\"\"");
                    if !after_quotes.is_empty() {
                        multiline_buffer.push_str(after_quotes);
                        multiline_buffer.push('\n');
                    }
                } else {
                    modelfile.system = Some(value.to_string());
                }
            }
            "TEMPLATE" => {
                if value.starts_with("\"\"\"") {
                    current_directive = Some("TEMPLATE");
                    multiline_buffer.clear();
                    let after_quotes = value.trim_start_matches("\"\"\"");
                    if !after_quotes.is_empty() {
                        multiline_buffer.push_str(after_quotes);
                        multiline_buffer.push('\n');
                    }
                } else {
                    modelfile.template = Some(value.to_string());
                }
            }
            "ADAPTER" => {
                if !value.is_empty() {
                    modelfile.adapters.push(value.to_string());
                }
            }
            "LICENSE" => {
                if value.starts_with("\"\"\"") {
                    current_directive = Some("LICENSE");
                    multiline_buffer.clear();
                } else {
                    modelfile.license = Some(value.to_string());
                }
            }
            "MESSAGE" => {
                let msg_parts: Vec<&str> = value.splitn(2, char::is_whitespace).collect();
                if msg_parts.len() >= 2 {
                    modelfile
                        .messages
                        .push((msg_parts[0].to_string(), msg_parts[1].to_string()));
                }
            }
            _ => {
                // Unknown directives are ignored for forward compatibility
            }
        }
    }

    if modelfile.from.is_empty() {
        return Err(ModelfileError::MissingFrom);
    }

    Ok(modelfile)
}

/// Generate a Modelfile string from the structure
pub fn generate_modelfile(modelfile: &Modelfile) -> String {
    let mut output = String::new();

    // FROM is required
    output.push_str(&format!("FROM {}\n", modelfile.from));

    // Parameters
    for (key, value) in &modelfile.parameters {
        output.push_str(&format!("PARAMETER {} {}\n", key, value));
    }

    // System prompt
    if let Some(system) = &modelfile.system {
        if system.contains('\n') {
            output.push_str(&format!("SYSTEM \"\"\"\n{}\n\"\"\"\n", system));
        } else {
            output.push_str(&format!("SYSTEM {}\n", system));
        }
    }

    // Template
    if let Some(template) = &modelfile.template {
        if template.contains('\n') {
            output.push_str(&format!("TEMPLATE \"\"\"\n{}\n\"\"\"\n", template));
        } else {
            output.push_str(&format!("TEMPLATE {}\n", template));
        }
    }

    // Adapters
    for adapter in &modelfile.adapters {
        output.push_str(&format!("ADAPTER {}\n", adapter));
    }

    // License
    if let Some(license) = &modelfile.license {
        if license.contains('\n') {
            output.push_str(&format!("LICENSE \"\"\"\n{}\n\"\"\"\n", license));
        } else {
            output.push_str(&format!("LICENSE {}\n", license));
        }
    }

    // Messages
    for (role, content) in &modelfile.messages {
        output.push_str(&format!("MESSAGE {} {}\n", role, content));
    }

    output
}

/// Create a Modelfile from just a model name
pub fn create_modelfile(model: &str) -> Modelfile {
    Modelfile {
        from: model.to_string(),
        ..Default::default()
    }
}

/// Merge user parameters into an existing Modelfile
///
/// Supported parameter types:
/// - `temperature`, `top_p`, `top_k`: Sampling parameters
/// - `num_ctx`, `num_predict`, `num_gpu`, `num_thread`: Resource limits
/// - `repeat_penalty`, `repeat_last_n`: Repetition control
/// - `stop`: Stop sequences
/// - `system`: System prompt (overrides SYSTEM directive)
pub fn merge_parameters(mut base: Modelfile, params: &HashMap<String, Value>) -> Modelfile {
    for (key, value) in params {
        match key.as_str() {
            "system" => {
                if let Some(s) = value.as_str() {
                    base.system = Some(s.to_string());
                }
            }
            "template" => {
                if let Some(s) = value.as_str() {
                    base.template = Some(s.to_string());
                }
            }
            _ => {
                // Add as PARAMETER directive
                let value_str = match value {
                    Value::Bool(b) => b.to_string(),
                    Value::Number(n) => n.to_string(),
                    Value::String(s) => s.clone(),
                    Value::Array(arr) => {
                        // For stop sequences, format as JSON array
                        serde_json::to_string(arr).unwrap_or_default()
                    }
                    _ => continue,
                };

                // Remove existing parameter with same key
                base.parameters.retain(|(k, _)| k != key);
                base.parameters.push((key.clone(), value_str));
            }
        }
    }

    base
}

/// Get the default port for Ollama
pub const fn default_port() -> u16 {
    11434
}

/// Generate the endpoint URL for an Ollama instance
pub fn endpoint_url(host: &str, port: u16) -> String {
    format!("http://{}:{}/v1", host, port)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_parse_simple_modelfile() {
        let content = r#"
FROM tinyllama:1.1b

PARAMETER temperature 0.7
PARAMETER num_ctx 2048

SYSTEM You are a helpful assistant.
"#;

        let modelfile = parse_modelfile(content).unwrap();
        assert_eq!(modelfile.from, "tinyllama:1.1b");
        assert_eq!(modelfile.parameters.len(), 2);
        assert_eq!(
            modelfile.parameters[0],
            ("temperature".to_string(), "0.7".to_string())
        );
        assert_eq!(
            modelfile.parameters[1],
            ("num_ctx".to_string(), "2048".to_string())
        );
        assert_eq!(
            modelfile.system,
            Some("You are a helpful assistant.".to_string())
        );
    }

    #[test]
    fn test_parse_multiline_system() {
        let content = r#"
FROM llama2

SYSTEM """
You are a helpful assistant.
You always respond in a friendly manner.
"""
"#;

        let modelfile = parse_modelfile(content).unwrap();
        assert_eq!(modelfile.from, "llama2");
        assert!(modelfile.system.unwrap().contains("friendly manner"));
    }

    #[test]
    fn test_parse_with_adapters() {
        let content = r#"
FROM llama2

ADAPTER /path/to/lora1.gguf
ADAPTER /path/to/lora2.gguf
"#;

        let modelfile = parse_modelfile(content).unwrap();
        assert_eq!(modelfile.adapters.len(), 2);
    }

    #[test]
    fn test_parse_missing_from() {
        let content = "PARAMETER temperature 0.7";
        let result = parse_modelfile(content);
        assert!(matches!(result, Err(ModelfileError::MissingFrom)));
    }

    #[test]
    fn test_generate_modelfile() {
        let modelfile = Modelfile {
            from: "tinyllama:1.1b".to_string(),
            parameters: vec![
                ("temperature".to_string(), "0.7".to_string()),
                ("num_ctx".to_string(), "2048".to_string()),
            ],
            system: Some("You are helpful.".to_string()),
            template: None,
            adapters: vec![],
            license: None,
            messages: vec![],
        };

        let output = generate_modelfile(&modelfile);
        assert!(output.contains("FROM tinyllama:1.1b"));
        assert!(output.contains("PARAMETER temperature 0.7"));
        assert!(output.contains("PARAMETER num_ctx 2048"));
        assert!(output.contains("SYSTEM You are helpful."));
    }

    #[test]
    fn test_merge_parameters() {
        let base = Modelfile {
            from: "llama2".to_string(),
            parameters: vec![("temperature".to_string(), "0.5".to_string())],
            ..Default::default()
        };

        let mut params = HashMap::new();
        params.insert(
            "temperature".to_string(),
            Value::Number(serde_json::Number::from_f64(0.9).unwrap()),
        );
        params.insert("num_ctx".to_string(), Value::Number(4096.into()));
        params.insert(
            "system".to_string(),
            Value::String("New system prompt".to_string()),
        );

        let merged = merge_parameters(base, &params);

        // temperature should be overwritten
        assert!(merged
            .parameters
            .iter()
            .any(|(k, v)| k == "temperature" && v == "0.9"));
        // num_ctx should be added
        assert!(merged
            .parameters
            .iter()
            .any(|(k, v)| k == "num_ctx" && v == "4096"));
        // system should be set
        assert_eq!(merged.system, Some("New system prompt".to_string()));
    }

    #[test]
    fn test_create_modelfile() {
        let modelfile = create_modelfile("tinyllama:1.1b");
        assert_eq!(modelfile.from, "tinyllama:1.1b");
        assert!(modelfile.parameters.is_empty());
    }

    #[test]
    fn test_roundtrip() {
        let original = Modelfile {
            from: "llama2".to_string(),
            parameters: vec![("temperature".to_string(), "0.7".to_string())],
            system: Some("Be helpful".to_string()),
            template: None,
            adapters: vec!["/path/to/adapter.gguf".to_string()],
            license: None,
            messages: vec![],
        };

        let generated = generate_modelfile(&original);
        let parsed = parse_modelfile(&generated).unwrap();

        assert_eq!(parsed.from, original.from);
        assert_eq!(parsed.parameters, original.parameters);
        assert_eq!(parsed.system, original.system);
        assert_eq!(parsed.adapters, original.adapters);
    }

    #[test]
    fn test_endpoint_url() {
        assert_eq!(
            endpoint_url("localhost", 11434),
            "http://localhost:11434/v1"
        );
        assert_eq!(
            endpoint_url("192.168.1.100", 8080),
            "http://192.168.1.100:8080/v1"
        );
    }
}
