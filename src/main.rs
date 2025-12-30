use std::process;

use clap::Parser;
use tracing::{error, info, warn};
use tracing_subscriber::EnvFilter;

use llmnet::cli::{
    format_cluster_status, format_context_list, format_current_context, format_dry_run,
    format_namespace_list, format_node_list, format_pipeline_detail, format_pipeline_list,
    format_validation_result, Cli, Commands, ContextAction, ControlPlaneClient, DeleteResource,
    GetResource, KillArgs, StopArgs,
};
use llmnet::runtime::new_shared_manager;
use llmnet::cluster::{
    create_control_plane_router, spawn_heartbeat, spawn_orchestrator, ControlPlaneState,
    HeartbeatConfig, Node, NodeCapacity, OrchestratorConfig, Pipeline, CONTROL_PLANE_PORT,
};
use llmnet::metrics::new_shared_collector;
use llmnet::config::load_composition_file;
use llmnet::context;
use llmnet::server::{create_router, AppState};

#[tokio::main]
async fn main() {
    let cli = Cli::parse();

    // Initialize logging
    let filter = match cli.verbose {
        0 => "warn",
        1 => "info",
        2 => "debug",
        _ => "trace",
    };

    tracing_subscriber::fmt()
        .with_env_filter(
            EnvFilter::try_from_default_env().unwrap_or_else(|_| EnvFilter::new(filter)),
        )
        .init();

    // Load context config
    let config_path = cli.config.unwrap_or_else(context::default_config_path);
    let mut config = context::load_config_from(&config_path).unwrap_or_default();

    // Execute command
    let result = match cli.command {
        Commands::Serve(args) => run_serve(args).await,
        Commands::Deploy(args) => run_deploy(&config, args).await,
        Commands::Get(args) => run_get(&config, args).await,
        Commands::Delete(args) => run_delete(&config, args).await,
        Commands::Scale(args) => run_scale(&config, args).await,
        Commands::Context(args) => run_context(&mut config, &config_path, args),
        Commands::Logs(args) => run_logs(&config, args).await,
        Commands::Status => run_status(&config).await,
        Commands::Validate(args) => run_validate(args),
        Commands::Run(args) => run_legacy(args).await,
        Commands::Stop(args) => run_stop(args).await,
        Commands::Kill(args) => run_kill(args).await,
    };

    if let Err(e) = result {
        error!("{}", e);
        process::exit(1);
    }
}

// ============================================================================
// Command Handlers
// ============================================================================

