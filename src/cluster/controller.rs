//! Cluster Controller - manages cluster state and orchestration
//!
//! The controller is responsible for:
//! - Tracking registered nodes
//! - Managing deployed pipelines
//! - Scheduling pipeline replicas to nodes
//! - Health monitoring and recovery

use std::collections::HashMap;
use std::sync::Arc;

use dashmap::DashMap;
use thiserror::Error;
use tokio::sync::RwLock;

use super::node::{Node, NodePhase, NodePipelineInfo, NodeStatus, ReplicaStatus};
use super::pipeline::{Pipeline, PipelineStatus};
use super::resources::{LabelSelector, Namespace};
use super::HEARTBEAT_INTERVAL_SECS;

/// Errors that can occur in the cluster controller
#[derive(Error, Debug)]
pub enum ControllerError {
    #[error("Pipeline '{0}' not found in namespace '{1}'")]
    PipelineNotFound(String, String),

    #[error("Pipeline '{0}' already exists in namespace '{1}'")]
    PipelineExists(String, String),

    #[error("Node '{0}' not found")]
    NodeNotFound(String),

    #[error("Node '{0}' already registered")]
    NodeExists(String),

    #[error("Namespace '{0}' not found")]
    NamespaceNotFound(String),

    #[error("No available nodes for scheduling")]
    NoAvailableNodes,

    #[error("Insufficient capacity: {0}")]
    InsufficientCapacity(String),

    #[error("Validation error: {0}")]
    ValidationError(String),

    #[error("Internal error: {0}")]
    InternalError(String),
}

/// The cluster controller manages all cluster state
#[derive(Clone)]
pub struct ClusterController {
    /// Registered nodes indexed by name
    nodes: Arc<DashMap<String, Node>>,

    /// Pipelines indexed by qualified name (namespace/name)
    pipelines: Arc<DashMap<String, Pipeline>>,

    /// Namespaces indexed by name
    namespaces: Arc<DashMap<String, Namespace>>,

    /// Controller configuration
    config: Arc<RwLock<ControllerConfig>>,
}

/// Controller configuration
#[derive(Debug, Clone)]
pub struct ControllerConfig {
    /// How often to check node health (seconds)
    pub health_check_interval: u64,

    /// Threshold for marking node as unknown (seconds)
    pub node_heartbeat_timeout: i64,

    /// Maximum pipelines per node (can be overridden per-node)
    pub default_max_pipelines_per_node: u32,
}

impl Default for ControllerConfig {
    fn default() -> Self {
        Self {
            health_check_interval: 10,
            node_heartbeat_timeout: (HEARTBEAT_INTERVAL_SECS * 3) as i64,
            default_max_pipelines_per_node: 10,
        }
    }
}

impl ClusterController {
    /// Create a new cluster controller
    pub fn new() -> Self {
        let controller = Self {
            nodes: Arc::new(DashMap::new()),
            pipelines: Arc::new(DashMap::new()),
            namespaces: Arc::new(DashMap::new()),
            config: Arc::new(RwLock::new(ControllerConfig::default())),
        };

        // Create default namespace
        controller
            .namespaces
            .insert("default".to_string(), Namespace::default());

        controller
    }

    /// Create with custom configuration
    pub fn with_config(config: ControllerConfig) -> Self {
        let controller = Self::new();
        let mut cfg = futures::executor::block_on(controller.config.write());
        *cfg = config;
        drop(cfg);
        controller
    }

    // =========================================================================
    // Node Management
    // =========================================================================

    /// Register a new node
    pub fn register_node(&self, node: Node) -> Result<(), ControllerError> {
        if self.nodes.contains_key(&node.metadata.name) {
            return Err(ControllerError::NodeExists(node.metadata.name.clone()));
        }
        self.nodes.insert(node.metadata.name.clone(), node);
        Ok(())
    }

    /// Update node status (heartbeat)
    ///
    /// If the status includes metrics, calculates and stores the node score.
    pub fn update_node_status(
        &self,
        name: &str,
        mut status: NodeStatus,
    ) -> Result<(), ControllerError> {
        let mut node = self
            .nodes
            .get_mut(name)
            .ok_or_else(|| ControllerError::NodeNotFound(name.to_string()))?;

        // Calculate score if metrics are present
        if let Some(ref metrics) = status.metrics {
            let has_gpu = status.capacity.gpu > 0;
            let score = super::scoring::calculate_node_score(metrics, has_gpu, None);
            status.score = Some(score);
        }

        node.status = Some(status);
        Ok(())
    }

