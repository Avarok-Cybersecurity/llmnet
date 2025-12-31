//! Display formatting for CLI output
//!
//! SBIO pattern: Pure functions that format data for display

use super::commands::{ContextInfo, ValidationResult};
use crate::cluster::Pipeline;
use crate::config::Composition;

// ============================================================================
// Table formatting helpers
// ============================================================================

/// Format a simple table with headers and rows
pub fn format_table(headers: &[&str], rows: Vec<Vec<String>>) -> String {
    if rows.is_empty() {
        return "No resources found.\n".to_string();
    }

    // Calculate column widths
    let mut widths: Vec<usize> = headers.iter().map(|h| h.len()).collect();
    for row in &rows {
        for (i, cell) in row.iter().enumerate() {
            if i < widths.len() {
                widths[i] = widths[i].max(cell.len());
            }
        }
    }

    let mut output = String::new();

    // Header
    for (i, header) in headers.iter().enumerate() {
        if i > 0 {
            output.push_str("   ");
        }
        output.push_str(&format!(
            "{:width$}",
            header.to_uppercase(),
            width = widths[i]
        ));
    }
    output.push('\n');

    // Rows
    for row in rows {
        for (i, cell) in row.iter().enumerate() {
            if i > 0 {
                output.push_str("   ");
            }
            if i < widths.len() {
                output.push_str(&format!("{:width$}", cell, width = widths[i]));
            } else {
                output.push_str(cell);
            }
        }
        output.push('\n');
    }

    output
}

// ============================================================================
// Context display
// ============================================================================

/// Format context list for display
pub fn format_context_list(contexts: &[ContextInfo]) -> String {
    let headers = &["", "NAME", "URL"];
    let rows: Vec<Vec<String>> = contexts
        .iter()
        .map(|ctx| {
            vec![
                if ctx.is_current { "*" } else { " " }.to_string(),
                ctx.name.clone(),
                ctx.url.clone(),
            ]
        })
        .collect();

    format_table(headers, rows)
}

/// Format current context for display
pub fn format_current_context(name: &str, url: &str) -> String {
    format!("Current context: {} ({})\n", name, url)
}

// ============================================================================
// Pipeline display
// ============================================================================

/// Format pipeline list for display
pub fn format_pipeline_list(pipelines: &[Pipeline]) -> String {
    let headers = &["NAMESPACE", "NAME", "REPLICAS", "READY", "STATUS"];
    let rows: Vec<Vec<String>> = pipelines
        .iter()
        .map(|p| {
            let (ready, total) = p
                .status
                .as_ref()
                .map(|s| (s.ready_replicas, s.replicas))
                .unwrap_or((0, p.spec.replicas));

            let status = if p.is_ready() {
                "Running"
            } else if p.status.is_some() {
                "Pending"
            } else {
                "Unknown"
            };

            vec![
                p.metadata.namespace.clone(),
                p.metadata.name.clone(),
                p.spec.replicas.to_string(),
                format!("{}/{}", ready, total),
                status.to_string(),
            ]
        })
        .collect();

    format_table(headers, rows)
}

/// Format a single pipeline for detailed display
pub fn format_pipeline_detail(pipeline: &Pipeline) -> String {
    let mut output = String::new();

    output.push_str(&format!("Name:       {}\n", pipeline.metadata.name));
    output.push_str(&format!("Namespace:  {}\n", pipeline.metadata.namespace));
    output.push_str(&format!("UID:        {}\n", pipeline.metadata.uid));

    if let Some(ts) = &pipeline.metadata.creation_timestamp {
        output.push_str(&format!("Created:    {}\n", ts));
    }

    output.push_str(&format!("Replicas:   {}\n", pipeline.spec.replicas));
    output.push_str(&format!("Port:       {}\n", pipeline.spec.port));

    if !pipeline.metadata.labels.is_empty() {
        output.push_str("Labels:\n");
        for (k, v) in &pipeline.metadata.labels {
            output.push_str(&format!("  {}={}\n", k, v));
        }
    }

    if let Some(status) = &pipeline.status {
        output.push_str("\nStatus:\n");
        output.push_str(&format!(
            "  Ready Replicas:       {}\n",
            status.ready_replicas
        ));
        output.push_str(&format!(
            "  Available Replicas:   {}\n",
            status.available_replicas
        ));
        output.push_str(&format!(
            "  Unavailable Replicas: {}\n",
            status.unavailable_replicas
        ));

        if !status.endpoints.is_empty() {
            output.push_str("  Endpoints:\n");
            for endpoint in &status.endpoints {
                output.push_str(&format!("    - {}\n", endpoint));
            }
        }

        if !status.conditions.is_empty() {
            output.push_str("  Conditions:\n");
            for cond in &status.conditions {
                output.push_str(&format!(
                    "    {} = {} ({})\n",
                    cond.condition_type, cond.status, cond.reason
                ));
            }
        }
    }

    output.push_str("\nComposition:\n");
    output.push_str(&format!(
        "  Models: {}\n",
        pipeline.spec.composition.models.len()
    ));
    output.push_str(&format!(
        "  Nodes:  {}\n",
        pipeline.spec.composition.architecture.len()
    ));

    output
}

