//! Runner process management
//!
//! This module provides functionality to spawn and manage local model runner
//! processes (ollama, vllm, llama.cpp) with graceful shutdown support.

use std::collections::HashMap;
use std::path::PathBuf;
use std::process::Stdio;
use std::sync::Arc;
use std::time::Duration;

use dashmap::DashMap;
use thiserror::Error;
use tokio::process::{Child, Command};
use tokio::sync::watch;
use tokio::time::sleep;
use tracing::{debug, error, info, warn};

use crate::config::models::{ModelConfig, RunnerType};

use super::fetch::fetch_file;
use super::ollama::{create_modelfile, generate_modelfile, merge_parameters, parse_modelfile};
use super::{llamacpp, vllm};

/// Errors that can occur during runner operations
#[derive(Error, Debug)]
pub enum RunnerError {
    #[error("Failed to spawn process: {0}")]
    SpawnError(String),

    #[error("Runner not found: {0}")]
    NotFound(String),

    #[error("Invalid configuration: {0}")]
    ConfigError(String),

    #[error("IO error: {0}")]
    IoError(#[from] std::io::Error),

    #[error("Fetch error: {0}")]
    FetchError(String),

    #[error("Modelfile error: {0}")]
    ModelfileError(String),

    #[error("Process exited unexpectedly")]
    ProcessExited,
}

/// Information about a running model runner process
pub struct RunnerProcess {
    /// The child process handle
    pub child: Child,
    /// The endpoint URL for this runner
    pub endpoint: String,
    /// The model name
    pub model_name: String,
    /// The runner type
    pub runner_type: RunnerType,
}

/// Manager for local model runner processes
///
/// Handles spawning, tracking, and graceful shutdown of runner processes.
pub struct RunnerManager {
    /// Map of model name to running process
    processes: DashMap<String, RunnerProcess>,
    /// Shutdown signal sender
    shutdown_tx: watch::Sender<bool>,
    /// Shutdown signal receiver (for cloning)
    shutdown_rx: watch::Receiver<bool>,
    /// Default host for spawned runners
    default_host: String,
    /// Working directory for runner configs
    work_dir: PathBuf,
}

impl RunnerManager {
    /// Create a new runner manager
    pub fn new() -> Self {
        let (shutdown_tx, shutdown_rx) = watch::channel(false);
        Self {
            processes: DashMap::new(),
            shutdown_tx,
            shutdown_rx,
            default_host: "127.0.0.1".to_string(),
            work_dir: std::env::temp_dir().join("llmnet-runners"),
        }
    }

    /// Create a runner manager with custom settings
    pub fn with_settings(host: impl Into<String>, work_dir: PathBuf) -> Self {
        let (shutdown_tx, shutdown_rx) = watch::channel(false);
        Self {
            processes: DashMap::new(),
            shutdown_tx,
            shutdown_rx,
            default_host: host.into(),
            work_dir,
        }
    }

    /// Spawn a runner process for the given model configuration
    ///
    /// Returns the endpoint URL for the spawned runner.
    pub async fn spawn_runner(
        &self,
        name: &str,
        config: &ModelConfig,
    ) -> Result<String, RunnerError> {
        // Check if already running
        if self.processes.contains_key(name) {
            let process = self.processes.get(name).unwrap();
            return Ok(process.endpoint.clone());
        }

        let port = self.next_available_port(config.runner.default_port().unwrap_or(8080));
        let host = &self.default_host;

        let (child, endpoint) = match config.runner {
            RunnerType::Ollama => self.spawn_ollama(name, config, host, port).await?,
            RunnerType::Vllm => self.spawn_vllm(name, config, host, port).await?,
            RunnerType::LlamaCpp => self.spawn_llamacpp(name, config, host, port).await?,
            RunnerType::External | RunnerType::Docker => {
                return Err(RunnerError::ConfigError(
                    "External and Docker runners are not spawned locally".to_string(),
                ));
            }
        };

        info!(
            "Spawned {} runner for '{}' at {}",
            config.type_name(),
            name,
            endpoint
        );

        self.processes.insert(
            name.to_string(),
            RunnerProcess {
                child,
                endpoint: endpoint.clone(),
                model_name: name.to_string(),
                runner_type: config.runner.clone(),
            },
        );

        // Wait for runner to be ready
        self.wait_for_ready(&endpoint).await?;

        Ok(endpoint)
    }

