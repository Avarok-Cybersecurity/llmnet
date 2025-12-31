//! Node resource - represents a machine in the LLMNet cluster
//!
//! A Node is a machine that can run Pipeline replicas. Each Node:
//! - Registers with the control plane
//! - Reports its capacity (GPU, memory, etc.)
//! - Receives pipeline deployments
//! - Sends heartbeats to stay registered

use std::collections::HashMap;

use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};

/// A Node in the LLMNet cluster
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Node {
    /// API version
    #[serde(rename = "apiVersion")]
    pub api_version: String,

    /// Kind is always "Node"
    pub kind: String,

    /// Metadata about the node
    pub metadata: NodeMetadata,

    /// Node specification (capabilities, labels)
    pub spec: NodeSpec,

    /// Current node status
    #[serde(skip_serializing_if = "Option::is_none")]
    pub status: Option<NodeStatus>,
}

/// Metadata for a Node
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct NodeMetadata {
    /// Unique name for this node
    pub name: String,

    /// Labels for node selection
    #[serde(default)]
    pub labels: HashMap<String, String>,

    /// Annotations for metadata
    #[serde(default)]
    pub annotations: HashMap<String, String>,
}

/// Node specification
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct NodeSpec {
    /// Address where this node can be reached (e.g., "192.168.1.100")
    pub address: String,

    /// Port where the node's control plane listens
    #[serde(default = "default_node_port")]
    pub port: u16,

    /// Whether this node can accept new pipelines
    #[serde(default = "default_true")]
    pub schedulable: bool,
}

fn default_node_port() -> u16 {
    super::WORKER_PORT
}

fn default_true() -> bool {
    true
}

/// Current status of a Node
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct NodeStatus {
    /// Overall phase: Ready, NotReady, Unknown
    pub phase: NodePhase,

    /// Detailed conditions
    #[serde(default)]
    pub conditions: Vec<NodeCondition>,

    /// Capacity of this node
    pub capacity: NodeCapacity,

    /// Currently allocated resources
    pub allocatable: NodeCapacity,

    /// Pipelines running on this node
    #[serde(default)]
    pub pipelines: Vec<NodePipelineInfo>,

    /// Last heartbeat received from this node
    #[serde(rename = "lastHeartbeat")]
    pub last_heartbeat: DateTime<Utc>,

    /// Node information (OS, version, etc.)
    #[serde(rename = "nodeInfo")]
    pub node_info: NodeInfo,

    /// Real-time metrics from this node
    #[serde(default)]
    #[serde(skip_serializing_if = "Option::is_none")]
    pub metrics: Option<NodeMetrics>,

    /// Calculated score for scheduling (populated by control plane)
    #[serde(default)]
    #[serde(skip_serializing_if = "Option::is_none")]
    pub score: Option<NodeScore>,
}

/// Phase of a node
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, Default)]
pub enum NodePhase {
    /// Node is ready to accept pipelines
    Ready,
    /// Node is not ready (failed health checks)
    NotReady,
    /// Node status is unknown (missed heartbeats)
    #[default]
    Unknown,
    /// Node is being terminated
    Terminating,
}

/// A condition of a Node
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct NodeCondition {
    /// Type of condition
    #[serde(rename = "type")]
    pub condition_type: NodeConditionType,

    /// Status: True, False, Unknown
    pub status: String,

    /// Last heartbeat time
    #[serde(rename = "lastHeartbeatTime")]
    pub last_heartbeat_time: DateTime<Utc>,

    /// Last transition time
    #[serde(rename = "lastTransitionTime")]
    pub last_transition_time: DateTime<Utc>,

    /// Reason for the condition
    pub reason: String,

    /// Human-readable message
    pub message: String,
}

/// Types of node conditions
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum NodeConditionType {
    /// Node is ready to accept pipelines
    Ready,
    /// Node has enough memory
    MemoryPressure,
    /// Node has enough disk space
    DiskPressure,
    /// Node has enough PIDs
    PIDPressure,
    /// Network is available
    NetworkUnavailable,
    /// GPU is available
    GPUAvailable,
}

