//! Function definitions and execution
//!
//! This module provides reusable function definitions that can be called from hooks:
//! - REST: HTTP requests
//! - Shell: Command execution
//! - WebSocket: Real-time messaging
//! - gRPC: RPC calls

use std::collections::HashMap;
use std::sync::Arc;
use std::time::{Duration, Instant};

use serde::{Deserialize, Serialize};
use serde_json::Value;
use thiserror::Error;
use tokio::process::Command;

use super::secrets::SecretsManager;

// ============================================================================
// SBIO: Pure data structures
// ============================================================================

/// HTTP methods for REST functions
#[derive(Debug, Clone, Deserialize, Serialize, PartialEq)]
#[serde(rename_all = "UPPERCASE")]
pub enum HttpMethod {
    Get,
    Post,
    Put,
    Patch,
    Delete,
}

impl Default for HttpMethod {
    fn default() -> Self {
        Self::Get
    }
}

/// Function type configuration
#[derive(Debug, Clone, Deserialize, Serialize)]
#[serde(tag = "type", rename_all = "kebab-case")]
pub enum FunctionType {
    /// HTTP REST call
    Rest {
        #[serde(default)]
        method: HttpMethod,
        url: String,
        #[serde(default)]
        headers: HashMap<String, String>,
        #[serde(skip_serializing_if = "Option::is_none")]
        body: Option<Value>,
        #[serde(default = "default_timeout")]
        timeout: u64,
    },
    /// Shell command execution
    Shell {
        command: String,
        #[serde(default)]
        args: Vec<String>,
        #[serde(default)]
        env: HashMap<String, String>,
        #[serde(default)]
        cwd: Option<String>,
        #[serde(default = "default_timeout")]
        timeout: u64,
    },
    /// WebSocket message
    Websocket {
        url: String,
        #[serde(skip_serializing_if = "Option::is_none")]
        message: Option<Value>,
        #[serde(default)]
        headers: HashMap<String, String>,
    },
    /// gRPC call
    Grpc {
        address: String,
        service: String,
        method: String,
        #[serde(skip_serializing_if = "Option::is_none")]
        request: Option<Value>,
        #[serde(default = "default_timeout")]
        timeout: u64,
    },
}

fn default_timeout() -> u64 {
    30
}

/// Result of function execution
#[derive(Debug, Clone)]
pub struct FunctionResult {
    pub success: bool,
    pub output: Option<Value>,
    pub error: Option<String>,
    pub duration_ms: u64,
}

impl FunctionResult {
    pub fn success(output: Option<Value>, duration_ms: u64) -> Self {
        Self {
            success: true,
            output,
            error: None,
            duration_ms,
        }
    }

    pub fn failure(error: String, duration_ms: u64) -> Self {
        Self {
            success: false,
            output: None,
            error: Some(error),
            duration_ms,
        }
    }
}

/// Errors during function execution
#[derive(Error, Debug)]
pub enum FunctionError {
    #[error("REST request failed: {0}")]
    RestError(String),

    #[error("Shell command failed: {0}")]
    ShellError(String),

    #[error("WebSocket error: {0}")]
    WebsocketError(String),

    #[error("gRPC error: {0}")]
    GrpcError(String),

    #[error("Timeout after {0}s")]
    Timeout(u64),

    #[error("Function not found: {0}")]
    NotFound(String),
}

// ============================================================================
// SBIO: Pure functions (no I/O)
// ============================================================================

/// Substitute variables in a string
/// Variables: $INPUT, $OUTPUT, $NODE, $PREV_NODE, $TIMESTAMP, $REQUEST_ID
pub fn substitute_variables(template: &str, variables: &HashMap<String, Value>) -> String {
    let mut result = template.to_string();

    for (key, value) in variables {
        let pattern = format!("${}", key);
        let replacement = match value {
            Value::String(s) => s.clone(),
            Value::Null => "null".to_string(),
            other => other.to_string(),
        };
        result = result.replace(&pattern, &replacement);
    }

    result
}

/// Substitute variables in a HashMap of strings
pub fn substitute_variables_in_map(
    map: &HashMap<String, String>,
    variables: &HashMap<String, Value>,
) -> HashMap<String, String> {
    map.iter()
        .map(|(k, v)| (k.clone(), substitute_variables(v, variables)))
        .collect()
}