async fn run_serve(args: llmnet::cli::ServeArgs) -> Result<(), Box<dyn std::error::Error>> {
    // Load .env file if specified
    if let Some(ref env_file) = args.env_file {
        if let Err(e) = dotenvy::from_path(env_file) {
            error!("Failed to load env file {}: {}", env_file.display(), e);
            process::exit(1);
        }
    }

    if args.control_plane {
        // Run as control plane server
        let port = args.port.unwrap_or(CONTROL_PLANE_PORT);
        let addr = format!("{}:{}", args.bind_addr, port);

        info!("Starting LLMNet control plane on {}", addr);

        let state = ControlPlaneState::new();

        // Spawn the orchestrator to schedule pipelines to workers
        let _orchestrator_shutdown = spawn_orchestrator(
            state.controller.clone(),
            OrchestratorConfig::default(),
        );
        info!("Orchestrator started - will schedule pipelines to workers");

        let app = create_control_plane_router(state);

        let listener = tokio::net::TcpListener::bind(&addr).await?;

        info!("Control plane listening on {}", addr);
        info!("Endpoints:");
        info!("  GET  /health                     - Health check");
        info!("  GET  /v1/status                  - Cluster status");
        info!("  GET  /v1/pipelines               - List all pipelines");
        info!("  POST /v1/pipelines               - Deploy pipeline");
        info!("  GET  /v1/nodes                   - List nodes");
        info!("  POST /v1/nodes                   - Register node");

        axum::serve(listener, app).await?;
    } else {
        // Run as worker node
        let port = args.port.unwrap_or(8080);
        let addr = format!("{}:{}", args.bind_addr, port);
        let node_name = args.node_name.unwrap_or_else(|| {
            hostname::get()
                .ok()
                .and_then(|h| h.into_string().ok())
                .unwrap_or_else(|| "worker".to_string())
        });

        // Create metrics collector for heartbeats
        let metrics_collector = new_shared_collector();

        // Optional: register with control plane and start heartbeat
        let _heartbeat_shutdown = if let Some(ref cp_url) = args.control_plane_url {
            info!(
                "Starting LLMNet worker '{}', registering with control plane at {}",
                node_name, cp_url
            );

            // Register node with control plane
            let client = reqwest::Client::new();
            let node = Node::new(&node_name, &args.bind_addr).with_port(port);

            match client
                .post(format!("{}/v1/nodes", cp_url))
                .json(&node)
                .send()
                .await
            {
                Ok(resp) if resp.status().is_success() => {
                    info!("Node '{}' registered with control plane", node_name);
                }
                Ok(resp) => {
                    let status = resp.status();
                    let body = resp.text().await.unwrap_or_default();
                    warn!(
                        "Failed to register node ({}): {}",
                        status, body
                    );
                }
                Err(e) => {
                    warn!("Failed to connect to control plane: {}", e);
                }
            }

            // Start heartbeat client
            let heartbeat_config = HeartbeatConfig::new(cp_url.clone(), node_name.clone())
                .with_capacity(NodeCapacity::default());

            Some(spawn_heartbeat(heartbeat_config, metrics_collector.clone()))
        } else {
            None
        };

        info!("Starting LLMNet worker '{}' on {}", node_name, addr);

        // Create runner manager for spawning model containers
        let runner_manager = new_shared_manager();

        // For now, run an empty worker that just responds to health checks
        // and waits for pipeline assignments from the control plane
        let json = r#"{
            "models": {},
            "architecture": [
                {"name": "router", "layer": 0, "adapter": "openai-api"},
                {"name": "output", "adapter": "output"}
            ]
        }"#;
        let composition = llmnet::config::Composition::from_str(json)?;
        let state = AppState::new(composition)
            .with_runner_manager(runner_manager)
            .with_bind_addr(&args.bind_addr);
        let app = create_router(state);

        let listener = tokio::net::TcpListener::bind(&addr).await?;

        info!("Worker endpoints:");
        info!("  GET  /health          - Health check");
        info!("  POST /v1/assignments  - Receive pipeline assignments from control plane");
        info!("  POST /v1/runners/spawn - Spawn model runners");

        axum::serve(listener, app).await?;
    }

    Ok(())
}

async fn run_deploy(
    config: &context::Config,
    args: llmnet::cli::DeployArgs,
) -> Result<(), Box<dyn std::error::Error>> {
    // Load the pipeline manifest
    let pipeline = if args.file.extension().and_then(|e| e.to_str()) == Some("yaml")
        || args.file.extension().and_then(|e| e.to_str()) == Some("yml")
    {
        // YAML pipeline manifest
        let content = std::fs::read_to_string(&args.file)?;
        serde_yaml::from_str::<Pipeline>(&content)?
    } else {
        // Try as pipeline JSON, fall back to composition
        let content = std::fs::read_to_string(&args.file)?;
        match serde_json::from_str::<Pipeline>(&content) {
            Ok(p) => p,
            Err(_) => {
                // Fall back to composition format
                let composition = load_composition_file(&args.file)?;
                let name = args
                    .file
                    .file_stem()
                    .and_then(|s| s.to_str())
                    .unwrap_or("pipeline");
                Pipeline::new(name, composition).with_namespace(&args.namespace)
            }
        }
    };

    if args.dry_run {
        println!("Dry-run mode: would deploy pipeline '{}'", pipeline.metadata.name);
        println!("{}", format_pipeline_detail(&pipeline));
        return Ok(());
    }

    // Deploy to current context
    let client = ControlPlaneClient::from_context(config)?;
    let deployed = client.deploy(&pipeline).await?;

    println!(
        "pipeline.llmnet/{} deployed to namespace {}",
        deployed.metadata.name, deployed.metadata.namespace
    );

    Ok(())
}

async fn run_get(
    config: &context::Config,
    args: llmnet::cli::GetArgs,
) -> Result<(), Box<dyn std::error::Error>> {
    let client = ControlPlaneClient::from_context(config)?;

    match args.resource {
        GetResource::Pipelines {
            namespace,
            all_namespaces,
        } => {
            let ns = if all_namespaces { None } else { namespace.as_deref() };
            let pipelines = client.list_pipelines(ns).await?;
            print!("{}", format_pipeline_list(&pipelines));
        }
        GetResource::Nodes => {
            let nodes = client.list_nodes().await?;
            print!("{}", format_node_list(&nodes));
        }
        GetResource::Namespaces => {
            let namespaces = client.list_namespaces().await?;
            print!("{}", format_namespace_list(&namespaces));
        }
    }

    Ok(())
}

