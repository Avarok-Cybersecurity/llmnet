use std::collections::HashMap;
use uuid::Uuid;

/// System variable names (constants for consistency)
pub mod vars {
    pub const INITIAL_INPUT: &str = "INITIAL_INPUT";
    pub const CURRENT_INPUT: &str = "CURRENT_INPUT";
    pub const PREV_NODE: &str = "PREV_NODE";
    pub const PREV_LAYER: &str = "PREV_LAYER";
    pub const CURRENT_LAYER: &str = "CURRENT_LAYER";
    pub const HOP_COUNT: &str = "HOP_COUNT";
    pub const TIMESTAMP: &str = "TIMESTAMP";
    pub const REQUEST_ID: &str = "REQUEST_ID";
    pub const ROUTE_DECISION: &str = "ROUTE_DECISION";
    pub const INPUT_LENGTH: &str = "INPUT_LENGTH";
    pub const WORD_COUNT: &str = "WORD_COUNT";
}

/// A request flowing through the pipeline
#[derive(Debug, Clone)]
pub struct PipelineRequest {
    /// Unique identifier for this request
    pub request_id: Uuid,
    /// The original user prompt
    pub original_prompt: String,
    /// Current content (may change as it flows through layers)
    pub current_content: String,
    /// Variables that can be used in conditions (includes system + custom)
    pub variables: HashMap<String, String>,
    /// Trace of hops through the pipeline
    pub trace: Vec<RequestHop>,
    /// Start timestamp for this request
    pub start_time: chrono::DateTime<chrono::Utc>,
}

/// A single hop in the pipeline trace
#[derive(Debug, Clone)]
pub struct RequestHop {
    pub node_name: String,
    pub layer: u32,
    pub timestamp: chrono::DateTime<chrono::Utc>,
    /// Router decision that led to this hop (if applicable)
    pub decision: Option<String>,
}

impl PipelineRequest {
    /// Create a new pipeline request
    pub fn new(prompt: String) -> Self {
        let now = chrono::Utc::now();
        let request_id = Uuid::new_v4();
        let mut variables = HashMap::new();

        // Initialize system variables
        variables.insert(vars::INITIAL_INPUT.to_string(), prompt.clone());
        variables.insert(vars::CURRENT_INPUT.to_string(), prompt.clone());
        variables.insert(vars::HOP_COUNT.to_string(), "0".to_string());
        variables.insert(vars::TIMESTAMP.to_string(), now.timestamp().to_string());
        variables.insert(vars::REQUEST_ID.to_string(), request_id.to_string());
        variables.insert(vars::INPUT_LENGTH.to_string(), prompt.len().to_string());
        variables.insert(
            vars::WORD_COUNT.to_string(),
            count_words(&prompt).to_string(),
        );

        Self {
            request_id,
            original_prompt: prompt.clone(),
            current_content: prompt,
            variables,
            trace: Vec::new(),
            start_time: now,
        }
    }

    /// Create a request with a specific ID (useful for testing)
    pub fn with_id(request_id: Uuid, prompt: String) -> Self {
        let now = chrono::Utc::now();
        let mut variables = HashMap::new();

        variables.insert(vars::INITIAL_INPUT.to_string(), prompt.clone());
        variables.insert(vars::CURRENT_INPUT.to_string(), prompt.clone());
        variables.insert(vars::HOP_COUNT.to_string(), "0".to_string());
        variables.insert(vars::TIMESTAMP.to_string(), now.timestamp().to_string());
        variables.insert(vars::REQUEST_ID.to_string(), request_id.to_string());
        variables.insert(vars::INPUT_LENGTH.to_string(), prompt.len().to_string());
        variables.insert(
            vars::WORD_COUNT.to_string(),
            count_words(&prompt).to_string(),
        );

        Self {
            request_id,
            original_prompt: prompt.clone(),
            current_content: prompt,
            variables,
            trace: Vec::new(),
            start_time: now,
        }
    }

