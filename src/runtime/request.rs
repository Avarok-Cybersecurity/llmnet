use std::collections::HashMap;
use uuid::Uuid;

/// A request flowing through the pipeline
#[derive(Debug, Clone)]
pub struct PipelineRequest {
    /// Unique identifier for this request
    pub request_id: Uuid,
    /// The original user prompt
    pub original_prompt: String,
    /// Current content (may change as it flows through layers)
    pub current_content: String,
    /// Custom headers that can be used in conditions
    pub headers: HashMap<String, String>,
    /// Trace of hops through the pipeline
    pub trace: Vec<RequestHop>,
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
        Self {
            request_id: Uuid::new_v4(),
            original_prompt: prompt.clone(),
            current_content: prompt,
            headers: HashMap::new(),
            trace: Vec::new(),
        }
    }

    /// Create a request with a specific ID (useful for testing)
    pub fn with_id(request_id: Uuid, prompt: String) -> Self {
        Self {
            request_id,
            original_prompt: prompt.clone(),
            current_content: prompt,
            headers: HashMap::new(),
            trace: Vec::new(),
        }
    }

    /// Add a hop to the trace
    pub fn add_hop(&mut self, node_name: String, layer: u32, decision: Option<String>) {
        self.trace.push(RequestHop {
            node_name,
            layer,
            timestamp: chrono::Utc::now(),
            decision,
        });
    }

    /// Update the current content (after processing by a node)
    pub fn set_content(&mut self, content: String) {
        self.current_content = content;
    }

    /// Set a header value
    pub fn set_header(&mut self, key: String, value: String) {
        self.headers.insert(key, value);
    }

    /// Get a header value
    pub fn get_header(&self, key: &str) -> Option<&String> {
        self.headers.get(key)
    }
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
    fn test_headers() {
        let mut req = PipelineRequest::new("Hello".to_string());
        req.set_header("X-Custom".to_string(), "value".to_string());

        assert_eq!(req.get_header("X-Custom"), Some(&"value".to_string()));
        assert_eq!(req.get_header("Nonexistent"), None);
    }

    #[test]
    fn test_update_content() {
        let mut req = PipelineRequest::new("Original".to_string());
        req.set_content("Modified".to_string());

        assert_eq!(req.original_prompt, "Original");
        assert_eq!(req.current_content, "Modified");
    }
}
