# llmnet deploy

Deploy an LLM pipeline to the current cluster context. This sends your pipeline configuration to the control plane, which then manages running it across available worker nodes.

## Synopsis

```
llmnet deploy [OPTIONS] <FILE>
```

## Arguments

| Argument | Type | Required | Default | Description |
|----------|------|----------|---------|-------------|
| `<FILE>` | path | yes | - | Path to the pipeline manifest (JSON or YAML) |
| `-n, --namespace` | string | no | `default` | Namespace to deploy the pipeline into |
| `--dry-run` | flag | no | false | Validate and show what would be deployed without actually deploying |

## What It Does

The `deploy` command takes a pipeline configuration file and sends it to your LLMNet control plane. Think of it like uploading a configuration to a central server that then manages running your LLM pipeline.

When you deploy:

1. **The config is validated** - Syntax errors, missing models, or invalid references are caught
2. **A Pipeline resource is created** - The control plane tracks this pipeline by name and namespace
3. **Replicas are scheduled** - The control plane assigns pipeline instances to available worker nodes
4. **The pipeline starts serving requests** - Workers begin processing incoming LLM queries

### Namespaces

Namespaces are a way to organize pipelines. They're like folders or projects:

- `default` - Where pipelines go if you don't specify a namespace
- `production` - You might put live customer-facing pipelines here
- `staging` - For testing new configurations
- `team-research` - For a specific team's experiments

Pipelines in different namespaces can have the same name without conflict.

## Examples

### Deploy a Basic Pipeline

```bash
llmnet deploy examples/basic-chatbot.json
```

**What happens:**
1. Reads `examples/basic-chatbot.json`
2. Creates a pipeline named after the file (`basic-chatbot`) in the `default` namespace
3. Schedules it to run on available worker nodes

Output:
```
pipeline.llmnet/basic-chatbot deployed to namespace default
```

### Deploy with a Specific Namespace

```bash
llmnet deploy pipeline.json --namespace production
```

**What happens:** Deploys the pipeline to the `production` namespace instead of `default`. This keeps production pipelines separate from development/testing.

### Preview Without Deploying (Dry Run)

```bash
llmnet deploy pipeline.json --dry-run
```

**What happens:** Parses and validates the configuration, shows you what would be deployed, but doesn't actually deploy anything. Useful for:
- Checking for configuration errors before deploying
- Reviewing the pipeline structure
- CI/CD validation steps

Output:
```
Dry-run mode: would deploy pipeline 'my-pipeline'
Name:       my-pipeline
Namespace:  default
Replicas:   1
Port:       8080

Composition:
  Models: 2
  Nodes:  4
```

### Deploy a YAML Pipeline Manifest

```bash
llmnet deploy customer-service.yaml
```

**What happens:** LLMNet supports YAML format for pipeline manifests. YAML is often easier to read and write than JSON:

```yaml
apiVersion: llmnet/v1
kind: Pipeline
metadata:
  name: customer-service
  namespace: production
  labels:
    team: support
    version: v2
spec:
  replicas: 3
  composition:
    models:
      router:
        type: external
        interface: openai-api
        url: http://localhost:11434
    architecture:
      - name: router
        layer: 0
        model: router
        output-to: [1]
      - name: output
        adapter: output
```

### Deploy an Existing Composition File

```bash
llmnet deploy examples/openrouter-pipeline.json
```

**What happens:** If you have an existing LLMNet composition file (the format used with `llmnet run`), the deploy command automatically converts it to a Pipeline resource. The pipeline name is taken from the filename.

## Pipeline Manifest Format

### Full YAML Example

```yaml
apiVersion: llmnet/v1
kind: Pipeline
metadata:
  name: my-chatbot
  namespace: default
  labels:
    app: chatbot
    environment: dev
spec:
  # Number of instances to run
  replicas: 2

  # Port for the OpenAI-compatible API
  port: 8080

  # Health check configuration
  health:
    livenessPath: /health
    readinessPath: /health
    periodSeconds: 10
    failureThreshold: 3

  # The actual LLM pipeline configuration
  composition:
    models:
      llama:
        type: external
        interface: openai-api
        url: http://ollama:11434
    architecture:
      - name: router
        layer: 0
        model: llama
        adapter: openai-api
        output-to: [1]
      - name: handler
        layer: 1
        use-case: "General questions"
      - name: output
        adapter: output
```

