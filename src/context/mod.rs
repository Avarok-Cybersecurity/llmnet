use std::collections::HashMap;
use std::path::PathBuf;

use serde::{Deserialize, Serialize};
use thiserror::Error;

/// Default control plane port for LLMNet clusters
pub const DEFAULT_CONTROL_PLANE_PORT: u16 = 8181;

/// Default worker node port
pub const DEFAULT_WORKER_PORT: u16 = 8080;

/// Default config file location: ~/.llmnet/config
pub fn default_config_path() -> PathBuf {
    dirs::home_dir()
        .unwrap_or_else(|| PathBuf::from("."))
        .join(".llmnet")
        .join("config")
}

/// Errors that can occur during context operations
#[derive(Error, Debug)]
pub enum ContextError {
    #[error("Config file not found: {0}")]
    ConfigNotFound(PathBuf),

    #[error("Context '{0}' not found")]
    ContextNotFound(String),

    #[error("No current context set")]
    NoCurrentContext,

    #[error("Failed to parse config: {0}")]
    ParseError(String),

    #[error("Failed to write config: {0}")]
    WriteError(String),

    #[error("IO error: {0}")]
    IoError(#[from] std::io::Error),

    #[error("Connection failed to {0}: {1}")]
    ConnectionFailed(String, String),
}

/// A single context representing a remote LLMNet cluster
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Context {
    /// Display name for this context
    pub name: String,
    /// URL of the control plane (e.g., "http://192.168.1.100:8181")
    pub url: String,
    /// Optional API key for authentication
    #[serde(skip_serializing_if = "Option::is_none")]
    pub api_key: Option<String>,
    /// Optional description
    #[serde(skip_serializing_if = "Option::is_none")]
    pub description: Option<String>,
}

/// The complete configuration file structure
#[derive(Debug, Clone, Serialize, Deserialize, Default)]
pub struct Config {
    /// Currently active context name
    #[serde(rename = "current-context")]
    #[serde(skip_serializing_if = "Option::is_none")]
    pub current_context: Option<String>,

    /// Map of context name to context definition
    #[serde(default)]
    pub contexts: HashMap<String, Context>,

    /// Local cluster configuration
    #[serde(default)]
    pub local: LocalConfig,
}

/// Local cluster configuration
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct LocalConfig {
    /// Default port for the local control plane
    #[serde(default = "default_control_plane_port")]
    pub port: u16,
    /// Default bind address
    #[serde(default = "default_bind_address")]
    pub bind_addr: String,
}

impl Default for LocalConfig {
    fn default() -> Self {
        Self {
            port: DEFAULT_CONTROL_PLANE_PORT,
            bind_addr: "0.0.0.0".to_string(),
        }
    }
}

fn default_control_plane_port() -> u16 {
    DEFAULT_CONTROL_PLANE_PORT
}

fn default_bind_address() -> String {
    "0.0.0.0".to_string()
}

// ============================================================================
// SBIO: Pure business logic (no I/O)
// ============================================================================

/// Parse config from YAML string
pub fn parse_config(content: &str) -> Result<Config, ContextError> {
    serde_yaml::from_str(content).map_err(|e| ContextError::ParseError(e.to_string()))
}

/// Serialize config to YAML string
pub fn serialize_config(config: &Config) -> Result<String, ContextError> {
    serde_yaml::to_string(config).map_err(|e| ContextError::WriteError(e.to_string()))
}

/// Add or update a context in the config
pub fn add_context(config: &mut Config, context: Context) {
    config.contexts.insert(context.name.clone(), context);
}

/// Remove a context from the config
pub fn remove_context(config: &mut Config, name: &str) -> Option<Context> {
    let removed = config.contexts.remove(name);
    // Clear current context if it was the removed one
    if config.current_context.as_deref() == Some(name) {
        config.current_context = None;
    }
    removed
}

/// Set the current context
pub fn set_current_context(config: &mut Config, name: &str) -> Result<(), ContextError> {
    // Allow built-in contexts: "local" (control plane) and "worker" (worker node)
    let is_builtin = name == "local" || name == "worker";
    if !config.contexts.contains_key(name) && !is_builtin {
        return Err(ContextError::ContextNotFound(name.to_string()));
    }
    config.current_context = Some(name.to_string());
    Ok(())
}

/// Get the current context
pub fn get_current_context(config: &Config) -> Result<&str, ContextError> {
    config
        .current_context
        .as_deref()
        .ok_or(ContextError::NoCurrentContext)
}

/// Get a context by name
pub fn get_context<'a>(config: &'a Config, name: &str) -> Result<&'a Context, ContextError> {
    config
        .contexts
        .get(name)
        .ok_or_else(|| ContextError::ContextNotFound(name.to_string()))
}

/// List all context names
pub fn list_contexts(config: &Config) -> Vec<&str> {
    config.contexts.keys().map(|s| s.as_str()).collect()
}

// ============================================================================
// I/O boundary functions
// ============================================================================

/// Load config from the default location
pub fn load_config() -> Result<Config, ContextError> {
    load_config_from(&default_config_path())
}

/// Load config from a specific path
pub fn load_config_from(path: &PathBuf) -> Result<Config, ContextError> {
    if !path.exists() {
        return Ok(Config::default());
    }
    let content = std::fs::read_to_string(path)?;
    parse_config(&content)
}

