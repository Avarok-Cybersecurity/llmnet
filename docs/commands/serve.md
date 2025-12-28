# llmnet serve

Start an LLMNet server that can either act as a **control plane** (managing pipelines across multiple machines) or as a **worker node** (running actual LLM pipelines).

## Synopsis

```
llmnet serve [OPTIONS]
```

## Arguments

| Argument | Type | Default | Description |
|----------|------|---------|-------------|
| `--control-plane` | flag | false | Run as a control plane server instead of a worker |
| `--bind-addr` | string | `0.0.0.0` | IP address to bind the server to |
| `-p, --port` | number | 8181 (control plane) or 8080 (worker) | Port to listen on |
| `--env-file` | path | none | Path to a `.env` file for loading API keys |
| `--node-name` | string | none | Name to identify this node when registering with a control plane |
| `--control-plane-url` | string | none | URL of the control plane to register with (worker mode only) |

## What It Does

The `serve` command starts a long-running server process. It has two modes:

### Control Plane Mode (`--control-plane`)

Think of this as the "manager" server. It doesn't run any LLM pipelines itself—instead, it:

- **Tracks which pipelines are deployed** across your cluster
- **Knows which worker nodes are available** and their capacity
- **Accepts deployment requests** from the CLI or API
- **Schedules pipelines** onto available worker nodes

This is similar to how a web application might have a central API server that coordinates work across multiple backend services.

### Worker Mode (default)

This is the actual "worker" that runs LLM pipelines. It:

- **Processes incoming chat requests** through the configured pipeline
- **Communicates with LLM backends** (Ollama, vLLM, OpenRouter, etc.)
- **Can register with a control plane** to be part of a managed cluster

## Examples

### Start a Control Plane

```bash
llmnet serve --control-plane
```

**What happens:** Starts the control plane server on port 8181. This server will:
- Accept pipeline deployments via `llmnet deploy`
- Track registered worker nodes
- Provide cluster status via `llmnet status`

You'll see output like:
```
INFO Starting LLMNet control plane on 0.0.0.0:8181
INFO Control plane listening on 0.0.0.0:8181
INFO Endpoints:
INFO   GET  /health                     - Health check
INFO   GET  /v1/status                  - Cluster status
INFO   GET  /v1/pipelines               - List all pipelines
INFO   POST /v1/pipelines               - Deploy pipeline
```

### Start a Control Plane on a Custom Port

```bash
llmnet serve --control-plane --port 9000
```

**What happens:** Same as above, but listens on port 9000 instead of the default 8181. Useful when running multiple services on the same machine or when port 8181 is already in use.

### Start a Worker Node

```bash
llmnet serve --port 8080
```

**What happens:** Starts a basic worker server on port 8080. In the current version, this runs a minimal pipeline. In future versions, workers will receive pipeline assignments from the control plane.

### Start a Worker and Register with Control Plane

```bash
llmnet serve \
  --node-name "gpu-worker-1" \
  --control-plane-url "http://10.0.0.1:8181"
```

**What happens:** Starts a worker that:
1. Names itself "gpu-worker-1" for identification
2. Attempts to register with the control plane at 10.0.0.1:8181
3. Will receive pipeline deployments from the control plane

> **Note:** Node registration is planned for a future release. Currently, this will start the worker but won't complete registration.

### Bind to Specific Interface

```bash
llmnet serve --control-plane --bind-addr 127.0.0.1
```

**What happens:** Starts the control plane but only accepts connections from localhost. Useful for:
- Development/testing
- When you want to use a reverse proxy (nginx, Caddy) in front
- Security: prevents external access

### Load API Keys from Environment File

```bash
llmnet serve --control-plane --env-file /etc/llmnet/.env
```

**What happens:** Before starting the server, loads environment variables from the specified file. This is useful for:
- Storing API keys (OpenRouter, etc.) outside of configuration files
- Different configurations per environment (dev, staging, production)

## Common Patterns

### Development Setup

Run everything locally on different ports:

```bash
# Terminal 1: Start control plane
llmnet serve --control-plane --port 8181

# Terminal 2: Start a worker
llmnet serve --port 8080
```

### Production Setup

On your management server:
```bash
llmnet serve --control-plane --bind-addr 0.0.0.0 --port 8181
```

On each GPU worker:
```bash
llmnet serve \
  --node-name "$(hostname)" \
  --control-plane-url "http://manager.internal:8181" \
  --port 8080
```

### Behind a Reverse Proxy

```bash
# Bind only to localhost, let nginx handle external traffic
llmnet serve --control-plane --bind-addr 127.0.0.1 --port 8181
```

## API Endpoints (Control Plane Mode)

When running as a control plane, these endpoints are available:

| Method | Path | Description |
|--------|------|-------------|
| GET | `/health` | Health check (returns 200 OK) |
| GET | `/v1/status` | Cluster statistics |
| GET | `/v1/pipelines` | List all deployed pipelines |
| POST | `/v1/pipelines` | Deploy a new pipeline |
| GET | `/v1/nodes` | List registered worker nodes |
| POST | `/v1/nodes` | Register a new worker node |
| GET | `/v1/namespaces` | List namespaces |

## Troubleshooting

### "Address already in use"

Another process is using the port. Either:
- Stop the other process
- Use a different port: `--port 9000`
- Find what's using it: `lsof -i :8181`

### Control plane not accessible from other machines

Check your `--bind-addr`:
- `127.0.0.1` = localhost only
- `0.0.0.0` = all interfaces (needed for remote access)

Also check firewalls and security groups.

### Worker can't connect to control plane

Verify:
1. Control plane is running
2. URL is correct (include `http://` or `https://`)
3. Network connectivity: `curl http://control-plane:8181/health`

## See Also

- [deploy](./deploy.md) - Deploy pipelines to the control plane
- [get](./get.md) - List resources managed by the control plane
- [status](./status.md) - Check cluster health
- [context](./context.md) - Manage connections to different clusters