async fn run_delete(
    config: &context::Config,
    args: llmnet::cli::DeleteArgs,
) -> Result<(), Box<dyn std::error::Error>> {
    let client = ControlPlaneClient::from_context(config)?;

    match args.resource {
        DeleteResource::Pipeline { name, namespace } => {
            if client.delete_pipeline(&namespace, &name).await? {
                println!("pipeline.llmnet/{} deleted", name);
            } else {
                error!("Pipeline '{}' not found in namespace '{}'", name, namespace);
                process::exit(1);
            }
        }
        DeleteResource::Node { name } => {
            if client.delete_node(&name).await? {
                println!("node.llmnet/{} deleted", name);
            } else {
                error!("Node '{}' not found", name);
                process::exit(1);
            }
        }
    }

    Ok(())
}

async fn run_scale(
    config: &context::Config,
    args: llmnet::cli::ScaleArgs,
) -> Result<(), Box<dyn std::error::Error>> {
    let client = ControlPlaneClient::from_context(config)?;

    let pipeline = client
        .scale_pipeline(&args.namespace, &args.name, args.replicas)
        .await?;

    println!(
        "pipeline.llmnet/{} scaled to {} replicas",
        pipeline.metadata.name, pipeline.spec.replicas
    );

    Ok(())
}

fn run_context(
    config: &mut context::Config,
    config_path: &std::path::PathBuf,
    args: llmnet::cli::ContextArgs,
) -> Result<(), Box<dyn std::error::Error>> {
    match args.action {
        ContextAction::List => {
            let contexts = llmnet::cli::context_list(config);
            print!("{}", format_context_list(&contexts));
        }
        ContextAction::Current => {
            let (name, url) = llmnet::cli::context_current(config)?;
            print!("{}", format_current_context(&name, &url));
        }
        ContextAction::Use { name } => {
            llmnet::cli::context_use(config, &name)?;
            context::save_config_to(config, config_path)?;
            println!("Switched to context '{}'", name);
        }
        ContextAction::Add { name, url, api_key } => {
            llmnet::cli::context_add(config, &name, &url, api_key.as_deref())?;
            context::save_config_to(config, config_path)?;
            println!("Context '{}' added", name);
        }
        ContextAction::Delete { name } => {
            if llmnet::cli::context_delete(config, &name)? {
                context::save_config_to(config, config_path)?;
                println!("Context '{}' deleted", name);
            } else {
                error!("Context '{}' not found", name);
                process::exit(1);
            }
        }
    }

    Ok(())
}

async fn run_logs(
    _config: &context::Config,
    args: llmnet::cli::LogsArgs,
) -> Result<(), Box<dyn std::error::Error>> {
    // TODO: Implement log streaming
    warn!(
        "Log streaming not yet implemented for pipeline '{}/{}'",
        args.namespace, args.name
    );
    if args.follow {
        warn!("Follow mode not yet implemented");
    }
    Ok(())
}

async fn run_status(config: &context::Config) -> Result<(), Box<dyn std::error::Error>> {
    let client = ControlPlaneClient::from_context(config)?;
    let status = client.status().await?;
    print!("{}", format_cluster_status(&status));
    Ok(())
}

fn run_validate(args: llmnet::cli::ValidateArgs) -> Result<(), Box<dyn std::error::Error>> {
    let result = llmnet::cli::validate_composition(&args.file)?;
    print!(
        "{}",
        format_validation_result(&result, &args.file.display().to_string())
    );

    if !result.valid {
        process::exit(1);
    }

    Ok(())
}

