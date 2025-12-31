use std::collections::HashMap;
use std::sync::Arc;

use thiserror::Error;

use crate::client::OpenAiClientTrait;
use crate::config::{Composition, OutputTarget};
use crate::runtime::node::{evaluate_condition, RuntimeNode};
use crate::runtime::request::PipelineRequest;
use crate::runtime::router::{NodeMetadata, Router, RouterError};

#[derive(Error, Debug)]
pub enum OrchestratorError {
    #[error("Router error: {0}")]
    Router(#[from] RouterError),

    #[error("No nodes found in layer {0}")]
    EmptyLayer(u32),

    #[error("Node '{0}' not found")]
    NodeNotFound(String),

    #[error("No output node reached")]
    NoOutput,

    #[error("Maximum hops exceeded: {0}")]
    MaxHopsExceeded(usize),
}

/// The main orchestrator that routes requests through the pipeline
pub struct Orchestrator<C: OpenAiClientTrait> {
    #[allow(dead_code)]
    composition: Arc<Composition>,
    nodes: HashMap<String, RuntimeNode>,
    router: Router<C>,
    max_hops: usize,
}

impl<C: OpenAiClientTrait> Orchestrator<C> {
    pub fn new(composition: Composition, router_client: C, router_model: String) -> Self {
        let nodes = build_runtime_nodes(&composition);
        Self {
            composition: Arc::new(composition),
            nodes,
            router: Router::new(router_client, router_model),
            max_hops: 50, // Reasonable limit to prevent infinite loops
        }
    }

    /// Process a request through the pipeline
    pub async fn process(&self, mut request: PipelineRequest) -> Result<String, OrchestratorError> {
        let mut current_layer = 0u32;
        let mut hop_count = 0;

        loop {
            if hop_count >= self.max_hops {
                return Err(OrchestratorError::MaxHopsExceeded(self.max_hops));
            }

            // Get nodes in current layer
            let layer_nodes = self.get_layer_nodes(current_layer);
            if layer_nodes.is_empty() {
                return Err(OrchestratorError::EmptyLayer(current_layer));
            }

            // If we're at the router layer (0), use routing to pick a node
            let selected_node = if current_layer == 0 && layer_nodes.len() == 1 {
                // Single router node at layer 0
                let router_node = &layer_nodes[0];

                // Get next layer's nodes for routing decision
                if let Some(target_layer) = self.get_target_layer(router_node) {
                    let next_nodes = self.get_layer_nodes(target_layer);
                    let metadata: Vec<NodeMetadata> =
                        next_nodes.iter().map(|n| (*n).into()).collect();

                    let selected = self
                        .router
                        .route(&request.current_content, &metadata)
                        .await?;

                    request.add_hop(
                        router_node.name.clone(),
                        current_layer,
                        Some(selected.clone()),
                    );
                    hop_count += 1;

                    // Find the selected node
                    self.nodes
                        .get(&selected)
                        .ok_or(OrchestratorError::NodeNotFound(selected))?
                } else {
                    router_node
                }
            } else {
                // For non-router layers with multiple nodes, we need selection logic
                // For now, pick the first node that passes its condition
                let node = layer_nodes
                    .iter()
                    .find(|n| {
                        n.condition
                            .as_ref()
                            .map(|c| evaluate_condition(c, request.get_variables()))
                            .unwrap_or(true)
                    })
                    .ok_or(OrchestratorError::EmptyLayer(current_layer))?;
                *node
            };

            // Check if we've reached an output node
            if selected_node.is_output() {
                request.add_hop(selected_node.name.clone(), selected_node.layer, None);
                return Ok(request.current_content);
            }

            // Record the hop
            if current_layer != 0 {
                request.add_hop(selected_node.name.clone(), current_layer, None);
                hop_count += 1;
            }

            // Determine next layer from output_targets
            match &selected_node.output_targets {
                Some(OutputTarget::Layers(layers)) => {
                    if let Some(&next) = layers.first() {
                        current_layer = next;
                    } else {
                        return Err(OrchestratorError::NoOutput);
                    }
                }
                Some(OutputTarget::Nodes(nodes)) => {
                    // Find the first target that exists and passes conditions
                    let target = nodes
                        .iter()
                        .find_map(|name| {
                            let node = self.nodes.get(name)?;
                            if node.is_output() {
                                return Some(node);
                            }
                            if node
                                .condition
                                .as_ref()
                                .map(|c| evaluate_condition(c, request.get_variables()))
                                .unwrap_or(true)
                            {
                                Some(node)
                            } else {
                                None
                            }
                        })
                        .ok_or(OrchestratorError::NoOutput)?;

                    if target.is_output() {
                        request.add_hop(target.name.clone(), target.layer, None);
                        return Ok(request.current_content);
                    }

                    current_layer = target.layer;
                }
                None => {
                    return Err(OrchestratorError::NoOutput);
                }
            }
        }
    }