// ============================================================================
// Node display
// ============================================================================

/// Format node list for display
pub fn format_node_list(nodes: &[serde_json::Value]) -> String {
    let headers = &["NAME", "STATUS", "ADDRESS", "PIPELINES"];
    let rows: Vec<Vec<String>> = nodes
        .iter()
        .map(|n| {
            let name = n["metadata"]["name"].as_str().unwrap_or("?").to_string();
            let status = n["status"]["phase"]
                .as_str()
                .unwrap_or("Unknown")
                .to_string();
            let address = n["spec"]["address"].as_str().unwrap_or("?").to_string();
            let port = n["spec"]["port"].as_u64().unwrap_or(8080);
            let pipelines = n["status"]["pipelines"]
                .as_array()
                .map(|a| a.len())
                .unwrap_or(0);

            vec![
                name,
                status,
                format!("{}:{}", address, port),
                pipelines.to_string(),
            ]
        })
        .collect();

    format_table(headers, rows)
}

// ============================================================================
// Namespace display
// ============================================================================

/// Format namespace list for display
pub fn format_namespace_list(namespaces: &[serde_json::Value]) -> String {
    let headers = &["NAME"];
    let rows: Vec<Vec<String>> = namespaces
        .iter()
        .map(|ns| vec![ns["metadata"]["name"].as_str().unwrap_or("?").to_string()])
        .collect();

    format_table(headers, rows)
}

// ============================================================================
// Worker resource display (containers, runners)
// ============================================================================

/// Format container list for display
pub fn format_container_list(containers: &[String]) -> String {
    let headers = &["CONTAINER"];
    let rows: Vec<Vec<String>> = containers.iter().map(|c| vec![c.clone()]).collect();

    format_table(headers, rows)
}

/// Format runner list for display
pub fn format_runner_list(runners: &[serde_json::Value]) -> String {
    let headers = &["NAME", "MODEL", "ENDPOINT", "STATUS"];
    let rows: Vec<Vec<String>> = runners
        .iter()
        .map(|r| {
            vec![
                r["name"].as_str().unwrap_or("?").to_string(),
                r["model"].as_str().unwrap_or("?").to_string(),
                r["endpoint"].as_str().unwrap_or("?").to_string(),
                r["status"].as_str().unwrap_or("running").to_string(),
            ]
        })
        .collect();

    format_table(headers, rows)
}

// ============================================================================
// Validation display
// ============================================================================

/// Format validation result for display
pub fn format_validation_result(result: &ValidationResult, path: &str) -> String {
    let mut output = String::new();

    if result.valid {
        output.push_str(&format!("✓ {} is valid\n\n", path));
        output.push_str(&format!("  Models: {}\n", result.models));
        output.push_str(&format!("  Nodes:  {}\n", result.nodes));
    } else {
        output.push_str(&format!("✗ {} is invalid\n\n", path));
        if let Some(ref error) = result.error {
            output.push_str(&format!("  Error: {}\n", error));
        }
    }

    output
}

// ============================================================================
// Cluster status display
// ============================================================================

/// Format cluster status for display
pub fn format_cluster_status(status: &serde_json::Value) -> String {
    let mut output = String::new();

    output.push_str("Cluster Status\n");
    output.push_str("==============\n\n");

    if let Some(stats) = status.get("stats") {
        output.push_str(&format!(
            "Nodes:     {}/{} ready\n",
            stats["ready_nodes"].as_u64().unwrap_or(0),
            stats["total_nodes"].as_u64().unwrap_or(0)
        ));
        output.push_str(&format!(
            "Pipelines: {}/{} ready\n",
            stats["ready_pipelines"].as_u64().unwrap_or(0),
            stats["total_pipelines"].as_u64().unwrap_or(0)
        ));
        output.push_str(&format!(
            "Namespaces: {}\n",
            stats["namespaces"].as_u64().unwrap_or(0)
        ));
    }

    output
}

// ============================================================================
// Dry-run display (legacy)
// ============================================================================

use crate::cli::RunArgs;

