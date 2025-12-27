//! Pipeline resource - the deployable unit in LLMNet
//!
//! A Pipeline is analogous to a Kubernetes Deployment. It defines:
//! - The LLM routing configuration (composition)
//! - Desired number of replicas
//! - Health check configuration
//! - Rollout strategy

use std::collections::HashMap;

use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};
use uuid::Uuid;

use crate::config::Composition;

/// A Pipeline is the deployable unit in LLMNet
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Pipeline {
    /// API version (e.g., "llmnet/v1")
    #[serde(rename = "apiVersion")]
    pub api_version: String,

    /// Kind is always "Pipeline"
    pub kind: String,

    /// Metadata about the pipeline
    pub metadata: PipelineMetadata,

    /// Desired state specification
    pub spec: PipelineSpec,

    /// Current observed status (populated by controller)
    #[serde(skip_serializing_if = "Option::is_none")]
    pub status: Option<PipelineStatus>,
}

/// Metadata for a Pipeline
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PipelineMetadata {
    /// Unique name within a namespace
    pub name: String,

    /// Namespace (defaults to "default")
    #[serde(default = "default_namespace")]
    pub namespace: String,

    /// Unique identifier (generated)
    #[serde(default = "Uuid::new_v4")]
    pub uid: Uuid,

    /// Labels for organization and selection
    #[serde(default)]
    pub labels: HashMap<String, String>,

    /// Annotations for metadata storage
    #[serde(default)]
    pub annotations: HashMap<String, String>,

    /// Creation timestamp
    #[serde(rename = "creationTimestamp")]
    #[serde(skip_serializing_if = "Option::is_none")]
    pub creation_timestamp: Option<DateTime<Utc>>,
}

fn default_namespace() -> String {
    "default".to_string()
}

/// Specification of desired Pipeline state
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PipelineSpec {
    /// Number of desired replicas (default: 1)
    #[serde(default = "default_replicas")]
    pub replicas: u32,

    /// The LLM routing composition
    pub composition: Composition,

    /// Port to expose the OpenAI-compatible API
    #[serde(default = "default_port")]
    pub port: u16,

    /// Health check configuration
    #[serde(default)]
    pub health: HealthConfig,

    /// Rollout strategy for updates
    #[serde(default)]
    pub strategy: RolloutStrategy,

    /// Node selector for placement (label-based)
    #[serde(rename = "nodeSelector")]
    #[serde(default)]
    pub node_selector: HashMap<String, String>,

    /// Resource requirements
    #[serde(default)]
    pub resources: ResourceRequirements,
}

fn default_replicas() -> u32 {
    1
}

fn default_port() -> u16 {
    8080
}

/// Health check configuration
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct HealthConfig {
    /// Liveness check path (default: /health)
    #[serde(rename = "livenessPath")]
    #[serde(default = "default_health_path")]
    pub liveness_path: String,

    /// Readiness check path (default: /health)
    #[serde(rename = "readinessPath")]
    #[serde(default = "default_health_path")]
    pub readiness_path: String,

    /// Initial delay before starting health checks (seconds)
    #[serde(rename = "initialDelaySeconds")]
    #[serde(default = "default_initial_delay")]
    pub initial_delay_seconds: u32,

    /// Interval between health checks (seconds)
    #[serde(rename = "periodSeconds")]
    #[serde(default = "default_period")]
    pub period_seconds: u32,

    /// Timeout for each health check (seconds)
    #[serde(rename = "timeoutSeconds")]
    #[serde(default = "default_timeout")]
    pub timeout_seconds: u32,

    /// Number of failures before marking unhealthy
    #[serde(rename = "failureThreshold")]
    #[serde(default = "default_failure_threshold")]
    pub failure_threshold: u32,
}

impl Default for HealthConfig {
    fn default() -> Self {
        Self {
            liveness_path: default_health_path(),
            readiness_path: default_health_path(),
            initial_delay_seconds: default_initial_delay(),
            period_seconds: default_period(),
            timeout_seconds: default_timeout(),
            failure_threshold: default_failure_threshold(),
        }
    }
}

fn default_health_path() -> String {
    "/health".to_string()
}

fn default_initial_delay() -> u32 {
    5
}

fn default_period() -> u32 {
    10
}

fn default_timeout() -> u32 {
    5
}

fn default_failure_threshold() -> u32 {
    3
}

/// Rollout strategy for pipeline updates
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct RolloutStrategy {
    /// Type of rollout: "RollingUpdate" or "Recreate"
    #[serde(rename = "type")]
    #[serde(default = "default_strategy_type")]
    pub strategy_type: String,

    /// Rolling update parameters (if type is RollingUpdate)
    #[serde(rename = "rollingUpdate")]
    #[serde(skip_serializing_if = "Option::is_none")]
    pub rolling_update: Option<RollingUpdateParams>,
}