    fn get_layer_nodes(&self, layer: u32) -> Vec<&RuntimeNode> {
        self.nodes
            .values()
            .filter(|n| n.layer == layer && !n.is_output())
            .collect()
    }

    fn get_target_layer(&self, node: &RuntimeNode) -> Option<u32> {
        match &node.output_targets {
            Some(OutputTarget::Layers(layers)) => layers.first().copied(),
            _ => None,
        }
    }
}

// ============================================================================
// SBIO: Pure function for building runtime nodes
// ============================================================================

/// Build runtime nodes from composition.
/// Pure function - no I/O.
fn build_runtime_nodes(composition: &Composition) -> HashMap<String, RuntimeNode> {
    let mut nodes = HashMap::new();

    for (port_offset, arch_node) in composition.architecture.iter().enumerate() {
        let port_offset = port_offset as u16;
        let model_config = arch_node
            .model
            .as_ref()
            .and_then(|m| composition.models.get(m))
            .cloned();

        let runtime = RuntimeNode::from_architecture(arch_node, model_config, port_offset);
        nodes.insert(runtime.name.clone(), runtime);
    }

    nodes
}

impl From<&RuntimeNode> for NodeMetadata {
    fn from(node: &RuntimeNode) -> Self {
        Self {
            name: node.name.clone(),
            use_case: node.use_case.clone(),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::client::openai::mock::MockOpenAiClient;

    fn create_test_composition() -> Composition {
        let json = r#"{
            "models": {
                "router-model": {
                    "type": "external",
                    "interface": "openai-api",
                    "url": "http://localhost:8080"
                }
            },
            "architecture": [
                {
                    "name": "router",
                    "layer": 0,
                    "model": "router-model",
                    "adapter": "openai-api",
                    "output-to": [1]
                },
                {
                    "name": "handler-a",
                    "layer": 1,
                    "adapter": "openai-api",
                    "use-case": "Handle type A requests",
                    "output-to": ["final-output"]
                },
                {
                    "name": "handler-b",
                    "layer": 1,
                    "adapter": "openai-api",
                    "use-case": "Handle type B requests",
                    "output-to": ["final-output"]
                },
                {
                    "name": "final-output",
                    "adapter": "output"
                }
            ]
        }"#;

        Composition::from_str(json).unwrap()
    }

    #[test]
    fn test_build_runtime_nodes() {
        let composition = create_test_composition();
        let nodes = build_runtime_nodes(&composition);

        assert_eq!(nodes.len(), 4);
        assert!(nodes.contains_key("router"));
        assert!(nodes.contains_key("handler-a"));
        assert!(nodes.contains_key("final-output"));
    }

    #[tokio::test]
    async fn test_orchestrator_routes_to_selected_node() {
        let composition = create_test_composition();
        let mock_client = MockOpenAiClient::new(vec!["handler-a".to_string()]);
        let orchestrator = Orchestrator::new(composition, mock_client, "test-model".to_string());

        let request = PipelineRequest::new("Process this request".to_string());
        let result = orchestrator.process(request).await.unwrap();

        assert_eq!(result, "Process this request");
    }

    #[tokio::test]
    async fn test_orchestrator_with_invalid_selection() {
        let composition = create_test_composition();
        let mock_client = MockOpenAiClient::new(vec!["nonexistent-node".to_string()]);
        let orchestrator = Orchestrator::new(composition, mock_client, "test-model".to_string());

        let request = PipelineRequest::new("Test".to_string());
        let result = orchestrator.process(request).await;

        // Should fail because the selected node doesn't exist
        assert!(result.is_err());
    }
}
