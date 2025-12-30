# Quick Start

This guide gets you up and running with llmnet in 5 minutes.

## 1. Create a Composition File

Create `my-pipeline.json`:

```json
{
  "models": {
    "router": {
      "type": "external",
      "interface": "openai-api",
      "url": "http://localhost:11434/v1",
      "api-key": "ollama"
    },
    "handler": {
      "type": "external",
      "interface": "openai-api",
      "url": "http://localhost:11434/v1",
      "api-key": "ollama"
    }
  },
  "architecture": [
    {
      "name": "router",
      "layer": 0,
      "model": "router",
      "adapter": "openai-api",
      "output-to": [1]
    },
    {
      "name": "assistant",
      "layer": 1,
      "model": "handler",
      "adapter": "openai-api",
      "use-case": "General assistant for all queries",
      "output-to": ["output"]
    },
    {
      "name": "output",
      "adapter": "output"
    }
  ]
}
```

## 2. Start Ollama

```bash
# Pull a model
ollama pull llama3.2:3b

# Ollama runs on localhost:11434 by default
```

## 3. Validate Your Configuration

```bash
llmnet validate my-pipeline.json
```

## 4. Start the Server

```bash
llmnet serve my-pipeline.json
```

## 5. Send a Request

```bash
curl -X POST http://localhost:8080/v1/chat/completions \
  -H "Content-Type: application/json" \
  -d '{
    "messages": [{"role": "user", "content": "Hello!"}]
  }'
```

## Next Steps

- Learn about [Models Configuration](../configuration/models.md)
- Add multiple handlers with [Architecture](../configuration/architecture.md)
- Set up pre/post processing with [Hooks](../configuration/hooks.md)
