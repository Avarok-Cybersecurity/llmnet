use std::collections::HashMap;

use thiserror::Error;

use crate::client::{ChatCompletionRequest as ClientRequest, Message, OpenAiClient, OpenAiClientTrait};
use crate::config::{Composition, ModelDefinition, OutputTarget};
use crate::runtime::node::RuntimeNode;
use crate::runtime::router::{build_routing_prompt, extract_node_selection, NodeMetadata};

#[derive(Error, Debug)]
pub enum ProcessorError {
    #[error("No router node found at layer 0")]
    NoRouter,

    #[error("Router model not configured")]
    RouterModelNotConfigured,

    #[error("No handler nodes found in layer {0}")]
    NoHandlers(u32),

    #[error("Selected handler '{0}' not found")]
    HandlerNotFound(String),

    #[error("Handler '{0}' has no model configured")]
    HandlerNoModel(String),

    #[error("API error: {0}")]
    ApiError(String),

    #[error("Invalid model type for handler '{0}': expected external")]
    InvalidModelType(String),
}

/// Processes requests through the LLM pipeline
pub struct PipelineProcessor {
    nodes: HashMap<String, RuntimeNode>,
    clients: HashMap<String, OpenAiClient>,
    router_node_name: String,
    router_model_name: String,
}

impl PipelineProcessor {
    /// Create a new pipeline processor from a composition
    pub fn new(composition: &Composition) -> Result<Self, ProcessorError> {
        let mut nodes = HashMap::new();
        let mut clients = HashMap::new();
        let mut router_node_name = None;
        let mut router_model_name = None;

        // Build nodes and clients
        let mut port_offset = 0u16;
        for arch_node in &composition.architecture {
            let model_config = arch_node
                .model
                .as_ref()
                .and_then(|m| composition.models.get(m))
                .cloned();

            let runtime = RuntimeNode::from_architecture(arch_node, model_config.clone(), port_offset);

            // Create client for nodes with external models
            if let Some(ModelDefinition::External(ext)) = &model_config {
                let model_name = runtime.model_override().unwrap_or_else(|| {
                    arch_node.model.clone().unwrap_or_else(|| "default".to_string())
                });

                let client = OpenAiClient::new(
                    ext.url.clone(),
                    ext.api_key.clone(),
                    model_name,
                );
                clients.insert(runtime.name.clone(), client);
            }

            // Track router node
            if arch_node.layer == Some(0) && arch_node.output_to.is_some() {
                router_node_name = Some(runtime.name.clone());
                router_model_name = runtime.model_override().or_else(|| {
                    arch_node.model.clone()
                });
            }

            nodes.insert(runtime.name.clone(), runtime);
            port_offset += 1;
        }

        let router_node_name = router_node_name.ok_or(ProcessorError::NoRouter)?;
        let router_model_name = router_model_name.ok_or(ProcessorError::RouterModelNotConfigured)?;

        Ok(Self {
            nodes,
            clients,
            router_node_name,
            router_model_name,
        })
    }

    /// Process a user message through the pipeline
    pub async fn process(&self, user_message: &str) -> Result<String, ProcessorError> {
        let mut current_content = user_message.to_string();
        let mut current_node_name = self.router_node_name.clone();
        let mut hop_count = 0;
        const MAX_HOPS: usize = 10;

        loop {
            if hop_count >= MAX_HOPS {
                return Err(ProcessorError::ApiError(format!(
                    "Maximum hops ({}) exceeded",
                    MAX_HOPS
                )));
            }
            hop_count += 1;

            let current_node = self
                .nodes
                .get(&current_node_name)
                .ok_or_else(|| ProcessorError::HandlerNotFound(current_node_name.clone()))?;

            // Determine next targets
            let next_targets = self.get_next_targets(current_node)?;

            // If multiple targets, we need to route
            let selected_target = if next_targets.len() > 1 {
                self.route_to_target(&current_node_name, &current_content, &next_targets)
                    .await?
            } else if next_targets.len() == 1 {
                next_targets[0].clone()
            } else {
                return Err(ProcessorError::ApiError("No next targets found".to_string()));
            };

            // Check if we've reached output
            if let Some(target_node) = self.nodes.get(&selected_target) {
                if target_node.is_output() {
                    return Ok(current_content);
                }
            }

            // Call the selected node's LLM
            current_content = self.call_node_llm(&selected_target, &current_content).await?;
            current_node_name = selected_target;
        }
    }

    /// Get the next target node names from a node's output_targets
    fn get_next_targets(&self, node: &RuntimeNode) -> Result<Vec<String>, ProcessorError> {
        match &node.output_targets {
            Some(OutputTarget::Layers(layers)) => {
                // Get all nodes in the target layers
                let mut targets = Vec::new();
                for layer in layers {
                    let layer_nodes: Vec<String> = self
                        .nodes
                        .values()
                        .filter(|n| n.layer == *layer && !n.is_output())
                        .map(|n| n.name.clone())
                        .collect();
                    targets.extend(layer_nodes);
                }
                if targets.is_empty() {
                    // Check for output nodes if no handler nodes found
                    for layer in layers {
                        let output_nodes: Vec<String> = self
                            .nodes
                            .values()
                            .filter(|n| n.layer == *layer && n.is_output())
                            .map(|n| n.name.clone())
                            .collect();
                        targets.extend(output_nodes);
                    }
                }
                Ok(targets)
            }
            Some(OutputTarget::Nodes(nodes)) => Ok(nodes.clone()),
            None => Err(ProcessorError::ApiError(
                "Node has no output targets".to_string(),
            )),
        }
    }