    /// Spawn an Ollama runner
    async fn spawn_ollama(
        &self,
        name: &str,
        config: &ModelConfig,
        host: &str,
        port: u16,
    ) -> Result<(Child, String), RunnerError> {
        // Prepare Modelfile
        let modelfile_content = self.prepare_ollama_modelfile(config).await?;

        // Write Modelfile to work directory
        std::fs::create_dir_all(&self.work_dir)?;
        let modelfile_path = self.work_dir.join(format!("{}.Modelfile", name));
        std::fs::write(&modelfile_path, &modelfile_content)?;

        debug!("Created Modelfile at {:?}", modelfile_path);

        // Start Ollama server (if not already running)
        // Note: In practice, Ollama usually runs as a daemon
        let child = Command::new("ollama")
            .args(["serve"])
            .env("OLLAMA_HOST", format!("{}:{}", host, port))
            .stdout(Stdio::null())
            .stderr(Stdio::null())
            .spawn()
            .map_err(|e| RunnerError::SpawnError(format!("Failed to start ollama serve: {}", e)))?;

        // Wait a bit for server to start
        sleep(Duration::from_secs(2)).await;

        // Create model from Modelfile
        let create_result = Command::new("ollama")
            .args(["create", name, "-f"])
            .arg(&modelfile_path)
            .env("OLLAMA_HOST", format!("{}:{}", host, port))
            .output()
            .await
            .map_err(|e| RunnerError::SpawnError(format!("Failed to create ollama model: {}", e)))?;

        if !create_result.status.success() {
            let stderr = String::from_utf8_lossy(&create_result.stderr);
            warn!("Ollama create warning: {}", stderr);
        }

        let endpoint = super::ollama::endpoint_url(host, port);
        Ok((child, endpoint))
    }

    /// Prepare Ollama Modelfile content
    async fn prepare_ollama_modelfile(&self, config: &ModelConfig) -> Result<String, RunnerError> {
        let source = config.source.as_deref().unwrap_or("tinyllama:1.1b");

        // Check if source is a Modelfile path
        if source.ends_with(".Modelfile") || source.ends_with(".modelfile") {
            // Fetch and parse existing Modelfile
            let path = fetch_file(source)
                .await
                .map_err(|e| RunnerError::FetchError(e.to_string()))?;

            let content = std::fs::read_to_string(&path)?;
            let mut modelfile =
                parse_modelfile(&content).map_err(|e| RunnerError::ModelfileError(e.to_string()))?;

            // Merge user parameters
            modelfile = merge_parameters(modelfile, &config.parameters);
            Ok(generate_modelfile(&modelfile))
        } else {
            // Create Modelfile from model name
            let mut modelfile = create_modelfile(source);
            modelfile = merge_parameters(modelfile, &config.parameters);
            Ok(generate_modelfile(&modelfile))
        }
    }

    /// Spawn a vLLM runner
    async fn spawn_vllm(
        &self,
        _name: &str,
        config: &ModelConfig,
        host: &str,
        port: u16,
    ) -> Result<(Child, String), RunnerError> {
        let source = config
            .source
            .as_deref()
            .ok_or_else(|| RunnerError::ConfigError("vLLM requires a model source".to_string()))?;

        let args = vllm::generate_args(source, host, port, &config.parameters);

        let child = Command::new("python")
            .args(&args)
            .stdout(Stdio::null())
            .stderr(Stdio::null())
            .spawn()
            .map_err(|e| RunnerError::SpawnError(format!("Failed to start vLLM: {}", e)))?;

        let endpoint = vllm::endpoint_url(host, port);
        Ok((child, endpoint))
    }

    /// Spawn a llama.cpp runner
    async fn spawn_llamacpp(
        &self,
        _name: &str,
        config: &ModelConfig,
        host: &str,
        port: u16,
    ) -> Result<(Child, String), RunnerError> {
        let source = config.source.as_deref().ok_or_else(|| {
            RunnerError::ConfigError("llama.cpp requires a model source".to_string())
        })?;

        // Fetch model file if remote
        let model_path = fetch_file(source)
            .await
            .map_err(|e| RunnerError::FetchError(e.to_string()))?;

        let args = llamacpp::generate_args(
            model_path.to_string_lossy().as_ref(),
            host,
            port,
            &config.parameters,
        );

        let child = Command::new("llama-server")
            .args(&args)
            .stdout(Stdio::null())
            .stderr(Stdio::null())
            .spawn()
            .map_err(|e| RunnerError::SpawnError(format!("Failed to start llama-server: {}", e)))?;

        let endpoint = llamacpp::endpoint_url(host, port);
        Ok((child, endpoint))
    }