/// Real-time metrics reported by a node
#[derive(Debug, Clone, Serialize, Deserialize, Default)]
pub struct NodeMetrics {
    /// CPU utilization percentage (0.0 - 100.0)
    #[serde(rename = "cpuUsagePercent")]
    #[serde(default)]
    pub cpu_usage_percent: f64,

    /// Memory utilization percentage (0.0 - 100.0)
    #[serde(rename = "memoryUsagePercent")]
    #[serde(default)]
    pub memory_usage_percent: f64,

    /// GPU utilization percentage (0.0 - 100.0), None if no GPU
    #[serde(rename = "gpuUsagePercent")]
    #[serde(skip_serializing_if = "Option::is_none")]
    pub gpu_usage_percent: Option<f64>,

    /// GPU memory utilization percentage (0.0 - 100.0), None if no GPU
    #[serde(rename = "gpuMemoryUsagePercent")]
    #[serde(skip_serializing_if = "Option::is_none")]
    pub gpu_memory_usage_percent: Option<f64>,

    /// Disk utilization percentage (0.0 - 100.0)
    #[serde(rename = "diskUsagePercent")]
    #[serde(default)]
    pub disk_usage_percent: f64,

    /// Total requests processed since last heartbeat
    #[serde(rename = "requestCount")]
    #[serde(default)]
    pub request_count: u64,

    /// Average request latency in milliseconds
    #[serde(rename = "avgLatencyMs")]
    #[serde(default)]
    pub avg_latency_ms: f64,

    /// Active concurrent requests
    #[serde(rename = "activeRequests")]
    #[serde(default)]
    pub active_requests: u32,

    /// Timestamp when metrics were collected
    #[serde(rename = "collectedAt")]
    #[serde(default = "Utc::now")]
    pub collected_at: DateTime<Utc>,
}

/// Calculated score for a node (higher = more preferred for scheduling)
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct NodeScore {
    /// Overall score (0.0 - 100.0)
    pub score: f64,

    /// Individual component scores
    pub breakdown: ScoreBreakdown,

    /// When this score was calculated
    #[serde(rename = "calculatedAt")]
    pub calculated_at: DateTime<Utc>,
}

impl Default for NodeScore {
    fn default() -> Self {
        Self {
            score: 50.0,
            breakdown: ScoreBreakdown::default(),
            calculated_at: Utc::now(),
        }
    }
}

/// Breakdown of score components
#[derive(Debug, Clone, Serialize, Deserialize, Default)]
pub struct ScoreBreakdown {
    /// CPU availability score (100 - usage%)
    #[serde(rename = "cpuScore")]
    pub cpu_score: f64,

    /// Memory availability score (100 - usage%)
    #[serde(rename = "memoryScore")]
    pub memory_score: f64,

    /// GPU availability score (100 - usage%), None if no GPU
    #[serde(rename = "gpuScore")]
    #[serde(skip_serializing_if = "Option::is_none")]
    pub gpu_score: Option<f64>,

    /// Disk availability score (100 - usage%)
    #[serde(rename = "diskScore")]
    pub disk_score: f64,

    /// Load score based on active requests
    #[serde(rename = "loadScore")]
    pub load_score: f64,
}

/// Resource capacity of a node
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct NodeCapacity {
    /// Number of CPU cores
    #[serde(default)]
    pub cpu: u32,

    /// Memory in bytes
    #[serde(default)]
    pub memory: u64,

    /// Number of GPUs
    #[serde(default)]
    pub gpu: u32,

    /// GPU memory in bytes
    #[serde(rename = "gpuMemory")]
    #[serde(default)]
    pub gpu_memory: u64,

    /// Maximum pipelines this node can run
    #[serde(rename = "maxPipelines")]
    #[serde(default = "default_max_pipelines")]
    pub max_pipelines: u32,
}

impl Default for NodeCapacity {
    fn default() -> Self {
        Self {
            cpu: 0,
            memory: 0,
            gpu: 0,
            gpu_memory: 0,
            max_pipelines: default_max_pipelines(),
        }
    }
}