    /// Route to select one target from multiple options
    async fn route_to_target(
        &self,
        router_name: &str,
        content: &str,
        targets: &[String],
    ) -> Result<String, ProcessorError> {
        let router_client = self
            .clients
            .get(router_name)
            .ok_or_else(|| ProcessorError::HandlerNoModel(router_name.to_string()))?;

        let router_node = self
            .nodes
            .get(router_name)
            .ok_or_else(|| ProcessorError::HandlerNotFound(router_name.to_string()))?;

        let metadata: Vec<NodeMetadata> = targets
            .iter()
            .filter_map(|name| self.nodes.get(name))
            .map(|n| NodeMetadata {
                name: n.name.clone(),
                use_case: n.use_case.clone(),
            })
            .collect();

        if metadata.is_empty() {
            return Err(ProcessorError::ApiError("No valid targets for routing".to_string()));
        }

        let routing_prompt = build_routing_prompt(content, &metadata);
        let model = router_node
            .model_override()
            .unwrap_or_else(|| self.router_model_name.clone());

        let request = ClientRequest {
            model,
            messages: vec![Message {
                role: "user".to_string(),
                content: routing_prompt,
            }],
            max_tokens: Some(100),
            temperature: Some(0.1),
        };

        let response = router_client
            .chat_completion(&request)
            .await
            .map_err(|e| ProcessorError::ApiError(e.to_string()))?;

        let output = response
            .choices
            .first()
            .map(|c| c.message.content.clone())
            .unwrap_or_default();

        extract_node_selection(&output, &metadata)
            .map_err(|e| ProcessorError::ApiError(e.to_string()))
    }

    /// Call a node's LLM with content
    async fn call_node_llm(&self, node_name: &str, content: &str) -> Result<String, ProcessorError> {
        let node = self
            .nodes
            .get(node_name)
            .ok_or_else(|| ProcessorError::HandlerNotFound(node_name.to_string()))?;

        let client = self
            .clients
            .get(node_name)
            .ok_or_else(|| ProcessorError::HandlerNoModel(node_name.to_string()))?;

        let model = node.model_override().unwrap_or_else(|| node_name.to_string());

        let request = ClientRequest {
            model,
            messages: vec![Message {
                role: "user".to_string(),
                content: content.to_string(),
            }],
            max_tokens: Some(1024),
            temperature: Some(0.7),
        };

        let response = client
            .chat_completion(&request)
            .await
            .map_err(|e| ProcessorError::ApiError(e.to_string()))?;

        Ok(response
            .choices
            .first()
            .map(|c| c.message.content.clone())
            .unwrap_or_else(|| "No response generated".to_string()))
    }

    /// Get number of nodes
    pub fn node_count(&self) -> usize {
        self.nodes.len()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_processor_creation() {
        let json = r#"{
            "models": {
                "router-model": {
                    "type": "external",
                    "interface": "openai-api",
                    "url": "http://localhost:8080"
                },
                "handler-model": {
                    "type": "external",
                    "interface": "openai-api",
                    "url": "http://localhost:8081"
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
                    "name": "handler",
                    "layer": 1,
                    "model": "handler-model",
                    "adapter": "openai-api",
                    "use-case": "General handler",
                    "output-to": ["output"]
                },
                {
                    "name": "output",
                    "adapter": "output"
                }
            ]
        }"#;

        let comp = Composition::from_str(json).unwrap();
        let processor = PipelineProcessor::new(&comp).unwrap();

        assert_eq!(processor.node_count(), 3);
        assert_eq!(processor.router_node_name, "router");
    }

    #[test]
    fn test_processor_no_router_model() {
        // A composition with a router that has no model configured
        let json = r#"{
            "models": {},
            "architecture": [
                {
                    "name": "router",
                    "layer": 0,
                    "adapter": "openai-api",
                    "output-to": [1]
                },
                {
                    "name": "handler",
                    "layer": 1,
                    "adapter": "openai-api",
                    "output-to": ["output"]
                },
                {"name": "output", "adapter": "output"}
            ]
        }"#;

        let comp = Composition::from_str(json).unwrap();
        let result = PipelineProcessor::new(&comp);

