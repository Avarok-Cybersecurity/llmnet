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
        // Step 1: Get router node and find next layer handlers
        let router_node = self
            .nodes
            .get(&self.router_node_name)
            .ok_or(ProcessorError::NoRouter)?;

        let target_layer = match &router_node.output_targets {
            Some(OutputTarget::Layers(layers)) => layers.first().copied().unwrap_or(1),
            _ => 1,
        };

        // Step 2: Get handler nodes metadata for routing
        let handler_nodes: Vec<&RuntimeNode> = self
            .nodes
            .values()
            .filter(|n| n.layer == target_layer && !n.is_output())
            .collect();

        if handler_nodes.is_empty() {
            return Err(ProcessorError::NoHandlers(target_layer));
        }

        let metadata: Vec<NodeMetadata> = handler_nodes
            .iter()
            .map(|n| NodeMetadata {
                name: n.name.clone(),
                use_case: n.use_case.clone(),
            })
            .collect();

        // Step 3: Call router to select a handler
        let router_client = self
            .clients
            .get(&self.router_node_name)
            .ok_or(ProcessorError::RouterModelNotConfigured)?;

        let routing_prompt = build_routing_prompt(user_message, &metadata);
        let router_request = ClientRequest {
            model: self.router_model_name.clone(),
            messages: vec![Message {
                role: "user".to_string(),
                content: routing_prompt,
            }],
            max_tokens: Some(100),
            temperature: Some(0.1),
        };

        let router_response = router_client
            .chat_completion(&router_request)
            .await
            .map_err(|e| ProcessorError::ApiError(e.to_string()))?;

        let router_output = router_response
            .choices
            .first()
            .map(|c| c.message.content.clone())
            .unwrap_or_default();

        let selected_handler = extract_node_selection(&router_output, &metadata)
            .map_err(|e| ProcessorError::ApiError(e.to_string()))?;

        // Step 4: Call the selected handler
        let handler_node = self
            .nodes
            .get(&selected_handler)
            .ok_or_else(|| ProcessorError::HandlerNotFound(selected_handler.clone()))?;

        let handler_client = self
            .clients
            .get(&selected_handler)
            .ok_or_else(|| ProcessorError::HandlerNoModel(selected_handler.clone()))?;

        let handler_model = handler_node
            .model_override()
            .unwrap_or_else(|| selected_handler.clone());

        let handler_request = ClientRequest {
            model: handler_model,
            messages: vec![Message {
                role: "user".to_string(),
                content: user_message.to_string(),
            }],
            max_tokens: Some(1024),
            temperature: Some(0.7),
        };

        let handler_response = handler_client
            .chat_completion(&handler_request)
            .await
            .map_err(|e| ProcessorError::ApiError(e.to_string()))?;

        let final_response = handler_response
            .choices
            .first()
            .map(|c| c.message.content.clone())
            .unwrap_or_else(|| "No response generated".to_string());

        Ok(final_response)
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
}
