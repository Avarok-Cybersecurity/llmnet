# Conditional Routing

> **Config file:** [`examples/conditional-routing.json`](../../examples/conditional-routing.json)

Route requests based on input characteristics using system variables and conditions.

## Topology

```
                    ┌─────────────────────┐
                    │  Short Input        │──────────────────────┐
                    │  Handler            │                      │
┌────────────┐     ╱└─────────────────────┘                      ▼
│   Router   │────<                                         ┌────────┐
└────────────┘     ╲┌─────────────────────┐     ┌────────┐  │ Output │
                    │  Long Input         │────▶│Refiner │─▶│        │
                    │  Handler            │     └────────┘  └────────┘
                    └─────────────────────┘

     Layer 0              Layer 1              Layer 2       Output

     Conditions:
     - Short: $WORD_COUNT < 10   → Direct to output
     - Long:  $WORD_COUNT >= 10  → Through refiner
```

**Key Feature:** The `if` field uses system variables to conditionally activate nodes.

## System Variables

llmnet automatically tracks these variables for each request:

| Variable | Description | Example |
|----------|-------------|---------|
| `$INITIAL_INPUT` | Original user prompt | "Hello world" |
| `$CURRENT_INPUT` | Current content (may change) | "Hello world" |
| `$PREV_NODE` | Name of previous node | "router" |
| `$PREV_LAYER` | Layer number of previous node | "0" |
| `$CURRENT_LAYER` | Current layer being evaluated | "1" |
| `$HOP_COUNT` | Number of hops so far | "2" |
| `$TIMESTAMP` | Unix timestamp | "1703612400" |
| `$REQUEST_ID` | Unique request UUID | "a1b2c3..." |
| `$ROUTE_DECISION` | Last routing decision | "technical-handler" |
| `$INPUT_LENGTH` | Character count | "150" |
| `$WORD_COUNT` | Word count | "25" |

## Condition Operators

| Operator | Example | Description |
|----------|---------|-------------|
| Existence | `$PREV_NODE` | True if variable exists and non-empty |
| `==` | `$PREV_NODE == "router"` | String equality |
| `!=` | `$PREV_NODE != "router"` | String inequality |
| `>` | `$WORD_COUNT > 10` | Numeric greater than |
| `<` | `$WORD_COUNT < 10` | Numeric less than |
| `>=` | `$HOP_COUNT >= 2` | Numeric greater or equal |
| `<=` | `$INPUT_LENGTH <= 100` | Numeric less or equal |

## Configuration

```json
{
  "architecture": [
    {
      "name": "router",
      "layer": 0,
      "output-to": [1]
    },
    {
      "name": "short-input-handler",
      "layer": 1,
      "if": "$WORD_COUNT < 10",
      "use-case": "Quick responses for short inputs",
      "output-to": ["output"]
    },
    {
      "name": "long-input-handler",
      "layer": 1,
      "if": "$WORD_COUNT >= 10",
      "use-case": "Detailed responses for longer inputs",
      "output-to": [2]
    },
    {
      "name": "response-refiner",
      "layer": 2,
      "output-to": ["output"]
    },
    { "name": "output", "adapter": "output" }
  ]
}
```

## Real-Life Use Cases

### 1. Complexity-Based Routing

**Scenario:** Use simple model for simple queries, powerful model for complex ones.

```json
{
  "name": "simple-handler",
  "if": "$WORD_COUNT < 20",
  "extra-options": { "model_override": "gemma-7b" }
},
{
  "name": "complex-handler",
  "if": "$WORD_COUNT >= 20",
  "extra-options": { "model_override": "llama-70b" }
}
```

**Cost savings:** Short queries (greetings, simple questions) use cheap, fast models. Complex queries get full power.

### 2. Path-Dependent Refinement

**Scenario:** Different refiners based on which handler processed the request.

```json
{
  "name": "code-refiner",
  "layer": 2,
  "if": "$PREV_NODE == \"code-handler\"",
  "use-case": "Format code blocks, add syntax highlighting"
},
{
  "name": "prose-refiner",
  "layer": 2,
  "if": "$PREV_NODE == \"writing-handler\"",
  "use-case": "Polish prose, check grammar"
}
```

**Why it works:** Code needs different post-processing than prose.

### 3. Loop Prevention

**Scenario:** Prevent infinite loops in complex pipelines.

```json
{
  "name": "recursive-processor",
  "layer": 1,
  "if": "$HOP_COUNT < 5",
  "output-to": [1, 2]  // Can loop back to layer 1
},
{
  "name": "final-processor",
  "layer": 1,
  "if": "$HOP_COUNT >= 5",
  "output-to": ["output"]  // Forces exit after 5 hops
}
```