async fn run_legacy(args: llmnet::cli::RunArgs) -> Result<(), Box<dyn std::error::Error>> {
    use llmnet::config::models::RunnerType;
    use tokio::signal;

    // Load .env file if specified
    if let Some(ref env_file) = args.env_file {
        if let Err(e) = dotenvy::from_path(env_file) {
            error!("Failed to load env file {}: {}", env_file.display(), e);
            process::exit(1);
        }
    }

    // Load and validate composition
    let mut composition = load_composition_file(&args.composition_file)?;

    // Dry-run mode: print pipeline info and exit
    if args.dry_run {
        let output = format_dry_run(&composition, &args);
        println!("{}", output);
        return Ok(());
    }

    // Create runner manager for local runners (Docker, Ollama, vLLM, llama.cpp)
    let runner_manager = new_shared_manager();

    // Collect models that need runners
    let models_needing_runners: Vec<_> = composition
        .models
        .iter()
        .filter_map(|(name, def)| {
            let config = def.to_config();
            let needs_runner = matches!(
                config.runner,
                RunnerType::Docker | RunnerType::Ollama | RunnerType::Vllm | RunnerType::LlamaCpp
            );
            if needs_runner {
                Some((name.clone(), config))
            } else {
                None
            }
        })
        .collect();

    // Spawn runners and collect endpoint updates
    let mut endpoint_updates: Vec<(String, String)> = Vec::new();

    for (model_name, config) in models_needing_runners {
        info!("Spawning {} runner for model '{}'...", config.type_name(), model_name);

        match runner_manager.spawn_runner(&model_name, &config).await {
            Ok(endpoint) => {
                info!("Runner for '{}' ready at {}", model_name, endpoint);
                endpoint_updates.push((model_name, endpoint));
            }
            Err(e) => {
                error!("Failed to spawn runner for '{}': {}", model_name, e);
                // Shutdown any already-spawned runners
                runner_manager.shutdown_all().await;
                return Err(e.into());
            }
        }
    }

    // Apply endpoint updates to composition
    for (model_name, endpoint) in endpoint_updates {
        if let Some(model) = composition.models.get_mut(&model_name) {
            let mut updated_config = model.to_config();
            updated_config.endpoint = Some(endpoint);
            *model = llmnet::config::models::ModelDefinition::Unified(updated_config);
        }
    }

    // Create application state with updated composition
    let state = AppState::new(composition);

    // Get router node info for binding
    let bind_addr = args.bind_addr.as_deref().unwrap_or("0.0.0.0");
    let port = args.port.unwrap_or(8080);
    let addr = format!("{}:{}", bind_addr, port);

    info!("Starting llmnet on {}", addr);
    info!("Loaded {} nodes", state.nodes.len());

    // Create the router
    let app = create_router(state);

    let listener = tokio::net::TcpListener::bind(&addr).await?;

    info!("Server listening on {}", addr);
    info!("Endpoints:");
    info!("  GET  /health             - Health check");
    info!("  GET  /status             - Pipeline status");
    info!("  POST /v1/chat/completions - OpenAI-compatible chat endpoint");

    // Clone runner_manager for the shutdown handler
    let shutdown_manager = runner_manager.clone();

    // Spawn the server with graceful shutdown
    let server = axum::serve(listener, app).with_graceful_shutdown(async move {
        let ctrl_c = async {
            signal::ctrl_c()
                .await
                .expect("Failed to install Ctrl+C handler");
        };

        #[cfg(unix)]
        let terminate = async {
            signal::unix::signal(signal::unix::SignalKind::terminate())
                .expect("Failed to install signal handler")
                .recv()
                .await;
        };

        #[cfg(not(unix))]
        let terminate = std::future::pending::<()>();

        tokio::select! {
            _ = ctrl_c => {},
            _ = terminate => {},
        }

        info!("Shutdown signal received, stopping runners...");
        shutdown_manager.shutdown_all().await;
        info!("All runners stopped");
    });

    server.await?;

    Ok(())
}

async fn run_stop(args: StopArgs) -> Result<(), Box<dyn std::error::Error>> {
    use tokio::process::Command;

    info!("Stopping container '{}'...", args.name);

    let output = Command::new("docker")
        .args(["stop", "-t", &args.timeout.to_string(), &args.name])
        .output()
        .await?;

    if output.status.success() {
        info!("Container '{}' stopped", args.name);
        println!("{}", args.name);
    } else {
        let stderr = String::from_utf8_lossy(&output.stderr);
        error!("Failed to stop container: {}", stderr);
        process::exit(1);
    }

    Ok(())
}

async fn run_kill(args: KillArgs) -> Result<(), Box<dyn std::error::Error>> {
    use tokio::process::Command;

    info!("Killing container '{}'...", args.name);

    let output = Command::new("docker")
        .args(["kill", &args.name])
        .output()
        .await?;

    if output.status.success() {
        info!("Container '{}' killed", args.name);
        println!("{}", args.name);
    } else {
        let stderr = String::from_utf8_lossy(&output.stderr);
        error!("Failed to kill container: {}", stderr);
        process::exit(1);
    }

    Ok(())
}
