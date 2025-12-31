//! CLI module for llmnet
//!
//! Provides kubectl-like subcommands:
//! - `llmnet serve` - Run as control plane or local pipeline server
//! - `llmnet deploy` - Deploy a pipeline to the current context
//! - `llmnet get` - List resources (pipelines, nodes, namespaces)
//! - `llmnet delete` - Delete resources
//! - `llmnet scale` - Scale pipelines
//! - `llmnet context` - Manage contexts
//! - `llmnet logs` - View pipeline logs

use clap::{ArgAction, Parser, Subcommand};
use std::path::PathBuf;

mod commands;
mod display;

pub use commands::*;
pub use display::*;

#[derive(Parser, Debug)]
#[command(name = "llmnet")]
#[command(about = "The Kubernetes of AI - Orchestrate LLM pipelines")]
#[command(version)]
pub struct Cli {
    /// Enable verbose logging output (-v, -vv, -vvv)
    #[arg(short, long, action = ArgAction::Count, global = true)]
    pub verbose: u8,

    /// Path to config file (default: ~/.llmnet/config)
    #[arg(long, global = true)]
    pub config: Option<PathBuf>,

    #[command(subcommand)]
    pub command: Commands,
}

#[derive(Subcommand, Debug)]
pub enum Commands {
    /// Run the control plane server or a local pipeline
    Serve(ServeArgs),

    /// Deploy a pipeline to the current context
    Deploy(DeployArgs),

    /// Get/list resources
    Get(GetArgs),

    /// Delete a resource
    Delete(DeleteArgs),

    /// Scale a pipeline
    Scale(ScaleArgs),

    /// Manage cluster contexts
    Context(ContextArgs),

    /// View pipeline logs
    Logs(LogsArgs),

    /// Show cluster status
    Status,

    /// Validate a composition file
    Validate(ValidateArgs),

    /// Run a local pipeline server (legacy mode)
    #[command(name = "run")]
    Run(RunArgs),

    /// Stop a running container (graceful shutdown)
    Stop(StopArgs),

    /// Kill a running container (force shutdown)
    Kill(KillArgs),
}

/// Arguments for the serve command
#[derive(Parser, Debug)]
pub struct ServeArgs {
    /// Run as control plane (manages cluster state)
    #[arg(long)]
    pub control_plane: bool,

    /// Bind address for the server
    #[arg(long, default_value = "0.0.0.0")]
    pub bind_addr: String,

    /// Advertise address for the worker (how control plane can reach this worker)
    /// If not specified, uses bind_addr (must be reachable, not 0.0.0.0)
    #[arg(long)]
    pub advertise_addr: Option<String>,

    /// Port to listen on (default: 8181 for control plane, 8080 for worker)
    #[arg(short, long)]
    pub port: Option<u16>,

    /// Path to a .env file for loading API keys
    #[arg(long, value_name = "FILE")]
    pub env_file: Option<PathBuf>,

    /// Node name when registering with control plane
    #[arg(long)]
    pub node_name: Option<String>,

    /// Control plane URL to register with (worker mode)
    #[arg(long)]
    pub control_plane_url: Option<String>,

    /// Force restart even if already running and healthy
    #[arg(long)]
    pub force: bool,
}

/// Arguments for the deploy command
#[derive(Parser, Debug)]
pub struct DeployArgs {
    /// Path to the pipeline manifest (JSON or YAML)
    pub file: PathBuf,

    /// Namespace to deploy to (default: "default")
    #[arg(short, long, default_value = "default")]
    pub namespace: String,

    /// Dry-run mode: validate without deploying
    #[arg(long)]
    pub dry_run: bool,
}

/// Arguments for the get command
#[derive(Parser, Debug)]
pub struct GetArgs {
    /// Resource type to list
    #[command(subcommand)]
    pub resource: GetResource,
}