fn default_max_pipelines() -> u32 {
    10
}

/// Information about a pipeline running on this node
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct NodePipelineInfo {
    /// Pipeline name
    pub name: String,

    /// Namespace
    pub namespace: String,

    /// Port where this replica is running
    pub port: u16,

    /// Status of this replica
    pub status: ReplicaStatus,
}

/// Status of a pipeline replica on a node
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum ReplicaStatus {
    /// Replica is starting up
    Starting,
    /// Replica is running and healthy
    Running,
    /// Replica failed health check
    Unhealthy,
    /// Replica is terminating
    Terminating,
    /// Replica crashed or failed
    Failed,
}

/// System information about a node
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct NodeInfo {
    /// Operating system
    pub os: String,

    /// Architecture (x86_64, aarch64)
    pub architecture: String,

    /// LLMNet version
    #[serde(rename = "llmnetVersion")]
    pub llmnet_version: String,

    /// Kernel version
    #[serde(rename = "kernelVersion")]
    #[serde(skip_serializing_if = "Option::is_none")]
    pub kernel_version: Option<String>,

    /// CUDA version (if available)
    #[serde(rename = "cudaVersion")]
    #[serde(skip_serializing_if = "Option::is_none")]
    pub cuda_version: Option<String>,

    /// GPU model names
    #[serde(rename = "gpuModels")]
    #[serde(default)]
    pub gpu_models: Vec<String>,
}

impl Node {
    /// Create a new Node with minimal configuration
    pub fn new(name: impl Into<String>, address: impl Into<String>) -> Self {
        Self {
            api_version: "llmnet/v1".to_string(),
            kind: "Node".to_string(),
            metadata: NodeMetadata {
                name: name.into(),
                labels: HashMap::new(),
                annotations: HashMap::new(),
            },
            spec: NodeSpec {
                address: address.into(),
                port: default_node_port(),
                schedulable: true,
            },
            status: None,
        }
    }

    /// Set the port
    pub fn with_port(mut self, port: u16) -> Self {
        self.spec.port = port;
        self
    }

    /// Add a label
    pub fn with_label(mut self, key: impl Into<String>, value: impl Into<String>) -> Self {
        self.metadata.labels.insert(key.into(), value.into());
        self
    }

    /// Mark as unschedulable (cordon)
    pub fn cordon(mut self) -> Self {
        self.spec.schedulable = false;
        self
    }

    /// Get the full address (address:port)
    pub fn full_address(&self) -> String {
        format!("{}:{}", self.spec.address, self.spec.port)
    }

    /// Check if node is ready
    pub fn is_ready(&self) -> bool {
        self.status
            .as_ref()
            .map(|s| s.phase == NodePhase::Ready)
            .unwrap_or(false)
    }

    /// Check if node can accept new pipelines
    pub fn can_schedule(&self) -> bool {
        self.spec.schedulable && self.is_ready()
    }

    /// Get the number of pipelines running
    pub fn pipeline_count(&self) -> usize {
        self.status.as_ref().map(|s| s.pipelines.len()).unwrap_or(0)
    }

    /// Check if node has capacity for more pipelines
    pub fn has_capacity(&self) -> bool {
        self.status
            .as_ref()
            .map(|s| s.pipelines.len() < s.capacity.max_pipelines as usize)
            .unwrap_or(false)
    }
}

impl NodeStatus {
    /// Create initial status for a node
    pub fn new(capacity: NodeCapacity, node_info: NodeInfo) -> Self {
        Self {
            phase: NodePhase::Ready,
            conditions: vec![],
            capacity: capacity.clone(),
            allocatable: capacity,
            pipelines: vec![],
            last_heartbeat: Utc::now(),
            node_info,
            metrics: None,
            score: None,
        }
    }

    /// Create status with metrics
    pub fn with_metrics(mut self, metrics: NodeMetrics) -> Self {
        self.metrics = Some(metrics);
        self
    }

    /// Update heartbeat timestamp
    pub fn heartbeat(&mut self) {
        self.last_heartbeat = Utc::now();
    }

