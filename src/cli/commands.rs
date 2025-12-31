//! Command implementations for the CLI
//!
//! SBIO pattern: Commands return Results, I/O is handled by caller

use std::path::PathBuf;

use thiserror::Error;

use crate::cluster::Pipeline;
use crate::config::load_composition_file;
use crate::context::{self, Config, Context, ContextError, DEFAULT_WORKER_PORT};

/// Errors that can occur during command execution
#[derive(Error, Debug)]
pub enum CommandError {
    #[error("Context error: {0}")]
    Context(#[from] ContextError),

    #[error("Config error: {0}")]
    Config(String),

    #[error("HTTP error: {0}")]
    Http(#[from] reqwest::Error),

    #[error("IO error: {0}")]
    Io(#[from] std::io::Error),

    #[error("JSON error: {0}")]
    Json(#[from] serde_json::Error),

    #[error("Server error: {0}")]
    Server(String),

    #[error("Not connected: no current context set")]
    NotConnected,
}

/// Result type for commands
pub type CommandResult<T> = Result<T, CommandError>;

// ============================================================================
// Context Commands (Pure business logic)
// ============================================================================

/// List all contexts
pub fn context_list(config: &Config) -> Vec<ContextInfo> {
    let current = config.current_context.as_deref();
    let mut contexts: Vec<_> = config
        .contexts
        .iter()
        .map(|(name, ctx)| ContextInfo {
            name: name.clone(),
            url: ctx.url.clone(),
            is_current: Some(name.as_str()) == current,
        })
        .collect();

    // Always include built-in contexts
    contexts.push(ContextInfo {
        name: "local".to_string(),
        url: format!("http://{}:{}", config.local.bind_addr, config.local.port),
        is_current: current == Some("local") || current.is_none(),
    });

    contexts.push(ContextInfo {
        name: "worker".to_string(),
        url: format!("http://localhost:{}", DEFAULT_WORKER_PORT),
        is_current: current == Some("worker"),
    });

    contexts.sort_by(|a, b| a.name.cmp(&b.name));
    contexts
}

/// Info about a context for display
#[derive(Debug, Clone)]
pub struct ContextInfo {
    pub name: String,
    pub url: String,
    pub is_current: bool,
}

/// Get current context name and URL
pub fn context_current(config: &Config) -> CommandResult<(String, String)> {
    let name = config.current_context.as_deref().unwrap_or("local");
    let url = config.current_url()?;
    Ok((name.to_string(), url))
}

/// Switch to a context
pub fn context_use(config: &mut Config, name: &str) -> CommandResult<()> {
    context::set_current_context(config, name)?;
    Ok(())
}

/// Add a new context
pub fn context_add(
    config: &mut Config,
    name: &str,
    url: &str,
    api_key: Option<&str>,
) -> CommandResult<()> {
    let mut ctx = Context::new(name, url);
    if let Some(key) = api_key {
        ctx = ctx.with_api_key(key);
    }
    context::add_context(config, ctx);
    Ok(())
}

/// Delete a context
pub fn context_delete(config: &mut Config, name: &str) -> CommandResult<bool> {
    let removed = context::remove_context(config, name);
    Ok(removed.is_some())
}

// ============================================================================
// Deploy Commands
// ============================================================================

/// Load and parse a pipeline manifest
pub fn load_pipeline_manifest(path: &PathBuf) -> CommandResult<Pipeline> {
    let content = std::fs::read_to_string(path)?;

    // Try YAML first, then JSON
    let pipeline: Pipeline = if path.extension().and_then(|e| e.to_str()) == Some("yaml")
        || path.extension().and_then(|e| e.to_str()) == Some("yml")
    {
        serde_yaml::from_str(&content).map_err(|e| CommandError::Config(e.to_string()))?
    } else {
        serde_json::from_str(&content)?
    };

    Ok(pipeline)
}

/// Create a pipeline from a composition file (legacy format)
pub fn pipeline_from_composition(path: &std::path::Path, name: &str) -> CommandResult<Pipeline> {
    let composition =
        load_composition_file(path).map_err(|e| CommandError::Config(e.to_string()))?;
    Ok(Pipeline::new(name, composition))
}

// ============================================================================
// Validate Commands
// ============================================================================

/// Validate a composition file
pub fn validate_composition(path: &std::path::Path) -> CommandResult<ValidationResult> {
    match load_composition_file(path) {
        Ok(comp) => Ok(ValidationResult {
            valid: true,
            models: comp.models.len(),
            nodes: comp.architecture.len(),
            error: None,
        }),
        Err(e) => Ok(ValidationResult {
            valid: false,
            models: 0,
            nodes: 0,
            error: Some(e.to_string()),
        }),
    }
}

/// Result of validating a composition
#[derive(Debug)]
pub struct ValidationResult {
    pub valid: bool,
    pub models: usize,
    pub nodes: usize,
    pub error: Option<String>,
}

// ============================================================================
// HTTP Client for Control Plane
// ============================================================================

/// Client for communicating with the control plane
pub struct ControlPlaneClient {
    client: reqwest::Client,
    base_url: String,
    api_key: Option<String>,
}

impl ControlPlaneClient {
    /// Create a new client
    pub fn new(base_url: impl Into<String>) -> Self {
        Self {
            client: reqwest::Client::new(),
            base_url: base_url.into(),
            api_key: None,
        }
    }

    /// Set the API key
    pub fn with_api_key(mut self, key: impl Into<String>) -> Self {
        self.api_key = Some(key.into());
        self
    }

    /// Create from current context
    pub fn from_context(config: &Config) -> CommandResult<Self> {
        let url = config.current_url()?;
        let api_key = config
            .current_context
            .as_ref()
            .and_then(|name| config.contexts.get(name))
            .and_then(|ctx| ctx.api_key.clone());

        let mut client = Self::new(url);
        if let Some(key) = api_key {
            client = client.with_api_key(key);
        }
        Ok(client)
    }

    fn build_request(&self, method: reqwest::Method, path: &str) -> reqwest::RequestBuilder {
        let url = format!("{}{}", self.base_url, path);
        let mut req = self.client.request(method, &url);
        if let Some(ref key) = self.api_key {
            req = req.header("Authorization", format!("Bearer {}", key));
        }
        req
    }

    /// Get cluster status
    pub async fn status(&self) -> CommandResult<serde_json::Value> {
        let resp = self
            .build_request(reqwest::Method::GET, "/v1/status")
            .send()
            .await?;

        if !resp.status().is_success() {
            return Err(CommandError::Server(format!(
                "Status check failed: {}",
                resp.status()
            )));
        }

        Ok(resp.json().await?)
    }

    /// Deploy a pipeline
    pub async fn deploy(&self, pipeline: &Pipeline) -> CommandResult<Pipeline> {
        let resp = self
            .build_request(reqwest::Method::POST, "/v1/pipelines")
            .json(pipeline)
            .send()
            .await?;

        let status = resp.status();
        let body: serde_json::Value = resp.json().await?;

        if !status.is_success() {
            let error = body["error"].as_str().unwrap_or("Unknown error");
            return Err(CommandError::Server(error.to_string()));
        }

        let pipeline: Pipeline = serde_json::from_value(body["pipeline"].clone())?;
        Ok(pipeline)
    }

    /// List pipelines
    pub async fn list_pipelines(&self, namespace: Option<&str>) -> CommandResult<Vec<Pipeline>> {
        let path = match namespace {
            Some(ns) => format!("/v1/namespaces/{}/pipelines", ns),
            None => "/v1/pipelines".to_string(),
        };

        let resp = self
            .build_request(reqwest::Method::GET, &path)
            .send()
            .await?;

        if !resp.status().is_success() {
            return Err(CommandError::Server(format!(
                "Failed to list pipelines: {}",
                resp.status()
            )));
        }

        let body: serde_json::Value = resp.json().await?;
        let pipelines: Vec<Pipeline> = serde_json::from_value(body["items"].clone())?;
        Ok(pipelines)
    }

    /// Get a specific pipeline
    pub async fn get_pipeline(
        &self,
        namespace: &str,
        name: &str,
    ) -> CommandResult<Option<Pipeline>> {
        let path = format!("/v1/namespaces/{}/pipelines/{}", namespace, name);

        let resp = self
            .build_request(reqwest::Method::GET, &path)
            .send()
            .await?;

        if resp.status().as_u16() == 404 {
            return Ok(None);
        }

        if !resp.status().is_success() {
            return Err(CommandError::Server(format!(
                "Failed to get pipeline: {}",
                resp.status()
            )));
        }

        Ok(resp.json().await?)
    }

    /// Delete a pipeline
    pub async fn delete_pipeline(&self, namespace: &str, name: &str) -> CommandResult<bool> {
        let path = format!("/v1/namespaces/{}/pipelines/{}", namespace, name);

        let resp = self
            .build_request(reqwest::Method::DELETE, &path)
            .send()
            .await?;

        Ok(resp.status().is_success())
    }

    /// Scale a pipeline
    pub async fn scale_pipeline(
        &self,
        namespace: &str,
        name: &str,
        replicas: u32,
    ) -> CommandResult<Pipeline> {
        let path = format!("/v1/namespaces/{}/pipelines/{}/scale", namespace, name);

        let resp = self
            .build_request(reqwest::Method::PATCH, &path)
            .json(&serde_json::json!({ "replicas": replicas }))
            .send()
            .await?;

        let status = resp.status();
        let body: serde_json::Value = resp.json().await?;

        if !status.is_success() {
            let error = body["error"].as_str().unwrap_or("Unknown error");
            return Err(CommandError::Server(error.to_string()));
        }

        let pipeline: Pipeline = serde_json::from_value(body["pipeline"].clone())?;
        Ok(pipeline)
    }

    /// List nodes
    pub async fn list_nodes(&self) -> CommandResult<Vec<serde_json::Value>> {
        let resp = self
            .build_request(reqwest::Method::GET, "/v1/nodes")
            .send()
            .await?;

        if !resp.status().is_success() {
            return Err(CommandError::Server(format!(
                "Failed to list nodes: {}",
                resp.status()
            )));
        }

        let body: serde_json::Value = resp.json().await?;
        let nodes: Vec<serde_json::Value> = serde_json::from_value(body["items"].clone())?;
        Ok(nodes)
    }

    /// Delete a node
    pub async fn delete_node(&self, name: &str) -> CommandResult<bool> {
        let path = format!("/v1/nodes/{}", name);

        let resp = self
            .build_request(reqwest::Method::DELETE, &path)
            .send()
            .await?;

        Ok(resp.status().is_success())
    }

    /// List namespaces
    pub async fn list_namespaces(&self) -> CommandResult<Vec<serde_json::Value>> {
        let resp = self
            .build_request(reqwest::Method::GET, "/v1/namespaces")
            .send()
            .await?;

        if !resp.status().is_success() {
            return Err(CommandError::Server(format!(
                "Failed to list namespaces: {}",
                resp.status()
            )));
        }

        let body: serde_json::Value = resp.json().await?;
        let namespaces: Vec<serde_json::Value> = serde_json::from_value(body["items"].clone())?;
        Ok(namespaces)
    }

    /// Stream logs for a pipeline
    /// Returns a Response that can be streamed
    pub async fn stream_logs(
        &self,
        namespace: &str,
        name: &str,
        follow: bool,
        tail: usize,
    ) -> CommandResult<reqwest::Response> {
        let path = format!(
            "/v1/namespaces/{}/pipelines/{}/logs?follow={}&tail={}",
            namespace, name, follow, tail
        );

        let resp = self
            .build_request(reqwest::Method::GET, &path)
            .send()
            .await?;

        if !resp.status().is_success() {
            let status = resp.status();
            let body = resp.text().await.unwrap_or_default();
            return Err(CommandError::Server(format!(
                "Failed to stream logs ({}): {}",
                status, body
            )));
        }

        Ok(resp)
    }
}

// ============================================================================
// HTTP Client for Worker Nodes
// ============================================================================

/// Client for communicating with a local worker node
pub struct WorkerClient {
    client: reqwest::Client,
    base_url: String,
}

impl WorkerClient {
    /// Create a new worker client
    pub fn new(base_url: impl Into<String>) -> Self {
        Self {
            client: reqwest::Client::new(),
            base_url: base_url.into(),
        }
    }

    /// Create from current context (must be "worker" context)
    pub fn from_context(config: &Config) -> CommandResult<Self> {
        let url = config.current_url()?;
        Ok(Self::new(url))
    }

    fn build_request(&self, method: reqwest::Method, path: &str) -> reqwest::RequestBuilder {
        let url = format!("{}{}", self.base_url, path);
        self.client.request(method, &url)
    }

    /// Get worker status
    pub async fn status(&self) -> CommandResult<serde_json::Value> {
        let resp = self
            .build_request(reqwest::Method::GET, "/status")
            .send()
            .await?;

        if !resp.status().is_success() {
            return Err(CommandError::Server(format!(
                "Status check failed: {}",
                resp.status()
            )));
        }

        Ok(resp.json().await?)
    }

    /// List running containers
    pub async fn list_containers(&self) -> CommandResult<Vec<String>> {
        let resp = self
            .build_request(reqwest::Method::GET, "/v1/containers")
            .send()
            .await?;

        if !resp.status().is_success() {
            return Err(CommandError::Server(format!(
                "Failed to list containers: {}",
                resp.status()
            )));
        }

        let body: serde_json::Value = resp.json().await?;
        let containers = body["containers"]
            .as_array()
            .map(|arr| {
                arr.iter()
                    .filter_map(|v| v.as_str().map(String::from))
                    .collect()
            })
            .unwrap_or_default();

        Ok(containers)
    }

    /// List running model runners
    pub async fn list_runners(&self) -> CommandResult<Vec<serde_json::Value>> {
        let resp = self
            .build_request(reqwest::Method::GET, "/v1/runners")
            .send()
            .await?;

        if !resp.status().is_success() {
            return Err(CommandError::Server(format!(
                "Failed to list runners: {}",
                resp.status()
            )));
        }

        let body: serde_json::Value = resp.json().await?;
        let runners = body["runners"].as_array().cloned().unwrap_or_default();

        Ok(runners)
    }

    /// Stream container logs
    pub async fn stream_logs(
        &self,
        container: &str,
        follow: bool,
        tail: usize,
    ) -> CommandResult<reqwest::Response> {
        let path = format!(
            "/v1/containers/{}/logs?follow={}&tail={}",
            container, follow, tail
        );

        let resp = self
            .build_request(reqwest::Method::GET, &path)
            .send()
            .await?;

        if !resp.status().is_success() {
            let status = resp.status();
            let body = resp.text().await.unwrap_or_default();
            return Err(CommandError::Server(format!(
                "Failed to stream logs ({}): {}",
                status, body
            )));
        }

        Ok(resp)
    }
}

// ============================================================================
// Server Health Check (for serve command pre-flight)
// ============================================================================

/// Result of checking if a server is already running
#[derive(Debug, Clone)]
pub enum ServerStatus {
    /// No server running on this port
    NotRunning,
    /// Server running and healthy
    RunningHealthy,
    /// Server running but unhealthy
    RunningUnhealthy(String),
}

/// Check if a server is already running on the given address
pub async fn check_server_status(addr: &str) -> ServerStatus {
    let client = reqwest::Client::builder()
        .timeout(std::time::Duration::from_secs(5))
        .build()
        .unwrap_or_else(|_| reqwest::Client::new());

    let health_url = format!("http://{}/health", addr);

    match client.get(&health_url).send().await {
        Ok(resp) => {
            if resp.status().is_success() {
                ServerStatus::RunningHealthy
            } else {
                ServerStatus::RunningUnhealthy(format!("Health check returned: {}", resp.status()))
            }
        }
        Err(e) => {
            // Check if it's a connection refused error (not running)
            // vs a timeout or other error (running but unhealthy)
            let err_str = e.to_string();
            if err_str.contains("Connection refused")
                || err_str.contains("connection refused")
                || err_str.contains("No connection could be made")
            {
                ServerStatus::NotRunning
            } else if e.is_timeout() {
                ServerStatus::RunningUnhealthy("Health check timed out".to_string())
            } else {
                // Could be DNS error or other - treat as not running
                ServerStatus::NotRunning
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_context_list() {
        let config = Config::default();
        let contexts = context_list(&config);

        // Should have at least "local"
        assert!(contexts.iter().any(|c| c.name == "local"));
    }

    #[test]
    fn test_context_add_and_list() {
        let mut config = Config::default();
        context_add(&mut config, "test", "http://localhost:8181", None).unwrap();

        let contexts = context_list(&config);
        assert!(contexts.iter().any(|c| c.name == "test"));
    }

    #[test]
    fn test_context_use() {
        let mut config = Config::default();
        context_add(&mut config, "test", "http://localhost:8181", None).unwrap();
        context_use(&mut config, "test").unwrap();

        let (current, _) = context_current(&config).unwrap();
        assert_eq!(current, "test");
    }

    #[test]
    fn test_context_delete() {
        let mut config = Config::default();
        context_add(&mut config, "test", "http://localhost:8181", None).unwrap();

        let removed = context_delete(&mut config, "test").unwrap();
        assert!(removed);

        let contexts = context_list(&config);
        assert!(!contexts.iter().any(|c| c.name == "test"));
    }

    #[test]
    fn test_validation_result() {
        // Test with a non-existent file
        let result = validate_composition(&PathBuf::from("/nonexistent/file.json"));
        // Should return an error or invalid result
        assert!(result.is_err() || !result.unwrap().valid);
    }
}
