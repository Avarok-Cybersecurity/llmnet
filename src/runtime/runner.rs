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

use super::docker::{self, DockerConfig, DockerError};
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

    #[error("Docker error: {0}")]
    DockerError(#[from] DockerError),

    #[error("Process exited unexpectedly")]
    ProcessExited,

    #[error("Health check failed for endpoint: {0}")]
    HealthCheckFailed(String),
}

/// Information about a running model runner process
pub struct RunnerProcess {
    /// The child process handle (None for Docker containers)
    pub child: Option<Child>,
    /// Docker container name (if applicable)
    pub container_name: Option<String>,
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

        // Use docker.port if specified, otherwise runner default, otherwise 8080
        let default_port = config
            .docker
            .as_ref()
            .and_then(|d| d.port)
            .or_else(|| config.runner.default_port())
            .unwrap_or(8080);
        let port = self.next_available_port(default_port);
        let host = &self.default_host;

        let (child, container_name, endpoint) = match config.runner {
            RunnerType::Ollama => {
                let (c, e) = self.spawn_ollama(name, config, host, port).await?;
                (Some(c), None, e)
            }
            RunnerType::Vllm => {
                let (c, e) = self.spawn_vllm(name, config, host, port).await?;
                (Some(c), None, e)
            }
            RunnerType::LlamaCpp => {
                let (c, e) = self.spawn_llamacpp(name, config, host, port).await?;
                (Some(c), None, e)
            }
            RunnerType::Docker => {
                let (cn, e) = self.spawn_docker(name, config, host, port).await?;
                (None, Some(cn), e)
            }
            RunnerType::TensorRtLlm => {
                let (c, e) = self.spawn_tensorrt_llm(name, config, host, port).await?;
                (Some(c), None, e)
            }
            RunnerType::External => {
                return Err(RunnerError::ConfigError(
                    "External runners are not spawned locally".to_string(),
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
                container_name,
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
            .map_err(|e| {
                RunnerError::SpawnError(format!("Failed to create ollama model: {}", e))
            })?;

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
            let mut modelfile = parse_modelfile(&content)
                .map_err(|e| RunnerError::ModelfileError(e.to_string()))?;

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

        // Check if vLLM is installed
        if !vllm::is_vllm_installed() {
            return Err(RunnerError::ConfigError(
                "vLLM is not installed. Install with: pip install vllm".to_string(),
            ));
        }

        // Check CUDA availability
        if !vllm::is_cuda_available() {
            warn!("CUDA not available - vLLM performance may be degraded");
        }

        // Check for HF token if model requires auth
        let hf_token = vllm::get_hf_token();
        if vllm::model_requires_auth(source) && hf_token.is_none() {
            warn!(
                "Model '{}' may require authentication. Set HF_TOKEN environment variable.",
                source
            );
        }

        let args = vllm::generate_args(source, host, port, &config.parameters);
        let env_vars = vllm::generate_env_vars(hf_token.as_deref());

        let mut cmd = Command::new("python");
        cmd.args(&args).stdout(Stdio::null()).stderr(Stdio::null());

        // Add environment variables
        for (key, value) in env_vars {
            cmd.env(key, value);
        }

        let child = cmd
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

    /// Spawn a TensorRT-LLM runner for NVIDIA Jetson/GPU devices
    async fn spawn_tensorrt_llm(
        &self,
        _name: &str,
        config: &ModelConfig,
        host: &str,
        port: u16,
    ) -> Result<(Child, String), RunnerError> {
        use crate::runtime::tensorrt_llm;

        let source = config.source.as_deref().ok_or_else(|| {
            RunnerError::ConfigError("TensorRT-LLM requires a model source".to_string())
        })?;

        // Check if we're on a Jetson device for optimal configuration
        let is_jetson = tensorrt_llm::is_jetson_device();
        if is_jetson {
            info!("Detected Jetson device, using optimized TensorRT-LLM configuration");
        }

        let args = tensorrt_llm::generate_args(source, host, port, &config.parameters);

        let child = Command::new("python")
            .args(&args)
            .envs(tensorrt_llm::generate_env_vars(config.api_key.as_deref()))
            .stdout(Stdio::null())
            .stderr(Stdio::null())
            .spawn()
            .map_err(|e| {
                RunnerError::SpawnError(format!("Failed to start TensorRT-LLM server: {}", e))
            })?;

        let endpoint = tensorrt_llm::endpoint_url(host, port);
        Ok((child, endpoint))
    }

    /// Spawn a Docker container for a model
    async fn spawn_docker(
        &self,
        name: &str,
        config: &ModelConfig,
        host: &str,
        port: u16,
    ) -> Result<(String, String), RunnerError> {
        let docker_config = config.docker.as_ref().ok_or_else(|| {
            RunnerError::ConfigError("Docker runner requires docker configuration".to_string())
        })?;

        // Validate Docker config
        docker_config.validate()?;

        let source = config.source.as_deref().ok_or_else(|| {
            RunnerError::ConfigError("Docker runner requires a model source".to_string())
        })?;

        // Generate container name
        let container_name = docker_config
            .name
            .clone()
            .unwrap_or_else(|| docker::generate_container_name("llmnet", name));

        // Handle Dockerfile build if needed
        if docker_config.needs_build() {
            self.build_docker_image(name, docker_config).await?;
        } else if let Some(image) = &docker_config.image {
            // Pull image if using registry
            if docker_config.registry.is_some() {
                self.pull_docker_image(image, docker_config).await?;
            }
        }

        // Generate run arguments
        let args = docker::generate_run_args(
            docker_config,
            source,
            port,
            &config.parameters,
            &container_name,
        );

        debug!("Docker run args: {:?}", args);

        // Run the container
        let output = Command::new("docker")
            .args(&args)
            .output()
            .await
            .map_err(|e| RunnerError::SpawnError(format!("Failed to run docker: {}", e)))?;

        if !output.status.success() {
            let stderr = String::from_utf8_lossy(&output.stderr);
            return Err(RunnerError::SpawnError(format!(
                "Docker run failed: {}",
                stderr
            )));
        }

        let endpoint = format!("http://{}:{}/v1", host, port);
        Ok((container_name, endpoint))
    }

    /// Build a Docker image from Dockerfile
    async fn build_docker_image(
        &self,
        name: &str,
        docker_config: &DockerConfig,
    ) -> Result<(), RunnerError> {
        let dockerfile_ref = docker_config
            .dockerfile
            .as_ref()
            .ok_or_else(|| RunnerError::ConfigError("Dockerfile path not specified".to_string()))?;

        // Fetch Dockerfile (supports local or remote)
        let dockerfile_path = fetch_file(dockerfile_ref)
            .await
            .map_err(|e| RunnerError::FetchError(e.to_string()))?;

        let context = docker_config
            .context
            .clone()
            .unwrap_or_else(|| ".".to_string());

        let image_name = docker_config.effective_image(name);
        let args = docker::generate_build_args(
            dockerfile_path.to_string_lossy().as_ref(),
            &context,
            &image_name,
        );

        info!("Building Docker image: {}", image_name);

        let output = Command::new("docker")
            .args(&args)
            .output()
            .await
            .map_err(|e| RunnerError::SpawnError(format!("Failed to run docker build: {}", e)))?;

        if !output.status.success() {
            let stderr = String::from_utf8_lossy(&output.stderr);
            return Err(RunnerError::SpawnError(format!(
                "Docker build failed: {}",
                stderr
            )));
        }

        Ok(())
    }

    /// Pull a Docker image from registry
    async fn pull_docker_image(
        &self,
        image: &str,
        docker_config: &DockerConfig,
    ) -> Result<(), RunnerError> {
        // Login if registry credentials provided
        if let Some(registry) = &docker_config.registry {
            if let Some((login_args, password)) = docker::generate_login_args(registry) {
                let mut login_cmd = Command::new("docker")
                    .args(&login_args)
                    .stdin(Stdio::piped())
                    .stdout(Stdio::null())
                    .stderr(Stdio::piped())
                    .spawn()
                    .map_err(|e| {
                        RunnerError::SpawnError(format!("Failed to run docker login: {}", e))
                    })?;

                // Write password to stdin
                if let Some(mut stdin) = login_cmd.stdin.take() {
                    use tokio::io::AsyncWriteExt;
                    stdin.write_all(password.as_bytes()).await?;
                }

                let output = login_cmd.wait_with_output().await?;
                if !output.status.success() {
                    let stderr = String::from_utf8_lossy(&output.stderr);
                    warn!("Docker login warning: {}", stderr);
                }
            }
        }

        // Pull the image
        let pull_args = docker::generate_pull_args(image, &docker_config.registry);

        info!("Pulling Docker image: {}", image);

        let output = Command::new("docker")
            .args(&pull_args)
            .output()
            .await
            .map_err(|e| RunnerError::SpawnError(format!("Failed to run docker pull: {}", e)))?;

        if !output.status.success() {
            let stderr = String::from_utf8_lossy(&output.stderr);
            warn!("Docker pull warning (may use local image): {}", stderr);
        }

        Ok(())
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

        error!(
            "Runner at {} failed health check after 30 attempts",
            endpoint
        );
        Err(RunnerError::HealthCheckFailed(endpoint.to_string()))
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

            // Handle Docker containers
            if let Some(container_name) = &process.container_name {
                let stop_args = docker::generate_stop_args(container_name);
                let _ = Command::new("docker").args(&stop_args).output().await;

                // Also remove the container
                let rm_args = docker::generate_rm_args(container_name);
                let _ = Command::new("docker").args(&rm_args).output().await;
            }

            // Handle regular processes
            if let Some(ref mut child) = process.child {
                child.kill().await?;
            }

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

    /// Get container name for a model (if it's a Docker runner)
    pub fn get_container_name(&self, model_name: &str) -> Option<String> {
        self.processes
            .get(model_name)
            .and_then(|p| p.container_name.clone())
    }

    /// List all Docker container names
    pub fn list_containers(&self) -> Vec<String> {
        self.processes
            .iter()
            .filter_map(|p| p.container_name.clone())
            .collect()
    }

    /// Stream container logs (returns a child process whose stdout can be read)
    pub async fn stream_container_logs(
        &self,
        container_name: &str,
        follow: bool,
        tail: Option<usize>,
    ) -> Result<tokio::process::Child, RunnerError> {
        let mut args = vec!["logs".to_string()];

        if follow {
            args.push("--follow".to_string());
        }

        if let Some(n) = tail {
            args.push("--tail".to_string());
            args.push(n.to_string());
        }

        args.push(container_name.to_string());

        let child = Command::new("docker")
            .args(&args)
            .stdout(Stdio::piped())
            .stderr(Stdio::piped())
            .spawn()
            .map_err(|e| RunnerError::SpawnError(format!("Failed to run docker logs: {}", e)))?;

        Ok(child)
    }

    /// Graceful shutdown of all runners
    pub async fn shutdown_all(&self) {
        info!("Shutting down all runners");

        // Signal shutdown
        let _ = self.shutdown_tx.send(true);

        // Kill all processes and containers
        for mut entry in self.processes.iter_mut() {
            let name = entry.key().clone();
            let process = entry.value_mut();
            info!("Stopping runner for '{}'", name);

            // Handle Docker containers
            if let Some(container_name) = &process.container_name {
                let stop_args = docker::generate_stop_args(container_name);
                let _ = Command::new("docker").args(&stop_args).output().await;

                let rm_args = docker::generate_rm_args(container_name);
                let _ = Command::new("docker").args(&rm_args).output().await;
            }

            // Handle regular processes
            if let Some(ref mut child) = process.child {
                if let Err(e) = child.kill().await {
                    error!("Failed to kill runner '{}': {}", name, e);
                }
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
