# Conditional Routing

Route based on input characteristics.

## Composition

```json
{
  "models": {
    "model": {
      "type": "external",
      "interface": "openai-api",
      "url": "http://localhost:11434/v1"
    }
  },
  "architecture": [
    {
      "name": "router",
      "layer": 0,
      "model": "model",
      "adapter": "openai-api",
      "output-to": [1]
    },
    {
      "name": "quick-responder",
      "layer": 1,
      "model": "model",
      "adapter": "openai-api",
      "use-case": "Quick, concise answers",
      "if": "$WORD_COUNT < 10",
      "output-to": ["output"]
    },
    {
      "name": "detailed-responder",
      "layer": 1,
      "model": "model",
      "adapter": "openai-api",
      "use-case": "Detailed, comprehensive answers",
      "if": "$WORD_COUNT >= 10",
      "output-to": ["output"]
    },
    {
      "name": "output",
      "adapter": "output"
    }
  ]
}
```

## Testing

```bash
# Short query → quick-responder
curl -d '{"messages": [{"role": "user", "content": "Hi there"}]}'

# Long query → detailed-responder
curl -d '{"messages": [{"role": "user", "content": "Can you explain in detail how machine learning models are trained and what factors affect their performance?"}]}'
```
