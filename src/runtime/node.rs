use crate::config::{ArchitectureNode, ModelDefinition, OutputTarget};

/// Adapter type for a runtime node
#[derive(Debug, Clone, PartialEq)]
pub enum AdapterType {
    OpenAiApi,
    Output,
    WebSocket { url: String },
}

impl AdapterType {
    pub fn from_node(node: &ArchitectureNode) -> Self {
        match node.adapter.as_str() {
            "output" => AdapterType::Output,
            "ws" => AdapterType::WebSocket {
                url: node.url.clone().unwrap_or_default(),
            },
            _ => AdapterType::OpenAiApi,
        }
    }
}

/// Runtime representation of a node in the pipeline
#[derive(Debug, Clone)]
pub struct RuntimeNode {
    pub name: String,
    pub layer: u32,
    pub adapter: AdapterType,
    pub bind_addr: String,
    pub bind_port: u16,
    pub model_config: Option<ModelDefinition>,
    pub output_targets: Option<OutputTarget>,
    pub use_case: Option<String>,
    pub condition: Option<String>,
}

impl RuntimeNode {
    /// Create from architecture node and model definition
    pub fn from_architecture(
        node: &ArchitectureNode,
        model_config: Option<ModelDefinition>,
        port_offset: u16,
    ) -> Self {
        let bind_port = node
            .bind_port
            .as_ref()
            .and_then(|p| p.parse().ok())
            .unwrap_or(8080 + port_offset);

        Self {
            name: node.name.clone(),
            layer: node.layer.unwrap_or(0),
            adapter: AdapterType::from_node(node),
            bind_addr: node.effective_bind_addr().to_string(),
            bind_port,
            model_config,
            output_targets: node.output_to.clone(),
            use_case: node.use_case.clone(),
            condition: node.condition.clone(),
        }
    }

    /// Check if this node is the final output
    pub fn is_output(&self) -> bool {
        matches!(self.adapter, AdapterType::Output)
    }

    /// Check if this is a WebSocket output
    pub fn is_websocket(&self) -> bool {
        matches!(self.adapter, AdapterType::WebSocket { .. })
    }

    /// Get the socket address for binding
    pub fn socket_addr(&self) -> String {
        format!("{}:{}", self.bind_addr, self.bind_port)
    }
}

// ============================================================================
// SBIO: Pure function for condition evaluation
// ============================================================================

/// Evaluate a condition expression against headers.
/// Pure function - no I/O.
pub fn evaluate_condition(
    condition: &str,
    headers: &std::collections::HashMap<String, String>,
) -> bool {
    let condition = condition.trim();

    // Handle simple variable existence check: "$VarName"
    if condition.starts_with('$') && !condition.contains(' ') {
        let var_name = &condition[1..];
        return headers.get(var_name).map(|v| !v.is_empty()).unwrap_or(false);
    }

    // Handle equality: "$VarName == \"value\""
    if condition.contains("==") {
        let parts: Vec<&str> = condition.split("==").collect();
        if parts.len() == 2 {
            let var_part = parts[0].trim();
            let val_part = parts[1].trim().trim_matches('"');

            if var_part.starts_with('$') {
                let var_name = &var_part[1..];
                return headers.get(var_name).map(|v| v == val_part).unwrap_or(false);
            }
        }
    }

    // Handle inequality: "$VarName != \"value\""
    if condition.contains("!=") {
        let parts: Vec<&str> = condition.split("!=").collect();
        if parts.len() == 2 {
            let var_part = parts[0].trim();
            let val_part = parts[1].trim().trim_matches('"');

            if var_part.starts_with('$') {
                let var_name = &var_part[1..];
                return headers.get(var_name).map(|v| v != val_part).unwrap_or(true);
            }
        }
    }

    // Default: condition passes if we can't parse it
    true
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::collections::HashMap;

    #[test]
    fn test_adapter_type_from_node() {
        let mut node = ArchitectureNode {
            name: "test".to_string(),
            layer: None,
            model: None,
            adapter: "openai-api".to_string(),
            bind_addr: None,
            bind_port: None,
            output_to: None,
            use_case: None,
            condition: None,
            url: None,
            extra_options: HashMap::new(),
        };

        assert_eq!(AdapterType::from_node(&node), AdapterType::OpenAiApi);

        node.adapter = "output".to_string();
        assert_eq!(AdapterType::from_node(&node), AdapterType::Output);

        node.adapter = "ws".to_string();
        node.url = Some("ws://localhost:3000".to_string());
        assert!(matches!(AdapterType::from_node(&node), AdapterType::WebSocket { .. }));
    }

    #[test]
    fn test_runtime_node_from_architecture() {
        let arch_node = ArchitectureNode {
            name: "test-node".to_string(),
            layer: Some(1),
            model: None,
            adapter: "openai-api".to_string(),
            bind_addr: Some("127.0.0.1".to_string()),
            bind_port: Some("9000".to_string()),
            output_to: None,
            use_case: Some("Test use case".to_string()),
            condition: None,
            url: None,
            extra_options: HashMap::new(),
        };

        let runtime = RuntimeNode::from_architecture(&arch_node, None, 0);

        assert_eq!(runtime.name, "test-node");
        assert_eq!(runtime.layer, 1);
        assert_eq!(runtime.bind_addr, "127.0.0.1");
        assert_eq!(runtime.bind_port, 9000);
        assert_eq!(runtime.socket_addr(), "127.0.0.1:9000");
    }

    #[test]
    fn test_evaluate_condition_existence() {
        let mut headers = HashMap::new();
        headers.insert("MyKey".to_string(), "some-value".to_string());

        assert!(evaluate_condition("$MyKey", &headers));
        assert!(!evaluate_condition("$NonExistent", &headers));
    }

    #[test]
    fn test_evaluate_condition_empty_value() {
        let mut headers = HashMap::new();
        headers.insert("EmptyKey".to_string(), "".to_string());

        assert!(!evaluate_condition("$EmptyKey", &headers));
    }

    #[test]
    fn test_evaluate_condition_equality() {
        let mut headers = HashMap::new();
        headers.insert("Mode".to_string(), "test".to_string());

        assert!(evaluate_condition("$Mode == \"test\"", &headers));
        assert!(!evaluate_condition("$Mode == \"prod\"", &headers));
    }

    #[test]
    fn test_evaluate_condition_inequality() {
        let mut headers = HashMap::new();
        headers.insert("Mode".to_string(), "test".to_string());

        assert!(evaluate_condition("$Mode != \"prod\"", &headers));
        assert!(!evaluate_condition("$Mode != \"test\"", &headers));
    }
}
