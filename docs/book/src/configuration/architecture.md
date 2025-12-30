# Architecture

The architecture defines nodes in your pipeline and how they connect.

## Node Properties

```json
{
  "name": "sales-agent",
  "layer": 1,
  "model": "handler-model",
  "adapter": "openai-api",
  "use-case": "Handle sales inquiries",
  "context": "You are a helpful sales assistant...",
  "if": "$WORD_COUNT > 10",
  "hooks": {
    "pre": [],
    "post": []
  },
  "output-to": [2]
}
```

| Property | Type | Required | Description |
|----------|------|----------|-------------|
| `name` | string | Yes | Unique node identifier |
| `layer` | number | No | Processing layer (0 = router) |
| `model` | string | No | Reference to a model |
| `adapter` | string | Yes | `openai-api` or `output` |
| `use-case` | string | No | Description for routing |
| `context` | string | No | System prompt |
| `if` | string | No | Condition for routing |
| `hooks` | object | No | Pre/post hooks |
| `output-to` | array | No | Target layers or node names |

## Layers

Organize nodes into layers:
- **Layer 0**: Router (entry point)
- **Layer 1-N**: Handler nodes
- **Output**: Final output node

## Output Targets

Specify targets by layer number or node name:

```json
// By layer number
"output-to": [1, 2]

// By node name
"output-to": ["sales", "support", "output"]
```

## Required Output Node

Every composition must have an output node:

```json
{
  "name": "output",
  "adapter": "output"
}
```
