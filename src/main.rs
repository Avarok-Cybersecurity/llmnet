use std::process;

use clap::Parser;
use tracing::{error, info};
use tracing_subscriber::EnvFilter;

use llmnet::cli::{format_dry_run, Args};
use llmnet::config::load_composition_file;
use llmnet::server::{create_router, AppState};

#[tokio::main]
async fn main() {
    let args = Args::parse();

    // Initialize logging
    let filter = match args.verbose {
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

    // Load .env file if specified
    if let Some(ref env_file) = args.env_file {
        if let Err(e) = dotenvy::from_path(env_file) {
            error!("Failed to load env file {}: {}", env_file.display(), e);
            process::exit(1);
        }
    }

    // Load and validate composition
    let composition = match load_composition_file(&args.composition_file) {
        Ok(comp) => comp,
        Err(e) => {
            error!(
                "Failed to load composition file {}: {}",
                args.composition_file.display(),
                e
            );
            process::exit(1);
        }
    };

    // Dry-run mode: print pipeline info and exit
    if args.dry_run {
        let output = format_dry_run(&composition, &args);
        println!("{}", output);
        return;
    }

    // Create application state
    let state = AppState::new(composition);

    // Get router node info for binding
    let bind_addr = args.bind_addr.as_deref().unwrap_or("0.0.0.0");
    let port = args.port.unwrap_or(8080);
    let addr = format!("{}:{}", bind_addr, port);

    info!("Starting llmnet on {}", addr);
    info!("Loaded {} nodes", state.nodes.len());

    // Create and run the server
    let app = create_router(state);

    let listener = match tokio::net::TcpListener::bind(&addr).await {
        Ok(l) => l,
        Err(e) => {
            error!("Failed to bind to {}: {}", addr, e);
            process::exit(1);
        }
    };

    info!("Server listening on {}", addr);
    info!("Endpoints:");
    info!("  GET  /health             - Health check");
    info!("  GET  /status             - Pipeline status");
    info!("  POST /v1/chat/completions - OpenAI-compatible chat endpoint");

    if let Err(e) = axum::serve(listener, app).await {
        error!("Server error: {}", e);
        process::exit(1);
    }
}
