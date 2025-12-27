//! # LLMNet Cluster Management
//!
//! This module provides Kubernetes-like orchestration for LLM pipelines.
//!
//! ## Design Philosophy: What We Mirror from K8s
//!
//! ### ✅ MIRRORED FEATURES
//!
//! 1. **Declarative Configuration**: JSON/YAML manifests declaring desired state
//! 2. **Contexts**: Switch between local/remote clusters (like kubeconfig)
//! 3. **Pipelines (≈ Deployments)**: The deployable unit with scaling
//! 4. **Namespaces**: Multi-tenant isolation for pipelines
//! 5. **Health Checks**: Liveness/readiness for nodes and pipelines
//! 6. **Horizontal Scaling**: Replicate pipelines across nodes
//! 7. **kubectl-like CLI**: `llmnet get`, `deploy`, `delete`, `logs`
//! 8. **Labels & Selectors**: Organize and query resources
//! 9. **Rollouts**: Gradual deployment updates
//!
//! ### ❌ EXCLUDED FEATURES (Too complex for LLM orchestration)
//!
//! 1. **Complex Scheduler**: No bin-packing, affinity, taints. Simple round-robin.
//! 2. **etcd/HA Control Plane**: Single leader, not distributed consensus.
//! 3. **CRDs**: No custom resource definitions (yet).
//! 4. **Network Policies**: All nodes in cluster can communicate.
//! 5. **Ingress Controllers**: Just expose OpenAI-compatible API.
//! 6. **RBAC**: Simple API key auth, not role-based.
//! 7. **StatefulSets/DaemonSets**: All pipelines are stateless.
//!
//! ## Core Resources
//!
//! - **Pipeline**: A deployable LLM routing configuration
//! - **Node**: A machine running LLMNet that can host pipelines
//! - **Namespace**: Logical isolation for pipelines
//!
//! ## Architecture
//!
//! ```text
//! ┌─────────────────────────────────────────────────────────────┐
//! │                    LLMNet Control Plane                      │
//! │                      (llmnet serve)                          │
//! │  ┌──────────────┐  ┌──────────────┐  ┌──────────────────┐  │
//! │  │ API Server   │  │ Pipeline     │  │ Node Registry    │  │
//! │  │ :8181        │  │ Controller   │  │                  │  │
//! │  └──────────────┘  └──────────────┘  └──────────────────┘  │
//! └─────────────────────────────────────────────────────────────┘
//!                              │
//!         ┌────────────────────┼────────────────────┐
//!         ▼                    ▼                    ▼
//!   ┌───────────┐        ┌───────────┐        ┌───────────┐
//!   │  Node 1   │        │  Node 2   │        │  Node 3   │
//!   │ (worker)  │        │ (worker)  │        │ (worker)  │
//!   │ :8080     │        │ :8080     │        │ :8080     │
//!   └───────────┘        └───────────┘        └───────────┘
//! ```

pub mod api;
pub mod controller;
pub mod node;
pub mod pipeline;
pub mod resources;

pub use api::{create_control_plane_router, ControlPlaneState};
pub use controller::{ClusterController, ClusterStats, ControllerConfig};
pub use node::{Node, NodeCondition, NodePhase, NodeStatus};
pub use pipeline::{Pipeline, PipelineCondition, PipelineSpec, PipelineStatus};
pub use resources::*;

/// Default control plane API port
pub const CONTROL_PLANE_PORT: u16 = 8181;

/// Default worker node API port
pub const WORKER_PORT: u16 = 8080;

/// Default heartbeat interval in seconds
pub const HEARTBEAT_INTERVAL_SECS: u64 = 30;
