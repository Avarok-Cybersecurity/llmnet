use serde::{Deserialize, Serialize};
use std::collections::HashMap;

/// Architecture node definition from the composition file
#[derive(Debug, Clone, PartialEq, Deserialize, Serialize)]
pub struct ArchitectureNode {
    pub name: String,

    #[serde(default)]
    pub layer: Option<u32>,

    /// Reference to a model in the models map
    pub model: Option<String>,

    /// Adapter type: "openai-api", "output", "ws"
    pub adapter: String,

    #[serde(rename = "bind-addr")]
    pub bind_addr: Option<String>,

    #[serde(rename = "bind-port")]
    pub bind_port: Option<String>,

    #[serde(rename = "output-to")]
    pub output_to: Option<OutputTarget>,

    /// Use-case description for routing decisions
    #[serde(rename = "use-case")]
    pub use_case: Option<String>,

    /// Conditional execution expression
    #[serde(rename = "if")]
    pub condition: Option<String>,

    /// WebSocket URL for "ws" adapter
    pub url: Option<String>,

    #[serde(rename = "extra-options", default)]
    pub extra_options: HashMap<String, serde_json::Value>,
}

/// Output target specification - can be layers or specific nodes
#[derive(Debug, Clone, PartialEq, Deserialize, Serialize)]
#[serde(untagged)]
pub enum OutputTarget {
    /// Output to specific layer numbers
    Layers(Vec<u32>),
    /// Output to specific node names
    Nodes(Vec<String>),
}

impl ArchitectureNode {
    /// Check if this is a router node (layer 0 with output-to)
    pub fn is_router(&self) -> bool {
        self.layer == Some(0) && self.output_to.is_some()
    }

    /// Check if this is an output node
    pub fn is_output(&self) -> bool {
        self.adapter == "output"
    }

    /// Get the bind address with default
    pub fn effective_bind_addr(&self) -> &str {
        self.bind_addr.as_deref().unwrap_or("0.0.0.0")
    }

    /// Get nodes in the next layer based on output-to
    pub fn get_target_layer(&self) -> Option<u32> {
        match &self.output_to {
            Some(OutputTarget::Layers(layers)) => layers.first().copied(),
            _ => None,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_parse_router_node() {
        let json = r#"{
            "name": "router-layer",
            "layer": 0,
            "model": "my-special-input-model",
            "adapter": "openai-api",
            "bind-addr": "0.0.0.0",
            "bind-port": "8080",
            "output-to": [1]
        }"#;

        let node: ArchitectureNode = serde_json::from_str(json).unwrap();
        assert_eq!(node.name, "router-layer");
        assert_eq!(node.layer, Some(0));
        assert_eq!(node.model, Some("my-special-input-model".to_string()));
        assert!(node.is_router());
        assert_eq!(node.get_target_layer(), Some(1));
    }

    #[test]
    fn test_parse_hidden_layer_node() {
        let json = r#"{
            "name": "company-2024-q3-fine-tune-qwen3-30b",
            "layer": 1,
            "adapter": "openai-api",
            "use-case": "If $INPUT is about company information during 2024 q3",
            "bind-addr": "0.0.0.0",
            "bind-port": "8081",
            "output-to": ["final-output"]
        }"#;

        let node: ArchitectureNode = serde_json::from_str(json).unwrap();
        assert_eq!(node.layer, Some(1));
        assert_eq!(
            node.use_case,
            Some("If $INPUT is about company information during 2024 q3".to_string())
        );
        match &node.output_to {
            Some(OutputTarget::Nodes(nodes)) => {
                assert_eq!(nodes, &vec!["final-output".to_string()]);
            }
            _ => panic!("Expected Nodes output target"),
        }
    }

    #[test]
    fn test_parse_output_node() {
        let json = r#"{
            "name": "final-output",
            "adapter": "output"
        }"#;

        let node: ArchitectureNode = serde_json::from_str(json).unwrap();
        assert!(node.is_output());
        assert!(!node.is_router());
    }

    #[test]
    fn test_parse_conditional_ws_node() {
        let json = r#"{
            "name": "output-to-ws",
            "if": "$OutputCustomKey",
            "adapter": "ws",
            "url": "ws://localhost:3000"
        }"#;

        let node: ArchitectureNode = serde_json::from_str(json).unwrap();
        assert_eq!(node.condition, Some("$OutputCustomKey".to_string()));
        assert_eq!(node.url, Some("ws://localhost:3000".to_string()));
    }

    #[test]
    fn test_effective_bind_addr_default() {
        let node = ArchitectureNode {
            name: "test".to_string(),
            layer: None,
            model: None,
            adapter: "output".to_string(),
            bind_addr: None,
            bind_port: None,
            output_to: None,
            use_case: None,
            condition: None,
            url: None,
            extra_options: HashMap::new(),
        };
        assert_eq!(node.effective_bind_addr(), "0.0.0.0");
    }
}