    /// Wait for a runner to become ready
    async fn wait_for_ready(&self, endpoint: &str) -> Result<(), RunnerError> {
        let health_url = format!("{}/models", endpoint.trim_end_matches("/v1"));
        let client = reqwest::Client::new();

        for attempt in 1..=30 {
            match client.get(&health_url).send().await {
                Ok(resp) if resp.status().is_success() => {
                    debug!("Runner ready at {} after {} attempts", endpoint, attempt);
                    return Ok(());
                }
                Ok(_) | Err(_) => {
                    if attempt < 30 {
                        sleep(Duration::from_secs(1)).await;
                    }
                }
            }
        }

        warn!("Runner at {} may not be ready, proceeding anyway", endpoint);
        Ok(())
    }

    /// Get the next available port starting from base
    fn next_available_port(&self, base: u16) -> u16 {
        let used_ports: Vec<u16> = self
            .processes
            .iter()
            .filter_map(|p| {
                p.endpoint
                    .split(':')
                    .next_back()
                    .and_then(|s| s.split('/').next())
                    .and_then(|s| s.parse().ok())
            })
            .collect();

        let mut port = base;
        while used_ports.contains(&port) {
            port += 1;
        }
        port
    }

    /// Stop a specific runner by name
    pub async fn stop_runner(&self, name: &str) -> Result<(), RunnerError> {
        if let Some((_, mut process)) = self.processes.remove(name) {
            info!("Stopping runner for '{}'", name);
            process.child.kill().await?;
            Ok(())
        } else {
            Err(RunnerError::NotFound(name.to_string()))
        }
    }

    /// Get the endpoint for a running model
    pub fn get_endpoint(&self, name: &str) -> Option<String> {
        self.processes.get(name).map(|p| p.endpoint.clone())
    }

    /// Check if a model runner is running
    pub fn is_running(&self, name: &str) -> bool {
        self.processes.contains_key(name)
    }

    /// List all running models
    pub fn list_running(&self) -> Vec<String> {
        self.processes.iter().map(|p| p.key().clone()).collect()
    }

    /// Graceful shutdown of all runners
    pub async fn shutdown_all(&self) {
        info!("Shutting down all runners");

        // Signal shutdown
        let _ = self.shutdown_tx.send(true);

        // Kill all processes
        for mut entry in self.processes.iter_mut() {
            let name = entry.key().clone();
            info!("Stopping runner for '{}'", name);
            if let Err(e) = entry.value_mut().child.kill().await {
                error!("Failed to kill runner '{}': {}", name, e);
            }
        }

        self.processes.clear();
    }

    /// Get shutdown receiver for spawning background tasks
    pub fn shutdown_receiver(&self) -> watch::Receiver<bool> {
        self.shutdown_rx.clone()
    }
}

impl Default for RunnerManager {
    fn default() -> Self {
        Self::new()
    }
}

/// Shared runner manager for use across async tasks
pub type SharedRunnerManager = Arc<RunnerManager>;

/// Create a new shared runner manager
pub fn new_shared_manager() -> SharedRunnerManager {
    Arc::new(RunnerManager::new())
}

/// Spawn runners for all local models in a configuration
pub async fn spawn_local_runners(
    manager: &RunnerManager,
    models: &HashMap<String, ModelConfig>,
) -> Result<HashMap<String, String>, RunnerError> {
    let mut endpoints = HashMap::new();

    for (name, config) in models {
        if config.runner.is_local_runner() {
            let endpoint = manager.spawn_runner(name, config).await?;
            endpoints.insert(name.clone(), endpoint);
        }
    }

    Ok(endpoints)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_runner_manager_new() {
        let manager = RunnerManager::new();
        assert!(manager.list_running().is_empty());
    }

    #[test]
    fn test_next_available_port() {
        let manager = RunnerManager::new();
        assert_eq!(manager.next_available_port(8080), 8080);
    }

    #[test]
    fn test_not_found_error() {
        let manager = RunnerManager::new();
        assert!(!manager.is_running("nonexistent"));
        assert!(manager.get_endpoint("nonexistent").is_none());
    }

    #[tokio::test]
    async fn test_external_runner_error() {
        let manager = RunnerManager::new();
        let config = ModelConfig::external("http://example.com");
        let result = manager.spawn_runner("test", &config).await;
        assert!(matches!(result, Err(RunnerError::ConfigError(_))));
    }

    #[test]
    fn test_shutdown_receiver() {
        let manager = RunnerManager::new();
        let rx = manager.shutdown_receiver();
        assert!(!*rx.borrow());
    }
}