    /// Unregister a node
    pub fn unregister_node(&self, name: &str) -> Result<Node, ControllerError> {
        self.nodes
            .remove(name)
            .map(|(_, n)| n)
            .ok_or_else(|| ControllerError::NodeNotFound(name.to_string()))
    }

    /// Get a node by name
    pub fn get_node(&self, name: &str) -> Option<Node> {
        self.nodes.get(name).map(|r| r.clone())
    }

    /// List all nodes
    pub fn list_nodes(&self) -> Vec<Node> {
        self.nodes.iter().map(|r| r.clone()).collect()
    }

    /// List nodes matching a label selector
    pub fn list_nodes_by_selector(&self, selector: &LabelSelector) -> Vec<Node> {
        self.nodes
            .iter()
            .filter(|r| selector.matches(&r.metadata.labels))
            .map(|r| r.clone())
            .collect()
    }

    /// Get nodes that can accept new pipelines
    pub fn get_schedulable_nodes(&self) -> Vec<Node> {
        self.nodes
            .iter()
            .filter(|r| r.can_schedule() && r.has_capacity())
            .map(|r| r.clone())
            .collect()
    }

    /// Cordon a node (mark unschedulable)
    pub fn cordon_node(&self, name: &str) -> Result<(), ControllerError> {
        let mut node = self
            .nodes
            .get_mut(name)
            .ok_or_else(|| ControllerError::NodeNotFound(name.to_string()))?;
        node.spec.schedulable = false;
        Ok(())
    }

    /// Uncordon a node (mark schedulable)
    pub fn uncordon_node(&self, name: &str) -> Result<(), ControllerError> {
        let mut node = self
            .nodes
            .get_mut(name)
            .ok_or_else(|| ControllerError::NodeNotFound(name.to_string()))?;
        node.spec.schedulable = true;
        Ok(())
    }

    /// Check for stale nodes and mark them as unknown
    pub async fn check_node_health(&self) {
        let config = self.config.read().await;
        let threshold = config.node_heartbeat_timeout;
        drop(config);

        for mut node in self.nodes.iter_mut() {
            if let Some(status) = &mut node.status {
                if status.is_stale(threshold) {
                    status.phase = NodePhase::Unknown;
                }
            }
        }
    }

    /// Add a pipeline to a node's tracked pipelines
    pub fn add_pipeline_to_node(
        &self,
        node_name: &str,
        namespace: &str,
        name: &str,
        port: u16,
    ) -> Result<(), ControllerError> {
        let mut node = self
            .nodes
            .get_mut(node_name)
            .ok_or_else(|| ControllerError::NodeNotFound(node_name.to_string()))?;

        let status = node.status.get_or_insert_with(|| {
            NodeStatus::new(
                super::node::NodeCapacity::default(),
                super::node::NodeInfo::from_system(),
            )
        });

        // Check if already tracked
        let already_exists = status
            .pipelines
            .iter()
            .any(|p| p.namespace == namespace && p.name == name);

        if !already_exists {
            status.pipelines.push(NodePipelineInfo {
                name: name.to_string(),
                namespace: namespace.to_string(),
                port,
                status: ReplicaStatus::Running,
            });
        }

        Ok(())
    }

    /// Remove a pipeline from a node's tracked pipelines
    pub fn remove_pipeline_from_node(
        &self,
        node_name: &str,
        namespace: &str,
        name: &str,
    ) -> Result<(), ControllerError> {
        let mut node = self
            .nodes
            .get_mut(node_name)
            .ok_or_else(|| ControllerError::NodeNotFound(node_name.to_string()))?;

        if let Some(status) = &mut node.status {
            status
                .pipelines
                .retain(|p| !(p.namespace == namespace && p.name == name));
        }

        Ok(())
    }

    // =========================================================================
    // Namespace Management
    // =========================================================================

    /// Create a namespace
    pub fn create_namespace(&self, ns: Namespace) -> Result<(), ControllerError> {
        if self.namespaces.contains_key(&ns.metadata.name) {
            return Ok(()); // Idempotent
        }
        self.namespaces.insert(ns.metadata.name.clone(), ns);
        Ok(())
    }

    /// List all namespaces
    pub fn list_namespaces(&self) -> Vec<Namespace> {
        self.namespaces.iter().map(|r| r.clone()).collect()
    }

    // =========================================================================
    // Pipeline Management
    // =========================================================================

