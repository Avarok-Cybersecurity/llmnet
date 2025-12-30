# Basic Chatbot

A minimal pipeline with a router and single handler.

## Composition

```json
{
  "models": {
    "llm": {
      "type": "external",
      "interface": "openai-api",
      "url": "http://localhost:11434/v1"
    }
  },
  "architecture": [
    {
      "name": "router",
      "layer": 0,
      "model": "llm",
      "adapter": "openai-api",
      "output-to": [1]
    },
    {
      "name": "assistant",
      "layer": 1,
      "model": "llm",
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

## Running

```bash
ollama serve
llmnet serve basic-chatbot.json
curl http://localhost:8080/v1/chat/completions \
  -d '{"messages": [{"role": "user", "content": "Hello!"}]}'
```