impl Default for RolloutStrategy {
    fn default() -> Self {
        Self {
            strategy_type: default_strategy_type(),
            rolling_update: Some(RollingUpdateParams::default()),
        }
    }
}

fn default_strategy_type() -> String {
    "RollingUpdate".to_string()
}

/// Parameters for rolling update
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct RollingUpdateParams {
    /// Maximum number of replicas that can be unavailable during update
    #[serde(rename = "maxUnavailable")]
    #[serde(default = "default_max_unavailable")]
    pub max_unavailable: u32,

    /// Maximum number of replicas that can be created above desired
    #[serde(rename = "maxSurge")]
    #[serde(default = "default_max_surge")]
    pub max_surge: u32,
}

impl Default for RollingUpdateParams {
    fn default() -> Self {
        Self {
            max_unavailable: default_max_unavailable(),
            max_surge: default_max_surge(),
        }
    }
}

fn default_max_unavailable() -> u32 {
    1
}

fn default_max_surge() -> u32 {
    1
}

/// Resource requirements for the pipeline
#[derive(Debug, Clone, Serialize, Deserialize, Default)]
pub struct ResourceRequirements {
    /// GPU memory requirement (e.g., "16Gi")
    #[serde(skip_serializing_if = "Option::is_none")]
    pub gpu_memory: Option<String>,

    /// CPU requirement (e.g., "4")
    #[serde(skip_serializing_if = "Option::is_none")]
    pub cpu: Option<String>,

    /// Memory requirement (e.g., "32Gi")
    #[serde(skip_serializing_if = "Option::is_none")]
    pub memory: Option<String>,
}

/// Current status of a Pipeline (observed state)
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PipelineStatus {
    /// Total replicas currently managed
    pub replicas: u32,

    /// Number of ready replicas
    #[serde(rename = "readyReplicas")]
    pub ready_replicas: u32,

    /// Number of available replicas (passed health checks)
    #[serde(rename = "availableReplicas")]
    pub available_replicas: u32,

    /// Number of unavailable replicas
    #[serde(rename = "unavailableReplicas")]
    pub unavailable_replicas: u32,

    /// Generation observed by controller
    #[serde(rename = "observedGeneration")]
    pub observed_generation: u64,

    /// Current conditions
    #[serde(default)]
    pub conditions: Vec<PipelineCondition>,

    /// Endpoints where pipeline is accessible
    #[serde(default)]
    pub endpoints: Vec<String>,
}

/// A condition of a Pipeline
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PipelineCondition {
    /// Type of condition (Available, Progressing, ReplicaFailure)
    #[serde(rename = "type")]
    pub condition_type: String,

    /// Status: True, False, Unknown
    pub status: String,

    /// Last time the condition was updated
    #[serde(rename = "lastUpdateTime")]
    pub last_update_time: DateTime<Utc>,

    /// Last time the condition transitioned
    #[serde(rename = "lastTransitionTime")]
    pub last_transition_time: DateTime<Utc>,

    /// Machine-readable reason for the condition
    pub reason: String,

    /// Human-readable message
    pub message: String,
}

impl Pipeline {
    /// Create a new Pipeline with minimal configuration
    pub fn new(name: impl Into<String>, composition: Composition) -> Self {
        Self {
            api_version: "llmnet/v1".to_string(),
            kind: "Pipeline".to_string(),
            metadata: PipelineMetadata {
                name: name.into(),
                namespace: default_namespace(),
                uid: Uuid::new_v4(),
                labels: HashMap::new(),
                annotations: HashMap::new(),
                creation_timestamp: Some(Utc::now()),
            },
            spec: PipelineSpec {
                replicas: 1,
                composition,
                port: default_port(),
                health: HealthConfig::default(),
                strategy: RolloutStrategy::default(),
                node_selector: HashMap::new(),
                resources: ResourceRequirements::default(),
            },
            status: None,
        }
    }

    /// Set the number of replicas
    pub fn with_replicas(mut self, replicas: u32) -> Self {
        self.spec.replicas = replicas;
        self
    }

    /// Set the namespace
    pub fn with_namespace(mut self, namespace: impl Into<String>) -> Self {
        self.metadata.namespace = namespace.into();
        self
    }

    /// Add a label
    pub fn with_label(mut self, key: impl Into<String>, value: impl Into<String>) -> Self {
        self.metadata.labels.insert(key.into(), value.into());
        self
    }

    /// Get the full qualified name (namespace/name)
    pub fn qualified_name(&self) -> String {
        format!("{}/{}", self.metadata.namespace, self.metadata.name)
    }

    /// Check if pipeline is ready (all replicas available)
    pub fn is_ready(&self) -> bool {
        self.status
            .as_ref()
            .map(|s| s.available_replicas >= self.spec.replicas)
            .unwrap_or(false)
    }
}

