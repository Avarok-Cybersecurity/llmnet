# llmnet run

Run an LLM pipeline locally on your machine. This is the simplest way to test a pipeline configuration without setting up a cluster.

## Synopsis

```
llmnet run [OPTIONS] <FILE>
```

## Arguments

| Argument | Type | Required | Default | Description |
|----------|------|----------|---------|-------------|
| `<FILE>` | path | yes | - | Path to the composition file (JSON, JSONC, or YAML) |
| `--dry-run` | flag | no | false | Validate and show pipeline structure without running |
| `--bind-addr` | string | no | `0.0.0.0` | IP address to bind the server to |
| `-p, --port` | number | no | `8080` | Port to listen on |
| `--env-file` | path | no | none | Path to a `.env` file for loading API keys |
| `--timeout` | seconds | no | `30` | Request timeout in seconds |
| `--max-concurrent` | number | no | `100` | Maximum concurrent requests per node |

## What It Does

The `run` command starts a local LLM pipeline server on your machine. It:

1. **Reads your configuration** from the specified file
2. **Starts a web server** that accepts OpenAI-compatible requests
3. **Routes requests** through your pipeline (router → handlers → output)
4. **Streams responses** back to the client

This is "legacy mode"—it runs everything locally without a control plane. It's perfect for:
- Development and testing
- Trying out new configurations
- Demos and experiments
- Running on a single machine

Think of it like:
- Running `npm start` for a Node.js app
- Starting a local development server
- Testing locally before deploying to production

## Examples

### Run a Basic Pipeline

```bash
llmnet run examples/basic-chatbot.json
```

**What happens:**
1. Parses `examples/basic-chatbot.json`
2. Starts a server on `http://0.0.0.0:8080`
3. Waits for incoming chat requests
4. Press Ctrl+C to stop

Output:
```
INFO Starting LLMNet pipeline server
INFO Binding to 0.0.0.0:8080
INFO Ready to accept requests
```

### Make a Request to the Running Pipeline

While the server is running (from another terminal):

```bash
curl http://localhost:8080/v1/chat/completions \
  -H "Content-Type: application/json" \
  -d '{"model": "llmnet", "messages": [{"role": "user", "content": "Hello!"}]}'
```

### Dry Run (Preview Without Running)

```bash
llmnet run --dry-run examples/openrouter-pipeline.json
```

**What happens:** Shows the pipeline structure without starting the server:

```
llmnet v0.1.0 - Dry Run Mode

Composition: examples/openrouter-pipeline.json

Models (2):
  • router: external @ https://openrouter.ai/api
  • handler: external @ https://openrouter.ai/api

Architecture:
  Layer 0: router (adapter: openai-api)
           └── outputs to: layer 1
  Layer 1: code-handler, general-handler (use-cases defined)
           └── outputs to: output
  Output:  output

Port: 8080
```

Use dry run to:
- Check your configuration before running
- Understand the pipeline structure
- Verify model and node definitions

### Run on a Different Port

```bash
llmnet run --port 9000 examples/basic-chatbot.json
```

**What happens:** Starts the server on port 9000 instead of 8080. Useful when:
- Port 8080 is already in use
- Running multiple pipelines locally
- Testing with specific port requirements

### Bind to Localhost Only

```bash
llmnet run --bind-addr 127.0.0.1 config.json
```

**What happens:** The server only accepts connections from your local machine. Use this when:
- You don't want the server accessible from the network
- Running in a shared environment
- Security is a concern

### Load API Keys from Environment File

```bash
llmnet run --env-file .env.local config.json
```

**What happens:** Before starting, loads environment variables from `.env.local`. Your config can reference them:

```json
{
  "models": {
    "openrouter": {
      "url": "https://openrouter.ai/api",
      "api-key": "$OPENROUTER_API_KEY"
    }
  }
}
```

`.env.local`:
```
OPENROUTER_API_KEY=sk-or-...
```

### Adjust Timeout and Concurrency

```bash
llmnet run --timeout 60 --max-concurrent 50 config.json
```

**What happens:**
- `--timeout 60`: Wait up to 60 seconds for model responses (default 30)
- `--max-concurrent 50`: Limit to 50 simultaneous requests per node

