# Composition Files

A composition file defines your complete LLM pipeline in JSON format.

## Structure

```json
{
  "secrets": { },      // Optional: credential sources
  "functions": { },    // Optional: hook functions
  "models": { },       // Required: LLM configurations
  "architecture": [ ]  // Required: pipeline nodes
}
```

## Minimal Example

```json
{
  "models": {
    "default": {
      "type": "external",
      "interface": "openai-api",
      "url": "http://localhost:11434/v1"
    }
  },
  "architecture": [
    {
      "name": "router",
      "layer": 0,
      "model": "default",
      "adapter": "openai-api",
      "output-to": [1]
    },
    {
      "name": "handler",
      "layer": 1,
      "model": "default",
      "adapter": "openai-api",
      "use-case": "General assistant",
      "output-to": ["output"]
    },
    {
      "name": "output",
      "adapter": "output"
    }
  ]
}
```

## Validation

Always validate your composition before running:

```bash
llmnet validate my-pipeline.json
```

This checks:
- JSON syntax
- Required fields
- Model references
- Function references in hooks
- Output node existence
- Layer connectivity