#[derive(Subcommand, Debug)]
pub enum GetResource {
    // ============================================================================
    // Control plane resources (require control plane context)
    // ============================================================================
    /// List pipelines
    #[command(name = "pipelines", visible_alias = "pipeline", visible_alias = "pl")]
    Pipelines {
        /// Namespace (omit for all namespaces)
        #[arg(short, long)]
        namespace: Option<String>,

        /// Show all namespaces
        #[arg(short = 'A', long)]
        all_namespaces: bool,
    },

    /// List nodes
    #[command(name = "nodes", visible_alias = "node", visible_alias = "no")]
    Nodes,

    /// List namespaces
    #[command(name = "namespaces", visible_alias = "namespace", visible_alias = "ns")]
    Namespaces,

    // ============================================================================
    // Worker resources (require worker context)
    // ============================================================================
    /// List local Docker containers (worker mode)
    #[command(name = "containers", visible_alias = "container", visible_alias = "c")]
    Containers,

    /// List local model runners (worker mode)
    #[command(name = "runners", visible_alias = "runner", visible_alias = "r")]
    Runners,
}

/// Arguments for the delete command
#[derive(Parser, Debug)]
pub struct DeleteArgs {
    /// Resource type and name (e.g., "pipeline my-pipeline")
    #[command(subcommand)]
    pub resource: DeleteResource,
}

#[derive(Subcommand, Debug)]
pub enum DeleteResource {
    /// Delete a pipeline
    #[command(name = "pipeline", visible_alias = "pl")]
    Pipeline {
        /// Pipeline name
        name: String,

        /// Namespace
        #[arg(short, long, default_value = "default")]
        namespace: String,
    },

    /// Delete a node
    #[command(name = "node", visible_alias = "no")]
    Node {
        /// Node name
        name: String,
    },
}

/// Arguments for the scale command
#[derive(Parser, Debug)]
pub struct ScaleArgs {
    /// Pipeline name
    pub name: String,

    /// Number of replicas
    #[arg(long)]
    pub replicas: u32,

    /// Namespace
    #[arg(short, long, default_value = "default")]
    pub namespace: String,
}

/// Arguments for the context command
#[derive(Parser, Debug)]
pub struct ContextArgs {
    #[command(subcommand)]
    pub action: ContextAction,
}

#[derive(Subcommand, Debug)]
pub enum ContextAction {
    /// List all contexts
    List,

    /// Show current context
    Current,

    /// Switch to a context
    Use {
        /// Context name
        name: String,
    },

    /// Add a new context
    Add {
        /// Context name
        name: String,

        /// Server URL
        #[arg(long)]
        url: String,

        /// API key for authentication
        #[arg(long)]
        api_key: Option<String>,
    },

    /// Delete a context
    Delete {
        /// Context name
        name: String,
    },
}

/// Arguments for the logs command
#[derive(Parser, Debug)]
pub struct LogsArgs {
    /// Pipeline name
    pub name: String,

    /// Namespace
    #[arg(short, long, default_value = "default")]
    pub namespace: String,

    /// Follow logs (like tail -f)
    #[arg(short, long)]
    pub follow: bool,

    /// Number of lines to show
    #[arg(long, default_value = "100")]
    pub tail: usize,
}

/// Arguments for the validate command
#[derive(Parser, Debug)]
pub struct ValidateArgs {
    /// Path to the composition file
    pub file: PathBuf,
}

/// Arguments for the legacy run command
#[derive(Parser, Debug)]
pub struct RunArgs {
    /// Path to the composition file (JSON or JSONC)
    pub composition_file: PathBuf,

    /// Dry-run mode: validate config and show pipeline without running
    #[arg(long)]
    pub dry_run: bool,

    /// Override the default bind address
    #[arg(long, value_name = "ADDR")]
    pub bind_addr: Option<String>,

    /// Override the starting port
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

/// Arguments for the stop command
#[derive(Parser, Debug)]
pub struct StopArgs {
    /// Container name to stop
    pub name: String,

