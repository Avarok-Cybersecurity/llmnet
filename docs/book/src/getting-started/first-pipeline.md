# Your First Pipeline

This guide walks through creating a complete router → handler → output pipeline.

## Understanding the Structure

A llmnet pipeline consists of:
- **Models**: LLM configurations (local or remote)
- **Architecture**: Nodes organized in layers
- **Functions**: Custom operations for hooks (optional)
- **Secrets**: Credential sources (optional)

## Step 1: Define Models

```json
{
  "models": {
    "small-router": {
      "type": "external",
      "interface": "openai-api",
      "url": "http://localhost:11434/v1"
    },
    "large-handler": {
      "type": "external",
      "interface": "openai-api",
      "url": "http://localhost:11434/v1"
    }
  }
}
```

## Step 2: Create Architecture

```json
{
  "architecture": [
    {
      "name": "router",
      "layer": 0,
      "model": "small-router",
      "adapter": "openai-api",
      "output-to": [1]
    },
    {
      "name": "expert",
      "layer": 1,
      "model": "large-handler",
      "adapter": "openai-api",
      "use-case": "Detailed expert responses",
      "output-to": ["output"]
    },
    {
      "name": "output",
      "adapter": "output"
    }
  ]
}
```

## Step 3: Run It

```bash
llmnet validate my-pipeline.json
llmnet serve my-pipeline.json
```

## Next Steps

- Add more handlers: [Architecture](../configuration/architecture.md)
- Add hooks: [Hooks](../configuration/hooks.md)
- Add secrets: [Secrets](../configuration/secrets.md)