    /// Add a hop to the trace and update system variables
    pub fn add_hop(&mut self, node_name: String, layer: u32, decision: Option<String>) {
        self.trace.push(RequestHop {
            node_name: node_name.clone(),
            layer,
            timestamp: chrono::Utc::now(),
            decision: decision.clone(),
        });

        // Update system variables after hop
        self.variables
            .insert(vars::PREV_NODE.to_string(), node_name);
        self.variables
            .insert(vars::PREV_LAYER.to_string(), layer.to_string());
        self.variables
            .insert(vars::HOP_COUNT.to_string(), self.trace.len().to_string());
        if let Some(dec) = decision {
            self.variables.insert(vars::ROUTE_DECISION.to_string(), dec);
        }
    }

    /// Set the current layer being evaluated
    pub fn set_current_layer(&mut self, layer: u32) {
        self.variables
            .insert(vars::CURRENT_LAYER.to_string(), layer.to_string());
    }

    /// Update the current content (after processing by a node)
    pub fn set_content(&mut self, content: String) {
        self.current_content = content.clone();
        self.variables
            .insert(vars::CURRENT_INPUT.to_string(), content.clone());
        self.variables
            .insert(vars::INPUT_LENGTH.to_string(), content.len().to_string());
        self.variables.insert(
            vars::WORD_COUNT.to_string(),
            count_words(&content).to_string(),
        );
        // Update timestamp on each content change
        self.variables.insert(
            vars::TIMESTAMP.to_string(),
            chrono::Utc::now().timestamp().to_string(),
        );
    }

    /// Set a custom variable value
    pub fn set_variable(&mut self, key: String, value: String) {
        self.variables.insert(key, value);
    }

    /// Get a variable value
    pub fn get_variable(&self, key: &str) -> Option<&String> {
        self.variables.get(key)
    }

    /// Get all variables (for condition evaluation)
    pub fn get_variables(&self) -> &HashMap<String, String> {
        &self.variables
    }

    // Legacy compatibility methods
    #[deprecated(note = "Use set_variable instead")]
    pub fn set_header(&mut self, key: String, value: String) {
        self.set_variable(key, value);
    }

    #[deprecated(note = "Use get_variable instead")]
    pub fn get_header(&self, key: &str) -> Option<&String> {
        self.get_variable(key)
    }
}

