# Hook Execution

Deep dive into how hooks are executed in the pipeline.

## Execution Order

1. **Request arrives** at a node
2. **Pre-hooks execute** in order
   - Each hook can modify input (transform mode)
   - Or observe without modifying (observe mode)
3. **LLM call** with (possibly modified) input
4. **Post-hooks execute** in order
   - Each hook can modify output
   - Or observe without modifying
5. **Result passed** to next node

## Observe Mode Details

```
┌─────────────────────────────────────┐
│           Observe Hook              │
│  ┌─────────────────────────────┐   │
│  │ 1. Spawn async task         │   │
│  │ 2. Execute function         │   │
│  │ 3. Log result (success/fail)│   │
│  └─────────────────────────────┘   │
│  Pipeline continues immediately    │
└─────────────────────────────────────┘
```

- Non-blocking: pipeline doesn't wait
- Fire-and-forget: result is logged but not used
- Failures are logged but don't affect pipeline
- Perfect for: logging, metrics, notifications

## Transform Mode Details

```
┌─────────────────────────────────────┐
│          Transform Hook             │
│  ┌─────────────────────────────┐   │
│  │ 1. Execute function (await) │   │
│  │ 2. Check success/failure    │   │
│  │ 3. Apply output as new data │   │
│  └─────────────────────────────┘   │
│  Pipeline waits for completion     │
└─────────────────────────────────────┘
```

- Blocking: pipeline waits for result
- Result replaces current data
- Failures can abort or continue
- Perfect for: validation, transformation, enrichment

## Error Handling

### Continue on Failure

```json
{"function": "validate", "mode": "transform", "on_failure": "continue"}
```

- Error is logged
- Original data is preserved
- Pipeline continues

### Abort on Failure

```json
{"function": "validate", "mode": "transform", "on_failure": "abort"}
```

- Error returned to caller
- Pipeline stops
- No further hooks execute