    /// Deploy a pipeline
    pub fn deploy_pipeline(&self, mut pipeline: Pipeline) -> Result<Pipeline, ControllerError> {
        let qualified_name = pipeline.qualified_name();

        // Ensure namespace exists
        if !self.namespaces.contains_key(&pipeline.metadata.namespace) {
            self.create_namespace(Namespace::new(&pipeline.metadata.namespace))?;
        }

        // Check if already exists
        if self.pipelines.contains_key(&qualified_name) {
            return Err(ControllerError::PipelineExists(
                pipeline.metadata.name.clone(),
                pipeline.metadata.namespace.clone(),
            ));
        }

        // Initialize status
        pipeline.status = Some(PipelineStatus::initial());

        // Store pipeline
        self.pipelines.insert(qualified_name, pipeline.clone());

        Ok(pipeline)
    }

    /// Update an existing pipeline
    pub fn update_pipeline(&self, pipeline: Pipeline) -> Result<Pipeline, ControllerError> {
        let qualified_name = pipeline.qualified_name();

        if !self.pipelines.contains_key(&qualified_name) {
            return Err(ControllerError::PipelineNotFound(
                pipeline.metadata.name.clone(),
                pipeline.metadata.namespace.clone(),
            ));
        }

        self.pipelines.insert(qualified_name, pipeline.clone());
        Ok(pipeline)
    }

    /// Delete a pipeline
    pub fn delete_pipeline(
        &self,
        namespace: &str,
        name: &str,
    ) -> Result<Pipeline, ControllerError> {
        let qualified_name = format!("{}/{}", namespace, name);
        self.pipelines
            .remove(&qualified_name)
            .map(|(_, p)| p)
            .ok_or_else(|| {
                ControllerError::PipelineNotFound(name.to_string(), namespace.to_string())
            })
    }

    /// Get a pipeline by name
    pub fn get_pipeline(&self, namespace: &str, name: &str) -> Option<Pipeline> {
        let qualified_name = format!("{}/{}", namespace, name);
        self.pipelines.get(&qualified_name).map(|r| r.clone())
    }

    /// List all pipelines in a namespace
    pub fn list_pipelines(&self, namespace: &str) -> Vec<Pipeline> {
        let prefix = format!("{}/", namespace);
        self.pipelines
            .iter()
            .filter(|r| r.key().starts_with(&prefix))
            .map(|r| r.clone())
            .collect()
    }

    /// List all pipelines across all namespaces
    pub fn list_all_pipelines(&self) -> Vec<Pipeline> {
        self.pipelines.iter().map(|r| r.clone()).collect()
    }

    /// Scale a pipeline
    pub fn scale_pipeline(
        &self,
        namespace: &str,
        name: &str,
        replicas: u32,
    ) -> Result<Pipeline, ControllerError> {
        let qualified_name = format!("{}/{}", namespace, name);

        let mut pipeline = self.pipelines.get_mut(&qualified_name).ok_or_else(|| {
            ControllerError::PipelineNotFound(name.to_string(), namespace.to_string())
        })?;

        pipeline.spec.replicas = replicas;

        Ok(pipeline.clone())
    }

    /// Update pipeline status
    pub fn update_pipeline_status(
        &self,
        namespace: &str,
        name: &str,
        status: PipelineStatus,
    ) -> Result<(), ControllerError> {
        let qualified_name = format!("{}/{}", namespace, name);

        let mut pipeline = self.pipelines.get_mut(&qualified_name).ok_or_else(|| {
            ControllerError::PipelineNotFound(name.to_string(), namespace.to_string())
        })?;

        pipeline.status = Some(status);

        Ok(())
    }

    // =========================================================================
    // Scheduling
    // =========================================================================

