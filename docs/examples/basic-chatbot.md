# Basic Chatbot (1-0-1 Topology)

> **Config file:** [`examples/basic-chatbot.json`](../../examples/basic-chatbot.json)

The simplest llmnet topology—wrap any LLM behind an OpenAI-compatible API endpoint.

## Topology

```
┌─────────┐     ┌─────────┐
│  User   │────▶│   LLM   │────▶ Response
└─────────┘     └─────────┘
```

**Layers:** 1 (input only)
**Nodes:** 1 LLM + output
**Routing:** None (direct passthrough)

## How It Works

1. User sends a request to the llmnet endpoint
2. Request is forwarded directly to the configured LLM
3. Response is returned to the user

This is essentially a proxy that standardizes any LLM behind the OpenAI API format.

## Configuration

```json
{
  "models": {
    "chatbot": {
      "type": "external",
      "interface": "openai-api",
      "url": "http://localhost:11434",
      "api-key": null
    }
  },
  "architecture": [
    {
      "name": "chat",
      "layer": 0,
      "model": "chatbot",
      "adapter": "openai-api",
      "bind-port": "8080",
      "output-to": ["output"]
    },
    {
      "name": "output",
      "adapter": "output"
    }
  ]
}
```

## Real-Life Use Cases

### 1. Local Model Gateway

**Scenario:** You run Ollama locally and want to use it with OpenAI-compatible tools.

```json
{
  "models": {
    "ollama-llama": {
      "type": "external",
      "interface": "openai-api",
      "url": "http://localhost:11434",
      "api-key": null
    }
  }
}
```

**Benefits:**
- Use local models with any OpenAI SDK
- No code changes in existing applications
- Switch between local and cloud models by changing config

### 2. API Key Abstraction

**Scenario:** Your team uses different AI providers, but you want a single endpoint.

```json
{
  "models": {
    "anthropic-claude": {
      "type": "external",
      "interface": "openai-api",
      "url": "https://api.anthropic.com/v1",
      "api-key": "$ANTHROPIC_API_KEY"
    }
  }
}
```

**Benefits:**
- Centralize API key management
- Developers don't need individual API keys
- Easy to switch providers without code changes

### 3. Development/Production Parity

**Scenario:** Use a cheap/fast model locally, expensive model in production.

**Development config:**
```json
{
  "models": {
    "dev-model": {
      "url": "http://localhost:11434"  // Ollama
    }
  }
}
```

**Production config:**
```json
{
  "models": {
    "prod-model": {
      "url": "https://api.openai.com",
      "api-key": "$OPENAI_API_KEY"
    }
  }
}
```

### 4. Rate Limiting & Monitoring Gateway

**Scenario:** Add observability to LLM calls without changing application code.

Run llmnet in front of your LLM endpoint to:
- Log all requests/responses
- Add custom metrics
- Implement rate limiting at the gateway level

### 5. Fine-Tuned Model Deployment

**Scenario:** Deploy your fine-tuned model with a standardized API.

```json
{
  "models": {
    "my-finetuned-model": {
      "type": "external",
      "interface": "openai-api",
      "url": "http://vllm-server:8000",
      "api-key": null
    }
  }
}
```

**Benefits:**
- Serve custom models via standard API
- Easy integration with existing tooling
- Drop-in replacement for commercial APIs

## When to Use This Topology

- You need a simple API wrapper
- You want to standardize access to a single model
- You're building a development environment
- You need a gateway for monitoring/logging

## When to Upgrade

Consider moving to a more complex topology when:
- You need to route between multiple models
- Different queries require different specialists
- You want to add processing layers (refinement, validation)
- You need conditional logic based on input characteristics

**Next step:** [Dual Expert Router](./dual-expert.md) for intent-based routing.