Use longer timeouts for:
- Large models that take time to respond
- Complex multi-step pipelines
- Slower network connections

## Pipeline Configuration Format

### Minimal Example

```json
{
  "models": {
    "my-llm": {
      "type": "external",
      "interface": "openai-api",
      "url": "http://localhost:11434"
    }
  },
  "architecture": [
    {
      "name": "router",
      "layer": 0,
      "model": "my-llm",
      "adapter": "openai-api",
      "output-to": ["output"]
    },
    {
      "name": "output",
      "adapter": "output"
    }
  ]
}
```

### With Multiple Handlers

```json
{
  "models": {
    "router": { "url": "http://localhost:11434" },
    "code-model": { "url": "http://localhost:11435" },
    "general-model": { "url": "http://localhost:11436" }
  },
  "architecture": [
    {
      "name": "router",
      "layer": 0,
      "model": "router",
      "output-to": [1]
    },
    {
      "name": "code-handler",
      "layer": 1,
      "model": "code-model",
      "use-case": "Code generation and debugging",
      "output-to": ["output"]
    },
    {
      "name": "general-handler",
      "layer": 1,
      "model": "general-model",
      "use-case": "General questions and conversation",
      "output-to": ["output"]
    },
    {
      "name": "output",
      "adapter": "output"
    }
  ]
}
```

## Common Patterns

### Development Workflow

```bash
# 1. Validate configuration
llmnet validate new-config.json

# 2. Preview the pipeline
llmnet run --dry-run new-config.json

# 3. Run locally for testing
llmnet run new-config.json

# 4. Test with curl
curl http://localhost:8080/v1/chat/completions \
  -d '{"model": "llmnet", "messages": [{"role": "user", "content": "Test"}]}'

# 5. When satisfied, deploy to cluster
llmnet deploy new-config.json
```

### Running with Ollama

```bash
# Terminal 1: Start Ollama
ollama serve

# Terminal 2: Run LLMNet with Ollama backend
llmnet run ollama-config.json
```

Where `ollama-config.json`:
```json
{
  "models": {
    "llama": {
      "type": "external",
      "interface": "openai-api",
      "url": "http://localhost:11434"
    }
  },
  "architecture": [
    {"name": "chat", "layer": 0, "model": "llama", "output-to": ["output"]},
    {"name": "output", "adapter": "output"}
  ]
}
```

### Background Running

```bash
# Run in background
llmnet run config.json > logs.txt 2>&1 &

# Check if it's running
curl http://localhost:8080/health

# Stop it
pkill llmnet
# or find and kill the process
ps aux | grep llmnet
kill <pid>
```

### Quick Testing with Debug Output

```bash
# Increase verbosity
llmnet -v run config.json    # Some debug info
llmnet -vv run config.json   # More debug info
llmnet -vvv run config.json  # Maximum verbosity
```

## Error Handling

### File Not Found

```bash
$ llmnet run nonexistent.json
Error: Failed to read file: nonexistent.json
```

**Fix:** Check the path and file exists.

### Invalid Configuration

```bash
$ llmnet run broken.json
Error: Failed to parse configuration: missing field `architecture`
```

**Fix:** Run `llmnet validate broken.json` to see detailed errors.

### Port Already in Use

```bash
$ llmnet run config.json
Error: Failed to bind to 0.0.0.0:8080: Address already in use
```

**Fix:** Either:
- Stop the other process using port 8080: `lsof -i :8080`
- Use a different port: `llmnet run --port 9000 config.json`

### Model Unreachable

```bash
$ llmnet run config.json
# Server starts, but requests fail:
Error: Connection refused to http://localhost:11434
```

**Fix:** Make sure your model backend (Ollama, vLLM, etc.) is running.

## Differences from `llmnet deploy`

| Aspect | `llmnet run` | `llmnet deploy` |
|--------|--------------|-----------------|
| Where it runs | Your local machine | Remote cluster nodes |
| Replicas | Single instance | Multiple (configurable) |
| Managed by | You (Ctrl+C to stop) | Control plane |
| Use case | Development/testing | Production |
| Requires control plane | No | Yes |

## See Also

- [deploy](./deploy.md) - Deploy to a cluster (production use)
- [validate](./validate.md) - Validate configuration without running
- [serve](./serve.md) - Start as part of a cluster