    /// Check if node has missed heartbeats (stale)
    pub fn is_stale(&self, threshold_secs: i64) -> bool {
        let now = Utc::now();
        (now - self.last_heartbeat).num_seconds() > threshold_secs
    }
}

impl NodeInfo {
    /// Gather info from current system
    pub fn from_system() -> Self {
        Self {
            os: std::env::consts::OS.to_string(),
            architecture: std::env::consts::ARCH.to_string(),
            llmnet_version: env!("CARGO_PKG_VERSION").to_string(),
            kernel_version: None,
            cuda_version: None,
            gpu_models: vec![],
        }
    }
}

impl NodeCondition {
    /// Create a Ready condition
    pub fn ready(status: bool, reason: &str, message: &str) -> Self {
        let now = Utc::now();
        Self {
            condition_type: NodeConditionType::Ready,
            status: if status { "True" } else { "False" }.to_string(),
            last_heartbeat_time: now,
            last_transition_time: now,
            reason: reason.to_string(),
            message: message.to_string(),
        }
    }
}

impl NodeCapacity {
    /// Create capacity with GPU
    pub fn with_gpu(gpu: u32, gpu_memory_gb: u32) -> Self {
        Self {
            cpu: 0,
            memory: 0,
            gpu,
            gpu_memory: (gpu_memory_gb as u64) * 1024 * 1024 * 1024,
            max_pipelines: default_max_pipelines(),
        }
    }

    /// Set CPU cores
    pub fn with_cpu(mut self, cores: u32) -> Self {
        self.cpu = cores;
        self
    }

    /// Set memory in GB
    pub fn with_memory_gb(mut self, gb: u64) -> Self {
        self.memory = gb * 1024 * 1024 * 1024;
        self
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_create_node() {
        let node = Node::new("node-1", "192.168.1.100");

        assert_eq!(node.metadata.name, "node-1");
        assert_eq!(node.spec.address, "192.168.1.100");
        assert_eq!(node.spec.port, 8080);
        assert!(node.spec.schedulable);
    }

    #[test]
    fn test_node_builder() {
        let node = Node::new("gpu-node", "10.0.0.1")
            .with_port(9090)
            .with_label("gpu", "true")
            .with_label("type", "dgx-spark");

        assert_eq!(node.spec.port, 9090);
        assert_eq!(node.metadata.labels.get("gpu"), Some(&"true".to_string()));
    }

    #[test]
    fn test_cordon() {
        let node = Node::new("node", "localhost").cordon();
        assert!(!node.spec.schedulable);
    }

    #[test]
    fn test_full_address() {
        let node = Node::new("node", "192.168.1.100").with_port(8080);
        assert_eq!(node.full_address(), "192.168.1.100:8080");
    }

    #[test]
    fn test_is_ready() {
        let mut node = Node::new("node", "localhost");
        assert!(!node.is_ready());

        node.status = Some(NodeStatus::new(
            NodeCapacity::default(),
            NodeInfo::from_system(),
        ));
        assert!(node.is_ready());
    }

    #[test]
    fn test_capacity_builder() {
        let cap = NodeCapacity::with_gpu(4, 80)
            .with_cpu(128)
            .with_memory_gb(512);

        assert_eq!(cap.gpu, 4);
        assert_eq!(cap.gpu_memory, 80 * 1024 * 1024 * 1024);
        assert_eq!(cap.cpu, 128);
        assert_eq!(cap.memory, 512 * 1024 * 1024 * 1024);
    }

    #[test]
    fn test_node_info_from_system() {
        let info = NodeInfo::from_system();
        assert!(!info.os.is_empty());
        assert!(!info.architecture.is_empty());
        assert!(!info.llmnet_version.is_empty());
    }

    #[test]
    fn test_heartbeat_staleness() {
        let mut status = NodeStatus::new(NodeCapacity::default(), NodeInfo::from_system());

        // Fresh heartbeat
        assert!(!status.is_stale(60));

        // Simulate old heartbeat
        status.last_heartbeat = Utc::now() - chrono::Duration::seconds(120);
        assert!(status.is_stale(60));
    }
}