/// Substitute variables in a serde_json::Value recursively
pub fn substitute_variables_in_value(
    value: &Value,
    variables: &HashMap<String, Value>,
) -> Value {
    match value {
        Value::String(s) => Value::String(substitute_variables(s, variables)),
        Value::Array(arr) => {
            Value::Array(arr.iter().map(|v| substitute_variables_in_value(v, variables)).collect())
        }
        Value::Object(map) => Value::Object(
            map.iter()
                .map(|(k, v)| {
                    (
                        substitute_variables(k, variables),
                        substitute_variables_in_value(v, variables),
                    )
                })
                .collect(),
        ),
        other => other.clone(),
    }
}

// ============================================================================
// SBIO: I/O - FunctionExecutor
// ============================================================================

/// Executor for running functions with variable and secret substitution
pub struct FunctionExecutor {
    client: reqwest::Client,
    secrets: Arc<SecretsManager>,
}

impl FunctionExecutor {
    pub fn new(secrets: Arc<SecretsManager>) -> Self {
        Self {
            client: reqwest::Client::new(),
            secrets,
        }
    }

    /// Execute a function with variable substitution
    pub async fn execute(
        &self,
        config: &FunctionType,
        variables: &HashMap<String, Value>,
    ) -> Result<FunctionResult, FunctionError> {
        let start = Instant::now();

        let result = match config {
            FunctionType::Rest {
                method,
                url,
                headers,
                body,
                timeout,
            } => {
                self.execute_rest(method, url, headers, body.as_ref(), *timeout, variables)
                    .await
            }
            FunctionType::Shell {
                command,
                args,
                env,
                cwd,
                timeout,
            } => {
                self.execute_shell(command, args, env, cwd.as_deref(), *timeout, variables)
                    .await
            }
            FunctionType::Websocket {
                url,
                message,
                headers,
            } => {
                self.execute_websocket(url, message.as_ref(), headers, variables)
                    .await
            }
            FunctionType::Grpc {
                address,
                service,
                method,
                request,
                timeout,
            } => {
                self.execute_grpc(address, service, method, request.as_ref(), *timeout, variables)
                    .await
            }
        };

        let duration_ms = start.elapsed().as_millis() as u64;

        match result {
            Ok(output) => Ok(FunctionResult::success(output, duration_ms)),
            Err(e) => Ok(FunctionResult::failure(e.to_string(), duration_ms)),
        }
    }

    /// Execute REST function
    async fn execute_rest(
        &self,
        method: &HttpMethod,
        url: &str,
        headers: &HashMap<String, String>,
        body: Option<&Value>,
        timeout: u64,
        variables: &HashMap<String, Value>,
    ) -> Result<Option<Value>, FunctionError> {
        // Substitute variables and secrets
        let url = self.substitute_all(url, variables);
        let headers = self.substitute_all_map(headers, variables);

        let mut request = match method {
            HttpMethod::Get => self.client.get(&url),
            HttpMethod::Post => self.client.post(&url),
            HttpMethod::Put => self.client.put(&url),
            HttpMethod::Patch => self.client.patch(&url),
            HttpMethod::Delete => self.client.delete(&url),
        };

        // Add headers
        for (key, value) in headers {
            request = request.header(&key, &value);
        }

        // Add body if present
        if let Some(body) = body {
            let substituted = self.substitute_all_value(body, variables);
            request = request.json(&substituted);
        }

        // Set timeout
        request = request.timeout(Duration::from_secs(timeout));

        let response = request
            .send()
            .await
            .map_err(|e| FunctionError::RestError(e.to_string()))?;

        if !response.status().is_success() {
            return Err(FunctionError::RestError(format!(
                "HTTP {} {}",
                response.status().as_u16(),
                response.status().as_str()
            )));
        }

        // Try to parse response as JSON
        let body = response
            .json::<Value>()
            .await
            .ok();

        Ok(body)
    }