    /// Timeout in seconds before force killing (default: 10)
    #[arg(short, long, default_value = "10")]
    pub timeout: u64,
}

/// Arguments for the kill command
#[derive(Parser, Debug)]
pub struct KillArgs {
    /// Container name to kill
    pub name: String,
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_parse_serve_control_plane() {
        let cli = Cli::parse_from(["llmnet", "serve", "--control-plane"]);
        match cli.command {
            Commands::Serve(args) => {
                assert!(args.control_plane);
            }
            _ => panic!("Expected Serve command"),
        }
    }

    #[test]
    fn test_parse_deploy() {
        let cli = Cli::parse_from(["llmnet", "deploy", "pipeline.json"]);
        match cli.command {
            Commands::Deploy(args) => {
                assert_eq!(args.file, PathBuf::from("pipeline.json"));
                assert_eq!(args.namespace, "default");
            }
            _ => panic!("Expected Deploy command"),
        }
    }

    #[test]
    fn test_parse_get_pipelines() {
        let cli = Cli::parse_from(["llmnet", "get", "pipelines"]);
        match cli.command {
            Commands::Get(args) => match args.resource {
                GetResource::Pipelines { .. } => {}
                _ => panic!("Expected Pipelines resource"),
            },
            _ => panic!("Expected Get command"),
        }
    }

    #[test]
    fn test_parse_get_nodes() {
        let cli = Cli::parse_from(["llmnet", "get", "nodes"]);
        match cli.command {
            Commands::Get(args) => match args.resource {
                GetResource::Nodes => {}
                _ => panic!("Expected Nodes resource"),
            },
            _ => panic!("Expected Get command"),
        }
    }

    #[test]
    fn test_parse_context_use() {
        let cli = Cli::parse_from(["llmnet", "context", "use", "my-cluster"]);
        match cli.command {
            Commands::Context(args) => match args.action {
                ContextAction::Use { name } => {
                    assert_eq!(name, "my-cluster");
                }
                _ => panic!("Expected Use action"),
            },
            _ => panic!("Expected Context command"),
        }
    }

    #[test]
    fn test_parse_context_add() {
        let cli = Cli::parse_from([
            "llmnet",
            "context",
            "add",
            "remote",
            "--url",
            "http://10.0.0.1:8181",
        ]);
        match cli.command {
            Commands::Context(args) => match args.action {
                ContextAction::Add { name, url, .. } => {
                    assert_eq!(name, "remote");
                    assert_eq!(url, "http://10.0.0.1:8181");
                }
                _ => panic!("Expected Add action"),
            },
            _ => panic!("Expected Context command"),
        }
    }

    #[test]
    fn test_parse_scale() {
        let cli = Cli::parse_from(["llmnet", "scale", "my-pipeline", "--replicas", "5"]);
        match cli.command {
            Commands::Scale(args) => {
                assert_eq!(args.name, "my-pipeline");
                assert_eq!(args.replicas, 5);
            }
            _ => panic!("Expected Scale command"),
        }
    }

    #[test]
    fn test_parse_delete_pipeline() {
        let cli = Cli::parse_from(["llmnet", "delete", "pipeline", "my-pipeline"]);
        match cli.command {
            Commands::Delete(args) => match args.resource {
                DeleteResource::Pipeline { name, .. } => {
                    assert_eq!(name, "my-pipeline");
                }
                _ => panic!("Expected Pipeline delete"),
            },
            _ => panic!("Expected Delete command"),
        }
    }

    #[test]
    fn test_parse_legacy_run() {
        let cli = Cli::parse_from(["llmnet", "run", "config.json"]);
        match cli.command {
            Commands::Run(args) => {
                assert_eq!(args.composition_file, PathBuf::from("config.json"));
            }
            _ => panic!("Expected Run command"),
        }
    }

    #[test]
    fn test_verbose_global() {
        let cli = Cli::parse_from(["llmnet", "-vvv", "status"]);
        assert_eq!(cli.verbose, 3);
    }
}
