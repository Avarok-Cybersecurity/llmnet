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
    pub extra_options: std::collections::HashMap<String, serde_json::Value>,
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
            extra_options: node.extra_options.clone(),
        }
    }

    /// Get the model override from extra_options if specified
    pub fn model_override(&self) -> Option<String> {
        self.extra_options
            .get("model_override")
            .and_then(|v| v.as_str())
            .map(|s| s.to_string())
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

/// Evaluate a condition expression against variables.
/// Pure function - no I/O.
///
/// Supported operators:
/// - `$VarName` - checks variable exists and is non-empty
/// - `$VarName == "value"` - string equality
/// - `$VarName != "value"` - string inequality
/// - `$VarName > value` - numeric greater than
/// - `$VarName < value` - numeric less than
/// - `$VarName >= value` - numeric greater than or equal
/// - `$VarName <= value` - numeric less than or equal
pub fn evaluate_condition(
    condition: &str,
    variables: &std::collections::HashMap<String, String>,
) -> bool {
    let condition = condition.trim();

    // Handle simple variable existence check: "$VarName"
    if condition.starts_with('$') && !condition.contains(' ') {
        let var_name = &condition[1..];
        return variables.get(var_name).map(|v| !v.is_empty()).unwrap_or(false);
    }

    // Handle equality: "$VarName == \"value\""
    if condition.contains("==") {
        let parts: Vec<&str> = condition.split("==").collect();
        if parts.len() == 2 {
            let var_part = parts[0].trim();
            let val_part = parts[1].trim().trim_matches('"');

            if var_part.starts_with('$') {
                let var_name = &var_part[1..];
                return variables.get(var_name).map(|v| v == val_part).unwrap_or(false);
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
                return variables.get(var_name).map(|v| v != val_part).unwrap_or(true);
            }
        }
    }

    // Handle numeric comparisons: >=, <=, >, <
    // Check >= and <= before > and < to avoid partial matches
    if let Some(result) = try_numeric_comparison(condition, ">=", variables, |a, b| a >= b) {
        return result;
    }
    if let Some(result) = try_numeric_comparison(condition, "<=", variables, |a, b| a <= b) {
        return result;
    }
    if let Some(result) = try_numeric_comparison(condition, ">", variables, |a, b| a > b) {
        return result;
    }
    if let Some(result) = try_numeric_comparison(condition, "<", variables, |a, b| a < b) {
        return result;
    }

    // Default: condition passes if we can't parse it
    true
}

/// Helper to parse and evaluate numeric comparisons
fn try_numeric_comparison<F>(
    condition: &str,
    operator: &str,
    variables: &std::collections::HashMap<String, String>,
    compare: F,
) -> Option<bool>
where
    F: Fn(f64, f64) -> bool,
{
    if !condition.contains(operator) {
        return None;
    }

    // Avoid matching >= when looking for > (and <= for <)
    if operator == ">" && condition.contains(">=") {
        return None;
    }
    if operator == "<" && condition.contains("<=") {
        return None;
    }

    let parts: Vec<&str> = condition.splitn(2, operator).collect();
    if parts.len() != 2 {
        return None;
    }

    let var_part = parts[0].trim();
    let val_part = parts[1].trim();

    if !var_part.starts_with('$') {
        return None;
    }

    let var_name = &var_part[1..];
    let var_value = variables.get(var_name)?;
    let var_num: f64 = var_value.parse().ok()?;
    let cmp_num: f64 = val_part.parse().ok()?;

    Some(compare(var_num, cmp_num))
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::collections::HashMap;

    #[test]
    fn test_adapter_type_from_node() {
        let node1 = ArchitectureNode {
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
        assert_eq!(AdapterType::from_node(&node1), AdapterType::OpenAiApi);

        let node2 = ArchitectureNode {
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
        assert_eq!(AdapterType::from_node(&node2), AdapterType::Output);

        let node3 = ArchitectureNode {
            name: "test".to_string(),
            layer: None,
            model: None,
            adapter: "ws".to_string(),
            bind_addr: None,
            bind_port: None,
            output_to: None,
            use_case: None,
            condition: None,
            url: Some("ws://localhost:3000".to_string()),
            extra_options: HashMap::new(),
        };
        assert!(matches!(AdapterType::from_node(&node3), AdapterType::WebSocket { .. }));
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
        let mut vars = HashMap::new();
        vars.insert("MyKey".to_string(), "some-value".to_string());

        assert!(evaluate_condition("$MyKey", &vars));
        assert!(!evaluate_condition("$NonExistent", &vars));
    }

    #[test]
    fn test_evaluate_condition_empty_value() {
        let mut vars = HashMap::new();
        vars.insert("EmptyKey".to_string(), "".to_string());

        assert!(!evaluate_condition("$EmptyKey", &vars));
    }

    #[test]
    fn test_evaluate_condition_equality() {
        let mut vars = HashMap::new();
        vars.insert("Mode".to_string(), "test".to_string());

        assert!(evaluate_condition("$Mode == \"test\"", &vars));
        assert!(!evaluate_condition("$Mode == \"prod\"", &vars));
    }

    #[test]
    fn test_evaluate_condition_inequality() {
        let mut vars = HashMap::new();
        vars.insert("Mode".to_string(), "test".to_string());

        assert!(evaluate_condition("$Mode != \"prod\"", &vars));
        assert!(!evaluate_condition("$Mode != \"test\"", &vars));
    }

    // ========================================================================
    // Numeric Comparison Operator Tests
    // ========================================================================

    #[test]
    fn test_evaluate_condition_greater_than() {
        let mut vars = HashMap::new();
        vars.insert("HOP_COUNT".to_string(), "5".to_string());

        assert!(evaluate_condition("$HOP_COUNT > 3", &vars));
        assert!(!evaluate_condition("$HOP_COUNT > 5", &vars));
        assert!(!evaluate_condition("$HOP_COUNT > 10", &vars));
    }

    #[test]
    fn test_evaluate_condition_less_than() {
        let mut vars = HashMap::new();
        vars.insert("HOP_COUNT".to_string(), "5".to_string());

        assert!(evaluate_condition("$HOP_COUNT < 10", &vars));
        assert!(!evaluate_condition("$HOP_COUNT < 5", &vars));
        assert!(!evaluate_condition("$HOP_COUNT < 3", &vars));
    }

    #[test]
    fn test_evaluate_condition_greater_than_or_equal() {
        let mut vars = HashMap::new();
        vars.insert("HOP_COUNT".to_string(), "5".to_string());

        assert!(evaluate_condition("$HOP_COUNT >= 5", &vars));
        assert!(evaluate_condition("$HOP_COUNT >= 3", &vars));
        assert!(!evaluate_condition("$HOP_COUNT >= 10", &vars));
    }

    #[test]
    fn test_evaluate_condition_less_than_or_equal() {
        let mut vars = HashMap::new();
        vars.insert("HOP_COUNT".to_string(), "5".to_string());

        assert!(evaluate_condition("$HOP_COUNT <= 5", &vars));
        assert!(evaluate_condition("$HOP_COUNT <= 10", &vars));
        assert!(!evaluate_condition("$HOP_COUNT <= 3", &vars));
    }

    #[test]
    fn test_evaluate_condition_float_comparison() {
        let mut vars = HashMap::new();
        vars.insert("SCORE".to_string(), "3.5".to_string());

        assert!(evaluate_condition("$SCORE > 3.0", &vars));
        assert!(evaluate_condition("$SCORE < 4.0", &vars));
        assert!(evaluate_condition("$SCORE >= 3.5", &vars));
        assert!(evaluate_condition("$SCORE <= 3.5", &vars));
    }

    #[test]
    fn test_evaluate_condition_input_length() {
        let mut vars = HashMap::new();
        vars.insert("INPUT_LENGTH".to_string(), "100".to_string());

        assert!(evaluate_condition("$INPUT_LENGTH > 50", &vars));
        assert!(evaluate_condition("$INPUT_LENGTH < 200", &vars));
        assert!(evaluate_condition("$INPUT_LENGTH >= 100", &vars));
        assert!(evaluate_condition("$INPUT_LENGTH <= 100", &vars));
    }

    #[test]
    fn test_evaluate_condition_word_count() {
        let mut vars = HashMap::new();
        vars.insert("WORD_COUNT".to_string(), "15".to_string());

        assert!(evaluate_condition("$WORD_COUNT > 10", &vars));
        assert!(!evaluate_condition("$WORD_COUNT > 20", &vars));
    }

    #[test]
    fn test_evaluate_condition_non_numeric_fails_gracefully() {
        let mut vars = HashMap::new();
        vars.insert("NAME".to_string(), "hello".to_string());

        // Non-numeric comparisons should return true (default pass)
        assert!(evaluate_condition("$NAME > 5", &vars));
    }

    #[test]
    fn test_evaluate_condition_missing_var_numeric() {
        let vars = HashMap::new();

        // Missing variable with numeric comparison returns true (default pass)
        assert!(evaluate_condition("$MISSING > 5", &vars));
    }

    #[test]
    fn test_evaluate_condition_edge_cases() {
        let mut vars = HashMap::new();
        vars.insert("ZERO".to_string(), "0".to_string());
        vars.insert("NEGATIVE".to_string(), "-5".to_string());

        assert!(evaluate_condition("$ZERO >= 0", &vars));
        assert!(evaluate_condition("$ZERO <= 0", &vars));
        assert!(!evaluate_condition("$ZERO > 0", &vars));
        assert!(!evaluate_condition("$ZERO < 0", &vars));

        assert!(evaluate_condition("$NEGATIVE < 0", &vars));
        assert!(evaluate_condition("$NEGATIVE > -10", &vars));
    }
}