    /// Execute Shell function
    async fn execute_shell(
        &self,
        command: &str,
        args: &[String],
        env: &HashMap<String, String>,
        cwd: Option<&str>,
        timeout: u64,
        variables: &HashMap<String, Value>,
    ) -> Result<Option<Value>, FunctionError> {
        let command = self.substitute_all(command, variables);
        let args: Vec<String> = args
            .iter()
            .map(|a| self.substitute_all(a, variables))
            .collect();
        let env = self.substitute_all_map(env, variables);

        let mut cmd = Command::new(&command);
        cmd.args(&args);

        // Add environment variables
        for (key, value) in env {
            cmd.env(&key, &value);
        }

        // Set working directory if specified
        if let Some(dir) = cwd {
            let dir = self.substitute_all(dir, variables);
            cmd.current_dir(dir);
        }

        let output = tokio::time::timeout(Duration::from_secs(timeout), cmd.output())
            .await
            .map_err(|_| FunctionError::Timeout(timeout))?
            .map_err(|e| FunctionError::ShellError(e.to_string()))?;

        if !output.status.success() {
            let stderr = String::from_utf8_lossy(&output.stderr);
            return Err(FunctionError::ShellError(format!(
                "Exit code {}: {}",
                output.status.code().unwrap_or(-1),
                stderr
            )));
        }

        // Return stdout as JSON string value
        let stdout = String::from_utf8_lossy(&output.stdout).trim().to_string();
        if stdout.is_empty() {
            Ok(None)
        } else {
            // Try to parse as JSON, fall back to string
            match serde_json::from_str::<Value>(&stdout) {
                Ok(v) => Ok(Some(v)),
                Err(_) => Ok(Some(Value::String(stdout))),
            }
        }
    }

    /// Execute WebSocket function
    async fn execute_websocket(
        &self,
        url: &str,
        message: Option<&Value>,
        headers: &HashMap<String, String>,
        variables: &HashMap<String, Value>,
    ) -> Result<Option<Value>, FunctionError> {
        use futures::SinkExt;
        use tokio_tungstenite::{connect_async, tungstenite::Message};

        let url = self.substitute_all(url, variables);
        let _headers = self.substitute_all_map(headers, variables);

        // Connect to WebSocket
        let (mut ws_stream, _) = connect_async(&url)
            .await
            .map_err(|e| FunctionError::WebsocketError(e.to_string()))?;

        // Send message if provided
        if let Some(msg) = message {
            let substituted = self.substitute_all_value(msg, variables);
            let text = serde_json::to_string(&substituted)
                .map_err(|e| FunctionError::WebsocketError(e.to_string()))?;

            ws_stream
                .send(Message::Text(text.into()))
                .await
                .map_err(|e: tokio_tungstenite::tungstenite::Error| {
                    FunctionError::WebsocketError(e.to_string())
                })?;
        }

        // Close connection
        let _ = ws_stream.close(None).await;

        Ok(None)
    }

    /// Execute gRPC function
    /// Note: This is a placeholder - full gRPC support would require tonic and protobuf
    async fn execute_grpc(
        &self,
        address: &str,
        service: &str,
        method: &str,
        request: Option<&Value>,
        timeout: u64,
        variables: &HashMap<String, Value>,
    ) -> Result<Option<Value>, FunctionError> {
        let address = self.substitute_all(address, variables);
        let _service = self.substitute_all(service, variables);
        let _method = self.substitute_all(method, variables);
        let _request = request.map(|r| self.substitute_all_value(r, variables));
        let _timeout = timeout;

        // gRPC support would require:
        // 1. Dynamic reflection or pre-compiled protobuf descriptors
        // 2. tonic client setup
        // For now, return a placeholder error
        Err(FunctionError::GrpcError(format!(
            "gRPC support not yet implemented for {}",
            address
        )))
    }

    /// Substitute both variables and secrets in a string
    fn substitute_all(&self, template: &str, variables: &HashMap<String, Value>) -> String {
        let with_vars = substitute_variables(template, variables);
        self.secrets.substitute(&with_vars)
    }

    /// Substitute both variables and secrets in a map
    fn substitute_all_map(
        &self,
        map: &HashMap<String, String>,
        variables: &HashMap<String, Value>,
    ) -> HashMap<String, String> {
        let with_vars = substitute_variables_in_map(map, variables);
        with_vars
            .into_iter()
            .map(|(k, v)| (k, self.secrets.substitute(&v)))
            .collect()
    }

