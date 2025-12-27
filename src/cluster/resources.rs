//! Shared resource types for LLMNet cluster management

use serde::{Deserialize, Serialize};

/// A namespace for organizing pipelines
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Namespace {
    /// API version
    #[serde(rename = "apiVersion")]
    pub api_version: String,

    /// Kind is always "Namespace"
    pub kind: String,

    /// Metadata
    pub metadata: NamespaceMetadata,
}

/// Namespace metadata
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct NamespaceMetadata {
    /// Namespace name
    pub name: String,
}

impl Namespace {
    /// Create a new namespace
    pub fn new(name: impl Into<String>) -> Self {
        Self {
            api_version: "llmnet/v1".to_string(),
            kind: "Namespace".to_string(),
            metadata: NamespaceMetadata { name: name.into() },
        }
    }
}

impl Default for Namespace {
    fn default() -> Self {
        Namespace::new("default")
    }
}

/// Response for listing resources
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ResourceList<T> {
    /// API version
    #[serde(rename = "apiVersion")]
    pub api_version: String,

    /// Kind (e.g., "PipelineList", "NodeList")
    pub kind: String,

    /// List of items
    pub items: Vec<T>,
}

impl<T> ResourceList<T> {
    /// Create a new resource list
    pub fn new(kind: impl Into<String>, items: Vec<T>) -> Self {
        Self {
            api_version: "llmnet/v1".to_string(),
            kind: kind.into(),
            items,
        }
    }
}

/// Label selector for filtering resources
#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct LabelSelector {
    /// Match exact labels
    #[serde(rename = "matchLabels")]
    #[serde(default)]
    pub match_labels: std::collections::HashMap<String, String>,
}

impl LabelSelector {
    /// Create a selector that matches a specific label
    pub fn matching(key: impl Into<String>, value: impl Into<String>) -> Self {
        let mut labels = std::collections::HashMap::new();
        labels.insert(key.into(), value.into());
        Self {
            match_labels: labels,
        }
    }

    /// Check if labels match this selector
    pub fn matches(&self, labels: &std::collections::HashMap<String, String>) -> bool {
        self.match_labels
            .iter()
            .all(|(k, v)| labels.get(k) == Some(v))
    }
}

/// Status of an operation
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct OperationStatus {
    /// Success or failure
    pub success: bool,

    /// Status message
    pub message: String,

    /// Details (if any)
    #[serde(skip_serializing_if = "Option::is_none")]
    pub details: Option<serde_json::Value>,
}

impl OperationStatus {
    /// Create success status
    pub fn success(message: impl Into<String>) -> Self {
        Self {
            success: true,
            message: message.into(),
            details: None,
        }
    }

    /// Create failure status
    pub fn failure(message: impl Into<String>) -> Self {
        Self {
            success: false,
            message: message.into(),
            details: None,
        }
    }

    /// Add details
    pub fn with_details(mut self, details: serde_json::Value) -> Self {
        self.details = Some(details);
        self
    }
}

/// Watch event for resource changes (for future streaming updates)
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct WatchEvent<T> {
    /// Type of event: ADDED, MODIFIED, DELETED
    #[serde(rename = "type")]
    pub event_type: WatchEventType,

    /// The affected resource
    pub object: T,
}

/// Types of watch events
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum WatchEventType {
    /// Resource was created
    #[serde(rename = "ADDED")]
    Added,
    /// Resource was modified
    #[serde(rename = "MODIFIED")]
    Modified,
    /// Resource was deleted
    #[serde(rename = "DELETED")]
    Deleted,
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_namespace() {
        let ns = Namespace::new("production");
        assert_eq!(ns.metadata.name, "production");
        assert_eq!(ns.kind, "Namespace");
    }

    #[test]
    fn test_resource_list() {
        let list: ResourceList<String> =
            ResourceList::new("StringList", vec!["a".into(), "b".into()]);
        assert_eq!(list.items.len(), 2);
        assert_eq!(list.kind, "StringList");
    }

    #[test]
    fn test_label_selector_matches() {
        let selector = LabelSelector::matching("env", "prod");

        let mut labels = std::collections::HashMap::new();
        labels.insert("env".to_string(), "prod".to_string());
        labels.insert("app".to_string(), "web".to_string());

        assert!(selector.matches(&labels));
    }

    #[test]
    fn test_label_selector_no_match() {
        let selector = LabelSelector::matching("env", "prod");

        let mut labels = std::collections::HashMap::new();
        labels.insert("env".to_string(), "dev".to_string());

        assert!(!selector.matches(&labels));
    }

    #[test]
    fn test_operation_status() {
        let success = OperationStatus::success("Pipeline deployed");
        assert!(success.success);

        let failure = OperationStatus::failure("Node not found");
        assert!(!failure.success);
    }
}
