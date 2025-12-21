use clap::{ArgAction, Parser};
use std::path::PathBuf;

#[derive(Parser, Debug)]
#[command(name = "llmnet")]
#[command(about = "Orchestrate LLM pipelines based on composition files")]
#[command(version)]
pub struct Args {
    /// Path to the composition file (JSON or JSONC)
    #[arg(required = true)]
    pub composition_file: PathBuf,

    /// Enable verbose logging output (-v, -vv, -vvv)
    #[arg(short, long, action = ArgAction::Count)]
    pub verbose: u8,

    /// Dry-run mode: validate config and show pipeline without running
    #[arg(long)]
    pub dry_run: bool,

    /// Override the default bind address for all nodes
    #[arg(long, value_name = "ADDR")]
    pub bind_addr: Option<String>,

    /// Override the starting port for nodes
    #[arg(short, long, value_name = "PORT")]
    pub port: Option<u16>,

    /// Path to a .env file for loading API keys
    #[arg(long, value_name = "FILE")]
    pub env_file: Option<PathBuf>,

    /// Request timeout in seconds
    #[arg(long, default_value = "30")]
    pub timeout: u64,

    /// Maximum concurrent requests per node
    #[arg(long, default_value = "100")]
    pub max_concurrent: usize,
}

// ============================================================================
// SBIO: Pure display logic (no I/O - returns formatted strings)
// ============================================================================

use crate::config::Composition;

/// Format a dry-run output showing the pipeline structure.
/// Pure function - returns a formatted string.
pub fn format_dry_run(composition: &Composition, args: &Args) -> String {
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

    fn create_test_args() -> Args {
        Args {
            composition_file: PathBuf::from("test.json"),
            verbose: 0,
            dry_run: true,
            bind_addr: None,
            port: None,
            env_file: None,
            timeout: 30,
            max_concurrent: 100,
        }
    }

    #[test]
    fn test_format_dry_run_basic() {
        let json = r#"{
            "models": {
                "test-model": {
                    "type": "external",
                    "interface": "openai-api",
                    "url": "http://localhost:8080"
                }
            },
            "architecture": [
                {"name": "router", "layer": 0, "model": "test-model", "adapter": "openai-api", "output-to": [1]},
                {"name": "node1", "layer": 1, "adapter": "openai-api", "use-case": "Handle queries"},
                {"name": "output", "adapter": "output"}
            ]
        }"#;

        let composition = Composition::from_str(json).unwrap();
        let args = create_test_args();
        let output = format_dry_run(&composition, &args);

        assert!(output.contains("Models (1):"));
        assert!(output.contains("test-model"));
        assert!(output.contains("Layer 0 (Input/Router):"));
        assert!(output.contains("[router]"));
        assert!(output.contains("Validation: PASSED"));
    }

    #[test]
    fn test_clap_parsing() {
        let args = Args::parse_from(["llmnet", "composition.json"]);
        assert_eq!(args.composition_file, PathBuf::from("composition.json"));
        assert!(!args.dry_run);
    }

    #[test]
    fn test_clap_dry_run() {
        let args = Args::parse_from(["llmnet", "--dry-run", "composition.json"]);
        assert!(args.dry_run);
    }

    #[test]
    fn test_clap_verbose() {
        let args = Args::parse_from(["llmnet", "-vvv", "composition.json"]);
        assert_eq!(args.verbose, 3);
    }

    #[test]
    fn test_clap_overrides() {
        let args = Args::parse_from([
            "llmnet",
            "--bind-addr",
            "127.0.0.1",
            "--port",
            "9000",
            "composition.json",
        ]);
        assert_eq!(args.bind_addr, Some("127.0.0.1".to_string()));
        assert_eq!(args.port, Some(9000));
    }
}