    /// Substitute both variables and secrets in a Value
    fn substitute_all_value(&self, value: &Value, variables: &HashMap<String, Value>) -> Value {
        let with_vars = substitute_variables_in_value(value, variables);
        self.secrets.substitute_value(&with_vars)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_substitute_variables_basic() {
        let mut vars = HashMap::new();
        vars.insert("NODE".to_string(), Value::String("router".to_string()));
        vars.insert("INPUT".to_string(), Value::String("hello".to_string()));

        let result = substitute_variables("Node: $NODE, Input: $INPUT", &vars);
        assert_eq!(result, "Node: router, Input: hello");
    }

    #[test]
    fn test_substitute_variables_number() {
        let mut vars = HashMap::new();
        vars.insert("COUNT".to_string(), Value::Number(42.into()));

        let result = substitute_variables("Count: $COUNT", &vars);
        assert_eq!(result, "Count: 42");
    }

    #[test]
    fn test_substitute_variables_in_value() {
        let mut vars = HashMap::new();
        vars.insert("KEY".to_string(), Value::String("value".to_string()));

        let value = serde_json::json!({
            "field": "$KEY",
            "nested": {
                "inner": "$KEY"
            }
        });

        let result = substitute_variables_in_value(&value, &vars);
        assert_eq!(result["field"], "value");
        assert_eq!(result["nested"]["inner"], "value");
    }

    #[test]
    fn test_function_type_rest_deserialize() {
        let json = r#"{
            "type": "rest",
            "method": "POST",
            "url": "https://api.example.com/log",
            "headers": {
                "Authorization": "Bearer $secrets.auth.TOKEN"
            },
            "body": {
                "message": "$INPUT"
            }
        }"#;

        let func: FunctionType = serde_json::from_str(json).unwrap();
        match func {
            FunctionType::Rest { method, url, .. } => {
                assert_eq!(method, HttpMethod::Post);
                assert_eq!(url, "https://api.example.com/log");
            }
            _ => panic!("Expected REST function"),
        }
    }

    #[test]
    fn test_function_type_shell_deserialize() {
        let json = r#"{
            "type": "shell",
            "command": "python",
            "args": ["validate.py", "--input", "$OUTPUT"],
            "timeout": 60
        }"#;

        let func: FunctionType = serde_json::from_str(json).unwrap();
        match func {
            FunctionType::Shell { command, args, timeout, .. } => {
                assert_eq!(command, "python");
                assert_eq!(args.len(), 3);
                assert_eq!(timeout, 60);
            }
            _ => panic!("Expected Shell function"),
        }
    }

    #[test]
    fn test_function_type_websocket_deserialize() {
        let json = r#"{
            "type": "websocket",
            "url": "wss://stream.example.com",
            "message": {
                "event": "log",
                "data": "$OUTPUT"
            }
        }"#;

        let func: FunctionType = serde_json::from_str(json).unwrap();
        match func {
            FunctionType::Websocket { url, message, .. } => {
                assert_eq!(url, "wss://stream.example.com");
                assert!(message.is_some());
            }
            _ => panic!("Expected WebSocket function"),
        }
    }

    #[test]
    fn test_function_type_grpc_deserialize() {
        let json = r#"{
            "type": "grpc",
            "address": "localhost:50051",
            "service": "QuotaService",
            "method": "CheckQuota",
            "request": {
                "user_id": "$USER_ID"
            }
        }"#;

        let func: FunctionType = serde_json::from_str(json).unwrap();
        match func {
            FunctionType::Grpc { address, service, method, .. } => {
                assert_eq!(address, "localhost:50051");
                assert_eq!(service, "QuotaService");
                assert_eq!(method, "CheckQuota");
            }
            _ => panic!("Expected gRPC function"),
        }
    }

    #[test]
    fn test_function_result_success() {
        let result = FunctionResult::success(Some(Value::String("ok".to_string())), 100);
        assert!(result.success);
        assert!(result.output.is_some());
        assert!(result.error.is_none());
        assert_eq!(result.duration_ms, 100);
    }

    #[test]
    fn test_function_result_failure() {
        let result = FunctionResult::failure("connection refused".to_string(), 50);
        assert!(!result.success);
        assert!(result.output.is_none());
        assert_eq!(result.error, Some("connection refused".to_string()));
    }
}
