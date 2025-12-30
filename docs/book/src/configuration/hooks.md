# Hooks

Hooks allow you to execute custom logic before and after LLM calls. They're defined at the node level and can observe, transform, or validate data.

## Hook Modes

| Mode | Behavior |
|------|----------|
| `observe` | Fire-and-forget. Runs asynchronously, doesn't affect pipeline data. |
| `transform` | Waits for result. Can modify input (pre) or output (post). |

## Hook Configuration

```json
{
  "architecture": [
    {
      "name": "processor",
      "layer": 1,
      "model": "my-model",
      "adapter": "openai-api",
      "hooks": {
        "pre": [
          {
            "function": "log-input",
            "mode": "observe",
            "on_failure": "continue"
          },
          {
            "function": "validate-input",
            "mode": "transform",
            "on_failure": "abort",
            "if": "$WORD_COUNT > 10"
          }
        ],
        "post": [
          {
            "function": "validate-output",
            "mode": "transform",
            "on_failure": "abort"
          }
        ]
      },
      "output-to": ["output"]
    }
  ]
}
```

## Hook Properties

| Property | Type | Default | Description |
|----------|------|---------|-------------|
| `function` | string | required | Name of the function to execute |
| `mode` | string | `observe` | `observe` or `transform` |
| `on_failure` | string | `continue` | `continue` or `abort` |
| `if` | string | optional | Condition expression |

## Failure Actions

| Action | Behavior |
|--------|----------|
| `continue` | Log error and proceed with original data |
| `abort` | Stop pipeline execution and return error |

## Pre vs Post Hooks

### Pre-hooks
Execute **before** the LLM call:
- Access: `$INPUT`, `$NODE`, `$REQUEST_ID`, `$TIMESTAMP`
- Transform mode: Can modify the input sent to the LLM

### Post-hooks
Execute **after** the LLM call:
- Access: All pre-hook variables plus `$OUTPUT`
- Transform mode: Can modify the output before passing to next node

## Conditional Hooks

Use the `if` property to conditionally execute hooks:

```json
{
  "function": "complex-validation",
  "mode": "transform",
  "if": "$WORD_COUNT >= 50"
}
```

See [Conditional Routing](./conditions.md) for available variables and operators.

## Example: Logging and Validation

```json
{
  "functions": {
    "log-request": {
      "type": "rest",
      "method": "POST",
      "url": "https://logging.example.com/api/log",
      "headers": {
        "Authorization": "Bearer $secrets.logging.API_KEY"
      },
      "body": {
        "node": "$NODE",
        "input": "$INPUT",
        "timestamp": "$TIMESTAMP"
      }
    },
    "validate-json": {
      "type": "shell",
      "command": "python",
      "args": ["validate.py", "--input", "$OUTPUT"],
      "timeout": 10
    }
  },
  "architecture": [
    {
      "name": "json-generator",
      "layer": 1,
      "model": "gpt-4",
      "adapter": "openai-api",
      "hooks": {
        "pre": [
          {"function": "log-request", "mode": "observe"}
        ],
        "post": [
          {"function": "validate-json", "mode": "transform", "on_failure": "abort"}
        ]
      },
      "output-to": ["output"]
    }
  ]
}
```

## Hook Chaining

Multiple hooks in a list execute sequentially. For transform mode, each hook receives the output of the previous:

```json
{
  "post": [
    {"function": "step1", "mode": "transform"},
    {"function": "step2", "mode": "transform"},
    {"function": "step3", "mode": "transform"}
  ]
}
```

Flow: `LLM output → step1 → step2 → step3 → final output`
