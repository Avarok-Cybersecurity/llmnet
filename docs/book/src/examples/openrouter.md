# OpenRouter Integration

Use multiple models via OpenRouter.

## Composition

```json
{
  "secrets": {
    "openrouter": {
      "source": "env",
      "variable": "OPENROUTER_API_KEY"
    }
  },
  "models": {
    "fast-router": {
      "type": "external",
      "interface": "openai-api",
      "url": "https://openrouter.ai/api/v1",
      "api-key": "$secrets.openrouter.OPENROUTER_API_KEY"
    },
    "powerful-handler": {
      "type": "external",
      "interface": "openai-api",
      "url": "https://openrouter.ai/api/v1",
      "api-key": "$secrets.openrouter.OPENROUTER_API_KEY"
    }
  },
  "architecture": [
    {
      "name": "router",
      "layer": 0,
      "model": "fast-router",
      "adapter": "openai-api",
      "extra-options": {
        "model_override": "meta-llama/llama-3.2-3b-instruct"
      },
      "output-to": [1]
    },
    {
      "name": "expert",
      "layer": 1,
      "model": "powerful-handler",
      "adapter": "openai-api",
      "extra-options": {
        "model_override": "anthropic/claude-3.5-sonnet"
      },
      "use-case": "Expert responses using a powerful model",
      "output-to": ["output"]
    },
    {
      "name": "output",
      "adapter": "output"
    }
  ]
}
```

## Setup

```bash
export OPENROUTER_API_KEY=sk-or-...
llmnet serve openrouter-pipeline.json
```