/// Save config to the default location
pub fn save_config(config: &Config) -> Result<(), ContextError> {
    save_config_to(config, &default_config_path())
}

/// Save config to a specific path
pub fn save_config_to(config: &Config, path: &PathBuf) -> Result<(), ContextError> {
    // Ensure parent directory exists
    if let Some(parent) = path.parent() {
        std::fs::create_dir_all(parent)?;
    }
    let content = serialize_config(config)?;
    std::fs::write(path, content)?;
    Ok(())
}

impl Config {
    /// Get the URL for the current context
    ///
    /// Built-in contexts:
    /// - "local" (default): Control plane at bind_addr:8181
    /// - "worker": Local worker node at localhost:8080
    pub fn current_url(&self) -> Result<String, ContextError> {
        let ctx_name = self.current_context.as_deref().unwrap_or("local");
        match ctx_name {
            "local" => Ok(format!(
                "http://{}:{}",
                self.local.bind_addr, self.local.port
            )),
            "worker" => Ok(format!("http://localhost:{}", DEFAULT_WORKER_PORT)),
            name => self
                .contexts
                .get(name)
                .map(|c| c.url.clone())
                .ok_or_else(|| ContextError::ContextNotFound(name.to_string())),
        }
    }

    /// Check if currently using local context (control plane)
    pub fn is_local(&self) -> bool {
        self.current_context.as_deref().unwrap_or("local") == "local"
    }

    /// Check if currently using worker context
    pub fn is_worker(&self) -> bool {
        self.current_context.as_deref() == Some("worker")
    }
}

impl Context {
    /// Create a new context
    pub fn new(name: impl Into<String>, url: impl Into<String>) -> Self {
        Self {
            name: name.into(),
            url: url.into(),
            api_key: None,
            description: None,
        }
    }

    /// Add an API key
    pub fn with_api_key(mut self, key: impl Into<String>) -> Self {
        self.api_key = Some(key.into());
        self
    }

    /// Add a description
    pub fn with_description(mut self, desc: impl Into<String>) -> Self {
        self.description = Some(desc.into());
        self
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_default_config() {
        let config = Config::default();
        assert!(config.current_context.is_none());
        assert!(config.contexts.is_empty());
        assert_eq!(config.local.port, DEFAULT_CONTROL_PLANE_PORT);
    }

    #[test]
    fn test_parse_config() {
        let yaml = r#"
current-context: my-cluster
contexts:
  my-cluster:
    name: my-cluster
    url: http://10.0.0.1:8181
    api_key: secret123
local:
  port: 8181
  bind_addr: "0.0.0.0"
"#;
        let config = parse_config(yaml).unwrap();
        assert_eq!(config.current_context, Some("my-cluster".to_string()));
        assert!(config.contexts.contains_key("my-cluster"));
    }

    #[test]
    fn test_serialize_config() {
        let mut config = Config::default();
        add_context(
            &mut config,
            Context::new("test", "http://localhost:8181").with_api_key("key123"),
        );
        set_current_context(&mut config, "test").unwrap();

        let yaml = serialize_config(&config).unwrap();
        assert!(yaml.contains("current-context: test"));
        assert!(yaml.contains("url: http://localhost:8181"));
    }

    #[test]
    fn test_add_context() {
        let mut config = Config::default();
        let ctx = Context::new("remote", "http://192.168.1.1:8181");
        add_context(&mut config, ctx);

        assert!(config.contexts.contains_key("remote"));
    }

    #[test]
    fn test_remove_context() {
        let mut config = Config::default();
        add_context(&mut config, Context::new("test", "http://localhost:8181"));
        set_current_context(&mut config, "test").unwrap();

        let removed = remove_context(&mut config, "test");
        assert!(removed.is_some());
        assert!(config.current_context.is_none()); // Should be cleared
    }

    #[test]
    fn test_set_current_context() {
        let mut config = Config::default();
        add_context(&mut config, Context::new("test", "http://localhost:8181"));

        set_current_context(&mut config, "test").unwrap();
        assert_eq!(config.current_context, Some("test".to_string()));
    }

    #[test]
    fn test_set_current_context_not_found() {
        let mut config = Config::default();
        let result = set_current_context(&mut config, "nonexistent");
        assert!(matches!(result, Err(ContextError::ContextNotFound(_))));
    }

    #[test]
    fn test_local_context_allowed() {
        let mut config = Config::default();
        let result = set_current_context(&mut config, "local");
        assert!(result.is_ok());
    }

    #[test]
    fn test_current_url_local() {
        let config = Config::default();
        let url = config.current_url().unwrap();
        assert_eq!(url, "http://0.0.0.0:8181");
    }

    #[test]
    fn test_current_url_remote() {
        let mut config = Config::default();
        add_context(&mut config, Context::new("remote", "http://10.0.0.1:8181"));
        set_current_context(&mut config, "remote").unwrap();

        let url = config.current_url().unwrap();
        assert_eq!(url, "http://10.0.0.1:8181");
    }

    #[test]
    fn test_context_builder() {
        let ctx = Context::new("test", "http://localhost:8181")
            .with_api_key("secret")
            .with_description("Test cluster");

        assert_eq!(ctx.name, "test");
        assert_eq!(ctx.api_key, Some("secret".to_string()));
        assert_eq!(ctx.description, Some("Test cluster".to_string()));
    }
}