        // Should fail because router has no model configured
        assert!(matches!(result, Err(ProcessorError::RouterModelNotConfigured)));
    }

    #[test]
    fn test_processor_multi_layer_creation() {
        // Test 1-2-1-1 topology: router -> 2 handlers -> 1 aggregator -> output
        let json = r#"{
            "models": {
                "router-model": {
                    "type": "external",
                    "interface": "openai-api",
                    "url": "http://localhost:8080"
                },
                "handler-model": {
                    "type": "external",
                    "interface": "openai-api",
                    "url": "http://localhost:8081"
                },
                "aggregator-model": {
                    "type": "external",
                    "interface": "openai-api",
                    "url": "http://localhost:8082"
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
                    "model": "handler-model",
                    "adapter": "openai-api",
                    "use-case": "Handle type A requests",
                    "output-to": [2]
                },
                {
                    "name": "handler-b",
                    "layer": 1,
                    "model": "handler-model",
                    "adapter": "openai-api",
                    "use-case": "Handle type B requests",
                    "output-to": [2]
                },
                {
                    "name": "aggregator",
                    "layer": 2,
                    "model": "aggregator-model",
                    "adapter": "openai-api",
                    "use-case": "Aggregate and refine responses",
                    "output-to": ["output"]
                },
                {
                    "name": "output",
                    "adapter": "output"
                }
            ]
        }"#;

        let comp = Composition::from_str(json).unwrap();
        let processor = PipelineProcessor::new(&comp).unwrap();

        assert_eq!(processor.node_count(), 5);
        assert_eq!(processor.router_node_name, "router");
    }

    #[test]
    fn test_get_next_targets_by_layer() {
        let json = r#"{
            "models": {
                "model": {
                    "type": "external",
                    "interface": "openai-api",
                    "url": "http://localhost:8080"
                }
            },
            "architecture": [
                {
                    "name": "router",
                    "layer": 0,
                    "model": "model",
                    "adapter": "openai-api",
                    "output-to": [1]
                },
                {
                    "name": "handler-a",
                    "layer": 1,
                    "model": "model",
                    "adapter": "openai-api",
                    "use-case": "A",
                    "output-to": ["output"]
                },
                {
                    "name": "handler-b",
                    "layer": 1,
                    "model": "model",
                    "adapter": "openai-api",
                    "use-case": "B",
                    "output-to": ["output"]
                },
                {"name": "output", "adapter": "output"}
            ]
        }"#;

        let comp = Composition::from_str(json).unwrap();
        let processor = PipelineProcessor::new(&comp).unwrap();
        let router = processor.nodes.get("router").unwrap();

        let targets = processor.get_next_targets(router).unwrap();
        assert_eq!(targets.len(), 2);
        assert!(targets.contains(&"handler-a".to_string()));
        assert!(targets.contains(&"handler-b".to_string()));
    }

    #[test]
    fn test_get_next_targets_by_node_name() {
        let json = r#"{
            "models": {
                "model": {
                    "type": "external",
                    "interface": "openai-api",
                    "url": "http://localhost:8080"
                }
            },
            "architecture": [
                {
                    "name": "router",
                    "layer": 0,
                    "model": "model",
                    "adapter": "openai-api",
                    "output-to": [1]
                },
                {
                    "name": "handler",
                    "layer": 1,
                    "model": "model",
                    "adapter": "openai-api",
                    "output-to": ["output"]
                },
                {"name": "output", "adapter": "output"}
            ]
        }"#;

        let comp = Composition::from_str(json).unwrap();
        let processor = PipelineProcessor::new(&comp).unwrap();
        let handler = processor.nodes.get("handler").unwrap();

        let targets = processor.get_next_targets(handler).unwrap();
        assert_eq!(targets.len(), 1);
        assert_eq!(targets[0], "output");
    }

    #[test]
    fn test_three_layer_pipeline_creation() {
        // Test 1-2-2-1 topology: router -> 2 domain handlers -> 2 refiners -> output
        let json = r#"{
            "models": {
                "model": {
                    "type": "external",
                    "interface": "openai-api",
                    "url": "http://localhost:8080"
                }
            },
            "architecture": [
                {
                    "name": "router",
                    "layer": 0,
                    "model": "model",
                    "adapter": "openai-api",
                    "output-to": [1]
                },
                {
                    "name": "sales",
                    "layer": 1,
                    "model": "model",
                    "adapter": "openai-api",
                    "use-case": "Sales queries",
                    "output-to": [2]
                },
                {
                    "name": "support",
                    "layer": 1,
                    "model": "model",
                    "adapter": "openai-api",
                    "use-case": "Support queries",
                    "output-to": [2]
                },
                {
                    "name": "formal-refiner",
                    "layer": 2,
                    "model": "model",
                    "adapter": "openai-api",
                    "use-case": "Formal tone refinement",
                    "output-to": ["output"]
                },
                {
                    "name": "casual-refiner",
                    "layer": 2,
                    "model": "model",
                    "adapter": "openai-api",
                    "use-case": "Casual tone refinement",
                    "output-to": ["output"]
                },
                {"name": "output", "adapter": "output"}
            ]
        }"#;

        let comp = Composition::from_str(json).unwrap();
        let processor = PipelineProcessor::new(&comp).unwrap();

        assert_eq!(processor.node_count(), 6);

        // Check layer 1 has 2 nodes
        let layer1_count = processor.nodes.values().filter(|n| n.layer == 1).count();
        assert_eq!(layer1_count, 2);

        // Check layer 2 has 2 nodes
        let layer2_count = processor.nodes.values().filter(|n| n.layer == 2).count();
        assert_eq!(layer2_count, 2);
    }
}
