# Conditional Routing

Conditions allow dynamic routing based on request characteristics.

## Available Variables

| Variable | Type | Description |
|----------|------|-------------|
| `$WORD_COUNT` | number | Word count of current input |
| `$INPUT_LENGTH` | number | Character count of input |
| `$HOP_COUNT` | number | Number of nodes visited |
| `$PREV_NODE` | string | Name of previous node |
| `$CURRENT_LAYER` | number | Current processing layer |

## Operators

### Existence Check
```json
"if": "$API_KEY"
```
Passes if variable exists and is non-empty.

### Equality
```json
"if": "$PREV_NODE == \"router\""
```

### Inequality
```json
"if": "$MODE != \"debug\""
```

### Numeric Comparisons
```json
"if": "$WORD_COUNT > 50"
"if": "$WORD_COUNT >= 50"
"if": "$HOP_COUNT < 3"
"if": "$INPUT_LENGTH <= 1000"
```

## Examples

### Route Long Inputs to Different Handler

```json
{
  "architecture": [
    {"name": "router", "layer": 0, "output-to": [1]},
    {
      "name": "short-handler",
      "layer": 1,
      "if": "$WORD_COUNT < 20",
      "use-case": "Quick responses"
    },
    {
      "name": "long-handler",
      "layer": 1,
      "if": "$WORD_COUNT >= 20",
      "use-case": "Detailed responses"
    }
  ]
}
```

### Prevent Infinite Loops

```json
{
  "name": "recursive-handler",
  "layer": 1,
  "if": "$HOP_COUNT < 5",
  "output-to": [1]
}
```

### Chain-Specific Processing

```json
{
  "name": "refiner-for-sales",
  "layer": 2,
  "if": "$PREV_NODE == \"sales\"",
  "use-case": "Refine sales responses"
}
```
