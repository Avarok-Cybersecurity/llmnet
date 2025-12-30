# Variable Substitution

Variables allow dynamic values in functions and hooks.

## Available Variables

| Variable | Scope | Description |
|----------|-------|-------------|
| `$INPUT` | Pre/Post | Current input content |
| `$OUTPUT` | Post only | LLM output |
| `$NODE` | Pre/Post | Current node name |
| `$PREV_NODE` | Pre/Post | Previous node name |
| `$TIMESTAMP` | Pre/Post | ISO 8601 timestamp |
| `$REQUEST_ID` | Pre/Post | Unique request ID |

## Secret Variables

Access secrets with: `$secrets.{name}.{variable}`

```json
"Authorization": "Bearer $secrets.api.TOKEN"
```

## Substitution in Bodies

Variables work in JSON bodies:

```json
{
  "body": {
    "node": "$NODE",
    "input": "$INPUT",
    "timestamp": "$TIMESTAMP"
  }
}
```

## Nested Substitution

Variables are substituted recursively in objects and arrays:

```json
{
  "body": {
    "data": {
      "nested": {
        "value": "$INPUT"
      }
    },
    "list": ["$NODE", "$TIMESTAMP"]
  }
}
```