/// Format a dry-run output showing the pipeline structure.
/// Pure function - returns a formatted string.
pub fn format_dry_run(composition: &Composition, args: &RunArgs) -> String {
    let mut output = String::new();

    output.push_str("llmnet v0.1.0 - Dry Run Mode\n\n");
    output.push_str(&format!(
        "Composition: {}\n\n",
        args.composition_file.display()
    ));

    // Models section
    output.push_str(&format!("Models ({}):\n", composition.models.len()));
    for (name, model) in &composition.models {
        output.push_str(&format!("  - {} [{}]\n", name, model.type_name()));
    }
    output.push('\n');

    // Architecture section
    output.push_str("Pipeline Architecture:\n");

    // Group nodes by layer
    let mut max_layer = 0u32;
    for node in &composition.architecture {
        if let Some(layer) = node.layer {
            max_layer = max_layer.max(layer);
        }
    }

    // Layer 0 (Input/Router)
    output.push_str("  Layer 0 (Input/Router):\n");
    for node in composition.nodes_in_layer(0) {
        let bind = format!(
            "{}:{}",
            node.effective_bind_addr(),
            node.bind_port.as_deref().unwrap_or("auto")
        );
        output.push_str(&format!("    [{}] {} -> ", node.name, bind));
        if let Some(ref target) = node.output_to {
            match target {
                crate::config::OutputTarget::Layers(layers) => {
                    output.push_str(&format!("Layer {:?}", layers));
                }
                crate::config::OutputTarget::Nodes(nodes) => {
                    output.push_str(&nodes.join(", "));
                }
            }
        }
        output.push('\n');
        if let Some(ref model) = node.model {
            output.push_str(&format!("      Model: {}\n", model));
        }
    }

    // Hidden layers
    for layer in 1..=max_layer {
        let nodes = composition.nodes_in_layer(layer);
        if !nodes.is_empty() {
            output.push_str(&format!("\n  Layer {} (Hidden):\n", layer));
            for node in nodes {
                let bind = format!(
                    "{}:{}",
                    node.effective_bind_addr(),
                    node.bind_port.as_deref().unwrap_or("auto")
                );
                output.push_str(&format!("    [{}] {}\n", node.name, bind));
                if let Some(ref use_case) = node.use_case {
                    let truncated = if use_case.len() > 60 {
                        format!("{}...", &use_case[..57])
                    } else {
                        use_case.clone()
                    };
                    output.push_str(&format!("      Use-case: {}\n", truncated));
                }
            }
        }
    }

    // Output layer (both "output" and "ws" adapters)
    let output_nodes: Vec<_> = composition
        .architecture
        .iter()
        .filter(|n| n.adapter == "output" || n.adapter == "ws")
        .collect();
    if !output_nodes.is_empty() {
        output.push_str("\n  Output Layer:\n");
        for node in output_nodes {
            output.push_str(&format!("    [{}] adapter: {}", node.name, node.adapter));
            if let Some(ref url) = node.url {
                output.push_str(&format!(" -> {}", url));
            }
            output.push('\n');
            if let Some(ref cond) = node.condition {
                output.push_str(&format!("      Condition: {}\n", cond));
            }
        }
    }

    output.push_str("\nValidation: PASSED\n");
    output.push_str("Ready to start pipeline. Remove --dry-run to execute.\n");

    output
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_format_table() {
        let headers = &["NAME", "AGE"];
        let rows = vec![
            vec!["Alice".to_string(), "30".to_string()],
            vec!["Bob".to_string(), "25".to_string()],
        ];

        let output = format_table(headers, rows);
        assert!(output.contains("NAME"));
        assert!(output.contains("Alice"));
        assert!(output.contains("Bob"));
    }

    #[test]
    fn test_format_table_empty() {
        let headers = &["NAME"];
        let rows: Vec<Vec<String>> = vec![];

        let output = format_table(headers, rows);
        assert!(output.contains("No resources found"));
    }

    #[test]
    fn test_format_context_list() {
        let contexts = vec![
            ContextInfo {
                name: "local".to_string(),
                url: "http://localhost:8181".to_string(),
                is_current: true,
            },
            ContextInfo {
                name: "remote".to_string(),
                url: "http://10.0.0.1:8181".to_string(),
                is_current: false,
            },
        ];

        let output = format_context_list(&contexts);
        assert!(output.contains("local"));
        assert!(output.contains("remote"));
        assert!(output.contains("*")); // Current marker
    }

    #[test]
    fn test_format_current_context() {
        let output = format_current_context("my-cluster", "http://10.0.0.1:8181");
        assert!(output.contains("my-cluster"));
        assert!(output.contains("http://10.0.0.1:8181"));
    }

    #[test]
    fn test_format_validation_valid() {
        let result = ValidationResult {
            valid: true,
            models: 2,
            nodes: 5,
            error: None,
        };

        let output = format_validation_result(&result, "test.json");
        assert!(output.contains("✓"));
        assert!(output.contains("valid"));
        assert!(output.contains("Models: 2"));
    }

    #[test]
    fn test_format_validation_invalid() {
        let result = ValidationResult {
            valid: false,
            models: 0,
            nodes: 0,
            error: Some("Parse error".to_string()),
        };

        let output = format_validation_result(&result, "test.json");
        assert!(output.contains("✗"));
        assert!(output.contains("invalid"));
        assert!(output.contains("Parse error"));
    }
}