### Using Existing Composition Files

You can also deploy standard composition files directly:

```json
{
  "models": {
    "my-model": {
      "type": "external",
      "interface": "openai-api",
      "url": "http://localhost:11434"
    }
  },
  "architecture": [
    {"name": "router", "layer": 0, "model": "my-model", "adapter": "openai-api", "output-to": [1]},
    {"name": "handler", "layer": 1, "use-case": "Handle queries"},
    {"name": "output", "adapter": "output"}
  ]
}
```

When you deploy a composition file, llmnet automatically:
- Uses the filename as the pipeline name
- Sets replicas to 1
- Uses default health check settings

## Common Patterns

### Development Workflow

```bash
# Validate first
llmnet deploy config.json --dry-run

# If validation passes, deploy to dev namespace
llmnet deploy config.json --namespace dev

# Test it out
curl http://localhost:8080/v1/chat/completions \
  -d '{"model": "llmnet", "messages": [{"role": "user", "content": "Hello"}]}'

# When ready, deploy to production
llmnet deploy config.json --namespace production
```

### CI/CD Pipeline

```yaml
# .github/workflows/deploy.yml
steps:
  - name: Validate configuration
    run: llmnet deploy pipeline.yaml --dry-run

  - name: Deploy to staging
    run: llmnet deploy pipeline.yaml --namespace staging

  - name: Run integration tests
    run: ./run-tests.sh

  - name: Deploy to production
    run: llmnet deploy pipeline.yaml --namespace production
```

### Multiple Pipelines for Different Use Cases

```bash
# Deploy a pipeline for coding assistance
llmnet deploy coding-assistant.yaml --namespace tools

# Deploy a pipeline for customer support
llmnet deploy support-bot.yaml --namespace support

# Deploy a pipeline for content generation
llmnet deploy writer.yaml --namespace content
```

## Error Handling

### Configuration Errors

```bash
$ llmnet deploy broken.json
Error: Failed to parse configuration: missing field `architecture`
```

**Fix:** Check your JSON/YAML syntax and ensure all required fields are present.

### Model Reference Errors

```bash
$ llmnet deploy config.json
Error: Model 'nonexistent' referenced by node 'router' is not defined
```

**Fix:** Make sure every `model` field in your architecture references a model defined in the `models` section.

### Connection Errors

```bash
$ llmnet deploy config.json
Error: Connection failed to http://0.0.0.0:8181: Connection refused
```

**Fix:**
1. Make sure the control plane is running: `llmnet serve --control-plane`
2. Check your context: `llmnet context current`
3. Verify network connectivity

### Pipeline Already Exists

```bash
$ llmnet deploy config.json
Error: Pipeline 'my-pipeline' already exists in namespace 'default'
```

**Fix:** Either:
- Delete the existing pipeline: `llmnet delete pipeline my-pipeline`
- Use a different name
- Deploy to a different namespace: `--namespace other`

## How It Differs from `llmnet run`

| Aspect | `llmnet deploy` | `llmnet run` |
|--------|-----------------|--------------|
| **Where it runs** | Remote control plane | Local machine |
| **Scaling** | Multiple replicas across nodes | Single instance |
| **Management** | Tracked, can list/delete/scale | Not tracked, Ctrl+C to stop |
| **Use case** | Production/team environments | Local development/testing |

## See Also

- [serve](./serve.md) - Start the control plane that receives deployments
- [get](./get.md) - List deployed pipelines
- [delete](./delete.md) - Remove deployed pipelines
- [scale](./scale.md) - Change the number of replicas
- [run](./run.md) - Run a pipeline locally without deploying
