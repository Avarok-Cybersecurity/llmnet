# Models

Models define how llmnet connects to LLMs.

## External Models

Connect to OpenAI-compatible APIs:

```json
{
  "models": {
    "gpt4": {
      "type": "external",
      "interface": "openai-api",
      "url": "https://api.openai.com/v1",
      "api-key": "$secrets.openai.API_KEY"
    },
    "ollama-local": {
      "type": "external",
      "interface": "openai-api",
      "url": "http://localhost:11434/v1",
      "api-key": "ollama"
    }
  }
}
```

## Model Override

Override the model name per-node:

```json
{
  "name": "router",
  "model": "ollama-local",
  "extra-options": {
    "model_override": "llama3.2:3b"
  }
}
```

## Spawnable Models

Define models to be spawned locally:

```json
{
  "models": {
    "local-llama": {
      "type": "ollama",
      "model": "llama3.2:3b",
      "options": {
        "num_ctx": 4096,
        "num_gpu": 1
      }
    }
  }
}
```

Supported spawnable types:
- `ollama`: Ollama models
- `vllm`: vLLM server
- `llamacpp`: llama.cpp server