    /// Score-based scheduler for pipeline replicas
    ///
    /// Distributes replicas preferring nodes with higher scores (more available
    /// resources). Falls back to round-robin if no scores are available.
    ///
    /// Returns a map of node name -> number of replicas to schedule
    pub fn schedule_replicas(
        &self,
        pipeline: &Pipeline,
    ) -> Result<HashMap<String, u32>, ControllerError> {
        let selector = &pipeline.spec.node_selector;
        let mut nodes: Vec<Node> = if selector.is_empty() {
            self.get_schedulable_nodes()
        } else {
            let label_selector = LabelSelector {
                match_labels: selector.clone(),
            };
            self.list_nodes_by_selector(&label_selector)
                .into_iter()
                .filter(|n| n.can_schedule() && n.has_capacity())
                .collect()
        };

        if nodes.is_empty() {
            return Err(ControllerError::NoAvailableNodes);
        }

        // Sort nodes by score (highest first)
        nodes.sort_by(|a, b| {
            let score_a = a
                .status
                .as_ref()
                .and_then(|s| s.score.as_ref())
                .map(|s| s.score)
                .unwrap_or(50.0); // Default score if no metrics
            let score_b = b
                .status
                .as_ref()
                .and_then(|s| s.score.as_ref())
                .map(|s| s.score)
                .unwrap_or(50.0);
            score_b
                .partial_cmp(&score_a)
                .unwrap_or(std::cmp::Ordering::Equal)
        });

        let replicas = pipeline.spec.replicas;
        let mut schedule: HashMap<String, u32> = HashMap::new();

        // Calculate total score for weighted distribution
        let total_score: f64 = nodes
            .iter()
            .map(|n| {
                n.status
                    .as_ref()
                    .and_then(|s| s.score.as_ref())
                    .map(|s| s.score)
                    .unwrap_or(50.0)
            })
            .sum();

        // Check if we have meaningful scores
        let has_meaningful_scores = nodes
            .iter()
            .any(|n| n.status.as_ref().and_then(|s| s.score.as_ref()).is_some());

        if !has_meaningful_scores || total_score == 0.0 {
            // Fallback to round-robin if no scores
            for i in 0..replicas {
                let node = &nodes[i as usize % nodes.len()];
                *schedule.entry(node.metadata.name.clone()).or_insert(0) += 1;
            }
        } else {
            // Weighted distribution based on scores
            // Nodes with higher scores get proportionally more replicas
            let mut remaining = replicas;

            for (i, node) in nodes.iter().enumerate() {
                if remaining == 0 {
                    break;
                }

                let score = node
                    .status
                    .as_ref()
                    .and_then(|s| s.score.as_ref())
                    .map(|s| s.score)
                    .unwrap_or(50.0);

                // Calculate fair share based on score proportion
                let share = ((score / total_score) * replicas as f64).round() as u32;

                // Ensure we assign at least some to good nodes, and handle remainder
                let to_assign = if i == nodes.len() - 1 {
                    remaining // Last node gets whatever is left
                } else {
                    share.min(remaining)
                };

                if to_assign > 0 {
                    schedule.insert(node.metadata.name.clone(), to_assign);
                    remaining = remaining.saturating_sub(to_assign);
                }
            }

            // If we still have remaining (due to rounding), assign to best node
            if remaining > 0 {
                let best_node = &nodes[0];
                *schedule.entry(best_node.metadata.name.clone()).or_insert(0) += remaining;
            }
        }

        Ok(schedule)
    }

    // =========================================================================
    // Cluster Stats
    // =========================================================================

    /// Get cluster statistics
    pub fn cluster_stats(&self) -> ClusterStats {
        let total_nodes = self.nodes.len();
        let ready_nodes = self.nodes.iter().filter(|r| r.is_ready()).count();
        let total_pipelines = self.pipelines.len();
        let ready_pipelines = self.pipelines.iter().filter(|r| r.is_ready()).count();

        ClusterStats {
            total_nodes,
            ready_nodes,
            total_pipelines,
            ready_pipelines,
            namespaces: self.namespaces.len(),
        }
    }
}

impl Default for ClusterController {
    fn default() -> Self {
        Self::new()
    }
}

/// Cluster statistics
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ClusterStats {
    pub total_nodes: usize,
    pub ready_nodes: usize,
    pub total_pipelines: usize,
    pub ready_pipelines: usize,
    pub namespaces: usize,
}

use serde::{Deserialize, Serialize};

#[cfg(test)]
mod tests {
    use super::*;
    use crate::cluster::node::{NodeCapacity, NodeInfo};
    use crate::config::Composition;

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

    fn create_test_node(name: &str) -> Node {
        let mut node = Node::new(name, "localhost");
        node.status = Some(NodeStatus::new(
            NodeCapacity::default(),
            NodeInfo::from_system(),
        ));
        node
    }

    #[test]
    fn test_register_node() {
        let controller = ClusterController::new();
        let node = create_test_node("node-1");

        controller.register_node(node.clone()).unwrap();

        let retrieved = controller.get_node("node-1");
        assert!(retrieved.is_some());
        assert_eq!(retrieved.unwrap().metadata.name, "node-1");
    }

    #[test]
    fn test_register_duplicate_node() {
        let controller = ClusterController::new();
        let node = create_test_node("node-1");

        controller.register_node(node.clone()).unwrap();
        let result = controller.register_node(node);

        assert!(matches!(result, Err(ControllerError::NodeExists(_))));
    }

