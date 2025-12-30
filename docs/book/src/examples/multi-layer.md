# Multi-Layer Pipeline

Chain responses through multiple processing layers.

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
      "name": "draft-writer",
      "layer": 1,
      "model": "model",
      "adapter": "openai-api",
      "use-case": "Write initial draft response",
      "output-to": [2]
    },
    {
      "name": "editor",
      "layer": 2,
      "model": "model",
      "adapter": "openai-api",
      "use-case": "Edit and improve the draft",
      "context": "Improve clarity and fix any errors in the text.",
      "output-to": [3]
    },
    {
      "name": "fact-checker",
      "layer": 3,
      "model": "model",
      "adapter": "openai-api",
      "use-case": "Verify facts and add citations",
      "output-to": ["output"]
    },
    {
      "name": "output",
      "adapter": "output"
    }
  ]
}
```

## Flow

```
Router → Draft Writer → Editor → Fact Checker → Output
```
