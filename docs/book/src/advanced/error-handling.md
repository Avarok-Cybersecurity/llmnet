# Error Handling

How llmnet handles errors throughout the pipeline.

## Hook Errors

### Observe Mode
- Errors are logged
- Pipeline continues unaffected
- Good for non-critical operations

### Transform Mode with Continue
- Error is logged
- Original data is preserved
- Pipeline continues with unchanged data

### Transform Mode with Abort
- Error returned to caller
- Pipeline stops immediately
- Subsequent hooks don't execute

## LLM Errors

- Network failures are retried (configurable)
- Timeout errors abort the request
- Invalid responses trigger error

## Validation Errors

Use `llmnet validate` to catch:
- Missing required fields
- Invalid model references
- Undefined functions in hooks
- Missing output node

## Best Practices

1. **Use abort for critical validation**: If bad data would cause problems downstream
2. **Use continue for logging**: Never let logging failures stop your pipeline
3. **Set appropriate timeouts**: Prevent hung connections
4. **Validate before deploy**: Always run `llmnet validate`