impl PipelineStatus {
    /// Create initial status for a newly created pipeline
    pub fn initial() -> Self {
        Self {
            replicas: 0,
            ready_replicas: 0,
            available_replicas: 0,
            unavailable_replicas: 0,
            observed_generation: 0,
            conditions: vec![],
            endpoints: vec![],
        }
    }

    /// Add a condition
    pub fn add_condition(&mut self, condition: PipelineCondition) {
        // Remove existing condition of same type
        self.conditions
            .retain(|c| c.condition_type != condition.condition_type);
        self.conditions.push(condition);
    }
}

impl PipelineCondition {
    /// Create a new condition
    pub fn new(
        condition_type: impl Into<String>,
        status: impl Into<String>,
        reason: impl Into<String>,
        message: impl Into<String>,
    ) -> Self {
        let now = Utc::now();
        Self {
            condition_type: condition_type.into(),
            status: status.into(),
            last_update_time: now,
            last_transition_time: now,
            reason: reason.into(),
            message: message.into(),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn create_test_composition() -> Composition {
        let json = r#"{
            "models": {},
            "architecture": [
                {"name": "router", "layer": 0, "adapter": "openai-api"},
                {"name": "output", "adapter": "output"}
            ]
        }"#;
        Composition::from_str(json).unwrap()
    }

    #[test]
    fn test_create_pipeline() {
        let comp = create_test_composition();
        let pipeline = Pipeline::new("my-pipeline", comp);

        assert_eq!(pipeline.metadata.name, "my-pipeline");
        assert_eq!(pipeline.metadata.namespace, "default");
        assert_eq!(pipeline.spec.replicas, 1);
        assert_eq!(pipeline.api_version, "llmnet/v1");
        assert_eq!(pipeline.kind, "Pipeline");
    }

    #[test]
    fn test_pipeline_builder() {
        let comp = create_test_composition();
        let pipeline = Pipeline::new("test", comp)
            .with_replicas(3)
            .with_namespace("production")
            .with_label("app", "chatbot");

        assert_eq!(pipeline.spec.replicas, 3);
        assert_eq!(pipeline.metadata.namespace, "production");
        assert_eq!(
            pipeline.metadata.labels.get("app"),
            Some(&"chatbot".to_string())
        );
    }

    #[test]
    fn test_qualified_name() {
        let comp = create_test_composition();
        let pipeline = Pipeline::new("my-pipeline", comp).with_namespace("prod");

        assert_eq!(pipeline.qualified_name(), "prod/my-pipeline");
    }

    #[test]
    fn test_is_ready() {
        let comp = create_test_composition();
        let mut pipeline = Pipeline::new("test", comp).with_replicas(3);

        // Not ready initially (no status)
        assert!(!pipeline.is_ready());

        // Partially ready
        pipeline.status = Some(PipelineStatus {
            replicas: 3,
            ready_replicas: 2,
            available_replicas: 2,
            unavailable_replicas: 1,
            observed_generation: 1,
            conditions: vec![],
            endpoints: vec![],
        });
        assert!(!pipeline.is_ready());

        // Fully ready
        pipeline.status = Some(PipelineStatus {
            replicas: 3,
            ready_replicas: 3,
            available_replicas: 3,
            unavailable_replicas: 0,
            observed_generation: 1,
            conditions: vec![],
            endpoints: vec![],
        });
        assert!(pipeline.is_ready());
    }

    #[test]
    fn test_serialize_pipeline() {
        let comp = create_test_composition();
        let pipeline = Pipeline::new("test", comp)
            .with_replicas(2)
            .with_label("env", "dev");

        let yaml = serde_yaml::to_string(&pipeline).unwrap();
        assert!(yaml.contains("apiVersion: llmnet/v1"));
        assert!(yaml.contains("kind: Pipeline"));
        assert!(yaml.contains("replicas: 2"));
    }

    #[test]
    fn test_default_health_config() {
        let health = HealthConfig::default();
        assert_eq!(health.liveness_path, "/health");
        assert_eq!(health.period_seconds, 10);
        assert_eq!(health.failure_threshold, 3);
    }

    #[test]
    fn test_default_rollout_strategy() {
        let strategy = RolloutStrategy::default();
        assert_eq!(strategy.strategy_type, "RollingUpdate");
        let rolling = strategy.rolling_update.unwrap();
        assert_eq!(rolling.max_unavailable, 1);
        assert_eq!(rolling.max_surge, 1);
    }

    #[test]
    fn test_pipeline_condition() {
        let condition = PipelineCondition::new("Available", "True", "MinimumReplicasAvailable", "");

        assert_eq!(condition.condition_type, "Available");
        assert_eq!(condition.status, "True");
    }
}
