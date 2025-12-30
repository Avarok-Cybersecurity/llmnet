use std::sync::Arc;

use dashmap::DashMap;
use uuid::Uuid;

use crate::config::Composition;
use crate::runtime::{PipelineProcessor, PipelineRequest, RuntimeNode, SharedRunnerManager};

/// Shared application state
#[derive(Clone)]
pub struct AppState {
    pub composition: Arc<Composition>,
    pub nodes: Arc<DashMap<String, RuntimeNode>>,
    pub active_requests: Arc<DashMap<Uuid, PipelineRequest>>,
    pub processor: Option<Arc<PipelineProcessor>>,
    pub runner_manager: Option<SharedRunnerManager>,
}

impl AppState {
    pub fn new(composition: Composition) -> Self {
        let nodes = Arc::new(DashMap::new());

        // Build runtime nodes
        let mut port_offset = 0u16;
        for arch_node in &composition.architecture {
            let model_config = arch_node
                .model
                .as_ref()
                .and_then(|m| composition.models.get(m))
                .cloned();

            let runtime = RuntimeNode::from_architecture(arch_node, model_config, port_offset);
            nodes.insert(runtime.name.clone(), runtime);
            port_offset += 1;
        }

        // Create pipeline processor
        let processor = PipelineProcessor::new(&composition).ok().map(Arc::new);

        Self {
            composition: Arc::new(composition),
            nodes,
            active_requests: Arc::new(DashMap::new()),
            processor,
            runner_manager: None,
        }
    }

    /// Create with a runner manager for worker mode
    pub fn with_runner_manager(mut self, manager: SharedRunnerManager) -> Self {
        self.runner_manager = Some(manager);
        self
    }

    /// Get the router node (layer 0)
    pub fn router_node(&self) -> Option<RuntimeNode> {
        self.nodes.iter().find(|r| r.layer == 0).map(|r| r.clone())
    }

    /// Register an active request
    pub fn register_request(&self, request: PipelineRequest) {
        self.active_requests.insert(request.request_id, request);
    }

    /// Complete and remove a request
    pub fn complete_request(&self, id: &Uuid) -> Option<PipelineRequest> {
        self.active_requests.remove(id).map(|(_, r)| r)
    }

    /// Get active request count
    pub fn active_request_count(&self) -> usize {
        self.active_requests.len()
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
    fn test_app_state_creation() {
        let comp = create_test_composition();
        let state = AppState::new(comp);

        assert_eq!(state.nodes.len(), 2);
        assert!(state.router_node().is_some());
    }

    #[test]
    fn test_request_tracking() {
        let comp = create_test_composition();
        let state = AppState::new(comp);

        let request = PipelineRequest::new("test".to_string());
        let id = request.request_id;

        state.register_request(request);
        assert_eq!(state.active_request_count(), 1);

        let completed = state.complete_request(&id);
        assert!(completed.is_some());
        assert_eq!(state.active_request_count(), 0);
    }
}