    #[test]
    fn test_list_nodes() {
        let controller = ClusterController::new();
        controller
            .register_node(create_test_node("node-1"))
            .unwrap();
        controller
            .register_node(create_test_node("node-2"))
            .unwrap();

        let nodes = controller.list_nodes();
        assert_eq!(nodes.len(), 2);
    }

    #[test]
    fn test_deploy_pipeline() {
        let controller = ClusterController::new();
        let comp = create_test_composition();
        let pipeline = Pipeline::new("my-pipeline", comp);

        let deployed = controller.deploy_pipeline(pipeline).unwrap();

        assert_eq!(deployed.metadata.name, "my-pipeline");
        assert!(deployed.status.is_some());
    }

    #[test]
    fn test_deploy_duplicate_pipeline() {
        let controller = ClusterController::new();
        let comp = create_test_composition();
        let pipeline = Pipeline::new("my-pipeline", comp.clone());

        controller.deploy_pipeline(pipeline).unwrap();

        let pipeline2 = Pipeline::new("my-pipeline", comp);
        let result = controller.deploy_pipeline(pipeline2);

        assert!(matches!(result, Err(ControllerError::PipelineExists(_, _))));
    }

    #[test]
    fn test_get_pipeline() {
        let controller = ClusterController::new();
        let comp = create_test_composition();
        let pipeline = Pipeline::new("test", comp).with_namespace("prod");

        controller.deploy_pipeline(pipeline).unwrap();

        let retrieved = controller.get_pipeline("prod", "test");
        assert!(retrieved.is_some());
    }

    #[test]
    fn test_delete_pipeline() {
        let controller = ClusterController::new();
        let comp = create_test_composition();
        let pipeline = Pipeline::new("test", comp);

        controller.deploy_pipeline(pipeline).unwrap();

        let deleted = controller.delete_pipeline("default", "test");
        assert!(deleted.is_ok());

        let retrieved = controller.get_pipeline("default", "test");
        assert!(retrieved.is_none());
    }

    #[test]
    fn test_scale_pipeline() {
        let controller = ClusterController::new();
        let comp = create_test_composition();
        let pipeline = Pipeline::new("test", comp);

        controller.deploy_pipeline(pipeline).unwrap();

        let scaled = controller.scale_pipeline("default", "test", 5).unwrap();
        assert_eq!(scaled.spec.replicas, 5);
    }

    #[test]
    fn test_schedule_replicas() {
        let controller = ClusterController::new();

        // Register some nodes
        controller
            .register_node(create_test_node("node-1"))
            .unwrap();
        controller
            .register_node(create_test_node("node-2"))
            .unwrap();

        let comp = create_test_composition();
        let pipeline = Pipeline::new("test", comp).with_replicas(4);

        let schedule = controller.schedule_replicas(&pipeline).unwrap();

        // Should distribute across 2 nodes
        assert_eq!(schedule.values().sum::<u32>(), 4);
        assert!(schedule.len() <= 2);
    }

    #[test]
    fn test_schedule_no_nodes() {
        let controller = ClusterController::new();
        let comp = create_test_composition();
        let pipeline = Pipeline::new("test", comp);

        let result = controller.schedule_replicas(&pipeline);
        assert!(matches!(result, Err(ControllerError::NoAvailableNodes)));
    }

    #[test]
    fn test_cordon_uncordon() {
        let controller = ClusterController::new();
        controller
            .register_node(create_test_node("node-1"))
            .unwrap();

        controller.cordon_node("node-1").unwrap();
        let node = controller.get_node("node-1").unwrap();
        assert!(!node.spec.schedulable);

        controller.uncordon_node("node-1").unwrap();
        let node = controller.get_node("node-1").unwrap();
        assert!(node.spec.schedulable);
    }

    #[test]
    fn test_cluster_stats() {
        let controller = ClusterController::new();
        controller
            .register_node(create_test_node("node-1"))
            .unwrap();

        let comp = create_test_composition();
        controller
            .deploy_pipeline(Pipeline::new("p1", comp.clone()))
            .unwrap();
        controller
            .deploy_pipeline(Pipeline::new("p2", comp))
            .unwrap();

        let stats = controller.cluster_stats();
        assert_eq!(stats.total_nodes, 1);
        assert_eq!(stats.total_pipelines, 2);
    }

    #[test]
    fn test_default_namespace_created() {
        let controller = ClusterController::new();
        let namespaces = controller.list_namespaces();

        assert!(namespaces.iter().any(|ns| ns.metadata.name == "default"));
    }
}
