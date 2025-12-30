# Dual Expert Pipeline

Route requests to specialized domain experts.

## Composition

```json
{
  "models": {
    "router": {
      "type": "external",
      "interface": "openai-api",
      "url": "http://localhost:11434/v1"
    },
    "expert": {
      "type": "external",
      "interface": "openai-api",
      "url": "http://localhost:11434/v1"
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
      "name": "sales-expert",
      "layer": 1,
      "model": "expert",
      "adapter": "openai-api",
      "use-case": "Sales inquiries, pricing, product information",
      "context": "You are a sales expert. Be helpful and informative about products.",
      "output-to": ["output"]
    },
    {
      "name": "support-expert",
      "layer": 1,
      "model": "expert",
      "adapter": "openai-api",
      "use-case": "Technical support, troubleshooting, bug reports",
      "context": "You are a technical support expert. Help solve problems.",
      "output-to": ["output"]
    },
    {
      "name": "output",
      "adapter": "output"
    }
  ]
}
```

## How It Works

1. User sends a message
2. Router analyzes the message and selects the best expert
3. Selected expert generates the response
4. Response is returned to user

## Testing

```bash
# Sales query
curl http://localhost:8080/v1/chat/completions \
  -d '{"messages": [{"role": "user", "content": "What are your pricing options?"}]}'

# Support query
curl http://localhost:8080/v1/chat/completions \
  -d '{"messages": [{"role": "user", "content": "My app keeps crashing on startup"}]}'
```
