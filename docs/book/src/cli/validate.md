# validate

Validate a composition file without running it.

## Usage

```bash
llmnet validate <COMPOSITION>
```

## Example

```bash
llmnet validate my-pipeline.json
```

## What It Checks

- JSON syntax
- Required fields
- Model references
- Architecture connectivity
- Output node existence
- Function references in hooks
