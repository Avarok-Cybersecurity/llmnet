# LLMNet CLI Commands

LLMNet provides a kubectl-like command-line interface for managing LLM pipelines across local and distributed clusters.

## Quick Reference

| Command | Description |
|---------|-------------|
| [`llmnet run`](./run.md) | Run a pipeline locally (development) |
| [`llmnet serve`](./serve.md) | Start a control plane or worker node |
| [`llmnet deploy`](./deploy.md) | Deploy a pipeline to the cluster |
| [`llmnet get`](./get.md) | List resources (pipelines, nodes, namespaces) |
| [`llmnet delete`](./delete.md) | Remove resources from the cluster |
| [`llmnet scale`](./scale.md) | Change the number of pipeline replicas |
| [`llmnet context`](./context.md) | Manage cluster connections |
| [`llmnet status`](./status.md) | View cluster health overview |
| [`llmnet validate`](./validate.md) | Check configuration files for errors |
| [`llmnet logs`](./logs.md) | View pipeline logs (planned) |

## Getting Started

### Local Development

For testing pipelines on your local machine:

```bash
# Validate your configuration
llmnet validate my-pipeline.json

# Run locally
llmnet run my-pipeline.json

# Make requests
curl http://localhost:8080/v1/chat/completions \
  -d '{"model": "llmnet", "messages": [{"role": "user", "content": "Hello!"}]}'
```

### Cluster Deployment

For running pipelines across multiple machines:

```bash
# Start the control plane (on manager machine)
llmnet serve --control-plane

# Deploy a pipeline
llmnet deploy my-pipeline.json

# Check status
llmnet status
llmnet get pipelines

# Scale up
llmnet scale my-pipeline --replicas 5
```

### Multi-Cluster Management

For managing multiple environments:

```bash
# Add cluster contexts
llmnet context add production --url https://prod.example.com:8181
llmnet context add staging --url http://staging.internal:8181

# Switch between clusters
llmnet context use staging
llmnet deploy new-feature.yaml

llmnet context use production
llmnet deploy stable-config.yaml
```

## Command Categories

### Pipeline Operations

| Command | Purpose |
|---------|---------|
| `run` | Run locally for development |
| `deploy` | Deploy to cluster |
| `delete` | Remove from cluster |
| `scale` | Adjust replica count |

### Cluster Management

| Command | Purpose |
|---------|---------|
| `serve` | Start control plane or worker |
| `get` | List resources |
| `status` | Cluster health overview |
| `context` | Manage cluster connections |

### Configuration

| Command | Purpose |
|---------|---------|
| `validate` | Check configuration syntax |

### Monitoring

| Command | Purpose |
|---------|---------|
| `logs` | View pipeline logs (planned) |
| `status` | Quick health check |

## Global Options

These options work with all commands:

```
-v, --verbose     Increase logging verbosity (use -vv or -vvv for more)
    --config      Path to config file (default: ~/.llmnet/config)
-h, --help        Show help for any command
-V, --version     Show version information
```

## Typical Workflows

### Development to Production

```bash
# 1. Create and validate configuration
vim new-pipeline.json
llmnet validate new-pipeline.json

# 2. Test locally
llmnet run new-pipeline.json

# 3. Deploy to staging
llmnet context use staging
llmnet deploy new-pipeline.json -n testing

# 4. Verify
llmnet get pipelines -n testing

# 5. Deploy to production
llmnet context use production
llmnet deploy new-pipeline.json -n production
llmnet scale new-pipeline --replicas 5 -n production
```

### Setting Up a New Cluster

```bash
# On manager node
llmnet serve --control-plane --port 8181

# On worker nodes
llmnet serve --node-name worker-1 --control-plane-url http://manager:8181
llmnet serve --node-name worker-2 --control-plane-url http://manager:8181

# From your workstation
llmnet context add my-cluster --url http://manager:8181
llmnet context use my-cluster
llmnet status
```

### Emergency Scaling

```bash
# Quick capacity increase
llmnet scale api-service --replicas 20 -n production

# Monitor rollout
watch llmnet get pipelines -n production

# Verify health
llmnet status
```

## See Also

- [Configuration Reference](../README.md) - Pipeline configuration format
- [Examples](../examples/) - Sample pipeline configurations
- [Architecture](../architecture.md) - How LLMNet works internally