### 4. Time-Based Routing

**Scenario:** Different handling during peak vs off-peak hours.

```json
{
  "name": "peak-hours-handler",
  "if": "$TIMESTAMP > 1703602800",  // After 9 AM
  "extra-options": { "model_override": "fast-model" }
},
{
  "name": "off-peak-handler",
  "if": "$TIMESTAMP <= 1703602800",
  "extra-options": { "model_override": "quality-model" }
}
```

### 5. Length-Based Summarization

**Scenario:** Only summarize long responses.

```json
{
  "name": "summarizer",
  "layer": 2,
  "if": "$INPUT_LENGTH > 1000",
  "use-case": "Summarize the response to key points"
},
{
  "name": "passthrough",
  "layer": 2,
  "if": "$INPUT_LENGTH <= 1000",
  "output-to": ["output"]
}
```

### 6. First-Time vs Return Processing

**Scenario:** Different processing for initial response vs subsequent refinements.

```json
{
  "name": "initial-processor",
  "layer": 1,
  "if": "$HOP_COUNT == \"0\"",
  "use-case": "Generate initial response"
},
{
  "name": "refinement-processor",
  "layer": 1,
  "if": "$HOP_COUNT > 0",
  "use-case": "Refine previous response"
}
```

### 7. Conditional WebSocket Alerts

**Scenario:** Only send WebSocket notifications for certain conditions.

```json
{
  "name": "ws-alert",
  "if": "$WORD_COUNT > 50",
  "adapter": "ws",
  "url": "ws://alerts:3000"
}
```

Long responses trigger WebSocket alerts; short ones don't.

### 8. A/B Testing

**Scenario:** Route a percentage of traffic to experimental handlers.

```json
{
  "name": "control-handler",
  "if": "$REQUEST_ID < \"8\"",  // ~50% of UUIDs start with 0-7
  "use-case": "Control group: existing model"
},
{
  "name": "experiment-handler",
  "if": "$REQUEST_ID >= \"8\"",  // ~50% start with 8-f
  "use-case": "Experiment: new model"
}
```

## Condition Fallback Behavior

If **no conditions match**, all nodes in the layer are considered valid targets. This prevents pipelines from getting stuck:

```json
// If input has exactly 10 words, neither condition matches
{
  "name": "handler-a",
  "if": "$WORD_COUNT < 10"   // False for 10 words
},
{
  "name": "handler-b",
  "if": "$WORD_COUNT > 10"   // False for 10 words
}
// Result: Both handlers become available, router chooses
```

## Best Practices

### 1. Make Conditions Mutually Exclusive

```json
// Good: No overlap
"if": "$WORD_COUNT < 10"
"if": "$WORD_COUNT >= 10"

// Bad: Overlap at 10
"if": "$WORD_COUNT <= 10"
"if": "$WORD_COUNT >= 10"
```

### 2. Include a Default Handler

```json
{
  "name": "specialized-handler",
  "if": "$WORD_COUNT > 50"
},
{
  "name": "default-handler"  // No condition = always matches
}
```

### 3. Use Descriptive Node Names

```json
// Good
"name": "short-query-handler"
"name": "complex-query-handler"

// Bad
"name": "handler-1"
"name": "handler-2"
```

## Testing Conditions

```bash
# Short input (< 10 words) - should use short-input-handler
curl -X POST http://localhost:8080/v1/chat/completions \
  -d '{"model":"llmnet","messages":[{"role":"user","content":"Hello"}]}'

# Long input (>= 10 words) - should use long-input-handler + refiner
curl -X POST http://localhost:8080/v1/chat/completions \
  -d '{"model":"llmnet","messages":[{"role":"user","content":"Please explain in detail how machine learning algorithms work and provide examples of common use cases"}]}'
```

## Combining with LLM Routing

Conditions work alongside LLM-based routing:

1. **Conditions filter available targets** based on variables
2. **Router chooses among filtered targets** using LLM intelligence

```json
{
  "name": "premium-handler",
  "if": "$INPUT_LENGTH > 500",  // Only available for long inputs
  "use-case": "Complex analysis requiring premium resources"
},
{
  "name": "standard-handler",
  "use-case": "Standard queries"  // Always available
}
```

For a 600-character input:
- Both handlers available (premium passes condition, standard has no condition)
- Router uses LLM to choose based on use-case descriptions

**Next step:** [Nemotron Router](./nemotron-router.md) for advanced LLM-based routing.