/// Count words in a string (simple whitespace split)
fn count_words(s: &str) -> usize {
    s.split_whitespace().count()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_new_request() {
        let req = PipelineRequest::new("Hello".to_string());
        assert_eq!(req.original_prompt, "Hello");
        assert_eq!(req.current_content, "Hello");
        assert!(req.trace.is_empty());
    }

    #[test]
    fn test_add_hop() {
        let mut req = PipelineRequest::new("Hello".to_string());
        req.add_hop("router".to_string(), 0, Some("node1".to_string()));

        assert_eq!(req.trace.len(), 1);
        assert_eq!(req.trace[0].node_name, "router");
        assert_eq!(req.trace[0].layer, 0);
        assert_eq!(req.trace[0].decision, Some("node1".to_string()));
    }

    #[test]
    fn test_variables() {
        let mut req = PipelineRequest::new("Hello".to_string());
        req.set_variable("X-Custom".to_string(), "value".to_string());

        assert_eq!(req.get_variable("X-Custom"), Some(&"value".to_string()));
        assert_eq!(req.get_variable("Nonexistent"), None);
    }

    #[test]
    fn test_update_content() {
        let mut req = PipelineRequest::new("Original".to_string());
        req.set_content("Modified".to_string());

        assert_eq!(req.original_prompt, "Original");
        assert_eq!(req.current_content, "Modified");
    }

    // ========================================================================
    // System Variable Tests
    // ========================================================================

    #[test]
    fn test_initial_input_variable() {
        let req = PipelineRequest::new("Hello World".to_string());
        assert_eq!(
            req.get_variable(vars::INITIAL_INPUT),
            Some(&"Hello World".to_string())
        );
    }

    #[test]
    fn test_current_input_variable() {
        let mut req = PipelineRequest::new("Hello".to_string());
        assert_eq!(
            req.get_variable(vars::CURRENT_INPUT),
            Some(&"Hello".to_string())
        );

        req.set_content("Modified content".to_string());
        assert_eq!(
            req.get_variable(vars::CURRENT_INPUT),
            Some(&"Modified content".to_string())
        );
        // INITIAL_INPUT should remain unchanged
        assert_eq!(
            req.get_variable(vars::INITIAL_INPUT),
            Some(&"Hello".to_string())
        );
    }

    #[test]
    fn test_prev_node_variable() {
        let mut req = PipelineRequest::new("Hello".to_string());
        assert_eq!(req.get_variable(vars::PREV_NODE), None);

        req.add_hop("router".to_string(), 0, None);
        assert_eq!(
            req.get_variable(vars::PREV_NODE),
            Some(&"router".to_string())
        );

        req.add_hop("handler".to_string(), 1, None);
        assert_eq!(
            req.get_variable(vars::PREV_NODE),
            Some(&"handler".to_string())
        );
    }

    #[test]
    fn test_prev_layer_variable() {
        let mut req = PipelineRequest::new("Hello".to_string());
        assert_eq!(req.get_variable(vars::PREV_LAYER), None);

        req.add_hop("router".to_string(), 0, None);
        assert_eq!(req.get_variable(vars::PREV_LAYER), Some(&"0".to_string()));

        req.add_hop("handler".to_string(), 1, None);
        assert_eq!(req.get_variable(vars::PREV_LAYER), Some(&"1".to_string()));
    }

    #[test]
    fn test_current_layer_variable() {
        let mut req = PipelineRequest::new("Hello".to_string());
        req.set_current_layer(2);
        assert_eq!(
            req.get_variable(vars::CURRENT_LAYER),
            Some(&"2".to_string())
        );
    }

    #[test]
    fn test_hop_count_variable() {
        let mut req = PipelineRequest::new("Hello".to_string());
        assert_eq!(req.get_variable(vars::HOP_COUNT), Some(&"0".to_string()));

        req.add_hop("router".to_string(), 0, None);
        assert_eq!(req.get_variable(vars::HOP_COUNT), Some(&"1".to_string()));

        req.add_hop("handler".to_string(), 1, None);
        assert_eq!(req.get_variable(vars::HOP_COUNT), Some(&"2".to_string()));
    }

    #[test]
    fn test_timestamp_variable() {
        let req = PipelineRequest::new("Hello".to_string());
        let ts = req.get_variable(vars::TIMESTAMP).unwrap();
        let ts_num: i64 = ts.parse().unwrap();
        // Should be a reasonable Unix timestamp (after year 2020)
        assert!(ts_num > 1577836800);
    }

    #[test]
    fn test_request_id_variable() {
        let req = PipelineRequest::new("Hello".to_string());
        let id = req.get_variable(vars::REQUEST_ID).unwrap();
        // Should be a valid UUID
        assert!(Uuid::parse_str(id).is_ok());
        assert_eq!(id, &req.request_id.to_string());
    }

    #[test]
    fn test_route_decision_variable() {
        let mut req = PipelineRequest::new("Hello".to_string());
        assert_eq!(req.get_variable(vars::ROUTE_DECISION), None);

        req.add_hop("router".to_string(), 0, Some("handler-a".to_string()));
        assert_eq!(
            req.get_variable(vars::ROUTE_DECISION),
            Some(&"handler-a".to_string())
        );
    }

    #[test]
    fn test_input_length_variable() {
        let mut req = PipelineRequest::new("Hello".to_string());
        assert_eq!(req.get_variable(vars::INPUT_LENGTH), Some(&"5".to_string()));

        req.set_content("Hello World".to_string());
        assert_eq!(
            req.get_variable(vars::INPUT_LENGTH),
            Some(&"11".to_string())
        );
    }

    #[test]
    fn test_word_count_variable() {
        let mut req = PipelineRequest::new("Hello World".to_string());
        assert_eq!(req.get_variable(vars::WORD_COUNT), Some(&"2".to_string()));

        req.set_content("The quick brown fox".to_string());
        assert_eq!(req.get_variable(vars::WORD_COUNT), Some(&"4".to_string()));
    }

    #[test]
    fn test_count_words_helper() {
        assert_eq!(count_words(""), 0);
        assert_eq!(count_words("hello"), 1);
        assert_eq!(count_words("hello world"), 2);
        assert_eq!(count_words("  hello   world  "), 2);
        assert_eq!(count_words("one\ttwo\nthree"), 3);
    }
}
