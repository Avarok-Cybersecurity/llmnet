# llmnet validate

Check a pipeline configuration file for errors without deploying or running it. Use this to catch mistakes before they cause problems.

## Synopsis

```
llmnet validate <FILE>
```

## Arguments

| Argument | Type | Required | Description |
|----------|------|----------|-------------|
| `<FILE>` | path | yes | Path to the composition file (JSON, JSONC, or YAML) |

## What It Does

The `validate` command parses your configuration file and checks for errors:

1. **Syntax errors:** Invalid JSON/YAML, missing commas, unbalanced braces
2. **Missing fields:** Required fields like `architecture` or model URLs
3. **Invalid references:** Nodes referencing models that don't exist
4. **Type errors:** Wrong types for fields (e.g., string where number expected)

It does NOT:
- Connect to any external services
- Verify that model URLs are reachable
- Deploy or run anything
- Require a control plane

Think of it like:
- A linter for your pipeline configs
- Syntax checking before committing code
- A safety check before deployment

## Examples

### Validate a Good Configuration

```bash
llmnet validate examples/basic-chatbot.json
```

**Output:**
```
✓ Configuration valid
  Models: 1
  Nodes: 2
```

**What it means:** The file is syntactically correct and has 1 model defined with 2 architecture nodes.

### Validate a Configuration with Errors

```bash
llmnet validate broken-config.json
```

**Output:**
```
✗ Configuration invalid
Error: missing field `architecture`
```

**What it means:** The file is missing the required `architecture` field.

### Validate YAML Configuration

```bash
llmnet validate pipeline.yaml
```

**Output:**
```
✓ Configuration valid
  Models: 3
  Nodes: 5
```

LLMNet validates both JSON and YAML formats.

### Validate JSONC (JSON with Comments)

```bash
llmnet validate config.jsonc
```

LLMNet supports JSON with comments (JSONC), which is helpful for documenting your configuration:

```json
{
  // This is the router model
  "models": {
    "router": {
      "type": "external",
      "interface": "openai-api",
      "url": "http://localhost:11434"
    }
  },
  "architecture": [
    // Entry point
    {"name": "router", "layer": 0, "model": "router", "output-to": [1]},
    {"name": "output", "adapter": "output"}
  ]
}
```

## Common Errors

### Missing Architecture

```
Error: missing field `architecture`
```

**Problem:** Every composition needs an `architecture` array defining the pipeline structure.

**Fix:** Add an architecture section:
```json
{
  "models": { ... },
  "architecture": [
    {"name": "router", "layer": 0, "model": "my-model", "output-to": ["output"]},
    {"name": "output", "adapter": "output"}
  ]
}
```

### Unknown Model Reference

```
Error: Model 'my-model' referenced by node 'router' is not defined
```

**Problem:** A node's `model` field references a model that doesn't exist in the `models` section.

**Fix:** Either define the model or fix the reference:
```json
{
  "models": {
    "my-model": {  // Make sure this name matches
      "type": "external",
      "url": "http://localhost:11434"
    }
  }
}
```

### Invalid JSON Syntax

```
Error: expected `,` or `}` at line 15 column 3
```

**Problem:** JSON syntax error—often a missing comma or extra trailing comma.

**Fix:** Check line 15 for missing/extra commas, unclosed braces, or typos.

### Missing Required Field in Model

```
Error: Model 'router' is missing required field 'url'
```

**Problem:** External models need a URL.

**Fix:**
```json
{
  "models": {
    "router": {
      "type": "external",
      "interface": "openai-api",
      "url": "http://localhost:11434"  // Add the URL
    }
  }
}
```

### Duplicate Node Names

```
Error: Duplicate node name 'handler' in architecture
```

**Problem:** Two nodes have the same name.

**Fix:** Give each node a unique name:
```json
{
  "architecture": [
    {"name": "handler-a", "layer": 1, ...},
    {"name": "handler-b", "layer": 1, ...}  // Unique names
  ]
}
```

## Common Patterns

### Pre-Commit Hook

Validate configurations before committing:

```bash
#!/bin/bash
# .git/hooks/pre-commit

# Find all changed config files
for file in $(git diff --cached --name-only | grep -E '\.(json|yaml)$'); do
  if [[ -f "$file" ]]; then
    if ! llmnet validate "$file" > /dev/null 2>&1; then
      echo "Validation failed: $file"
      llmnet validate "$file"
      exit 1
    fi
  fi
done
```

### CI/CD Pipeline

```yaml
# .github/workflows/validate.yml
name: Validate Configs
on: [push, pull_request]

jobs:
  validate:
    runs-on: ubuntu-latest
    steps:
      - uses: actions/checkout@v4
      - name: Install llmnet
        run: cargo install --path .
      - name: Validate all configs
        run: |
          for config in configs/*.json; do
            echo "Validating $config"
            llmnet validate "$config"
          done
```

### Batch Validation

```bash
#!/bin/bash
# validate-all.sh

failed=0
for file in configs/*.json pipelines/*.yaml; do
  if ! llmnet validate "$file" > /dev/null 2>&1; then
    echo "FAIL: $file"
    failed=1
  else
    echo "OK: $file"
  fi
done

exit $failed
```

### Development Workflow

```bash
# Edit your config
vim new-pipeline.json

# Validate before trying to run
llmnet validate new-pipeline.json

# If valid, test locally
llmnet run new-pipeline.json

# If that works, deploy
llmnet deploy new-pipeline.json
```

## What Gets Validated

| Check | Description |
|-------|-------------|
| JSON/YAML syntax | Valid structure, no typos |
| Required fields | `architecture` array present |
| Model definitions | Each model has required fields |
| Node references | Nodes reference existing models |
| Output references | `output-to` references valid layers/nodes |
| Condition syntax | `if` conditions are parseable |

## What Doesn't Get Validated

| Not Checked | Why |
|-------------|-----|
| Model URL reachability | Would require network access |
| API key validity | Secrets aren't tested |
| Model compatibility | Would require running models |
| Resource availability | No cluster connection |
| Runtime behavior | Static analysis only |

To catch these issues, use `llmnet deploy --dry-run` which connects to the control plane for more thorough validation.

## Exit Codes

| Code | Meaning |
|------|---------|
| 0 | Configuration is valid |
| 1 | Configuration is invalid or file not found |

Use exit codes in scripts:
```bash
if llmnet validate config.json; then
  echo "Good to go!"
else
  echo "Fix your config first"
  exit 1
fi
```

## Comparison with Other Commands

| Command | Validation Level |
|---------|------------------|
| `llmnet validate` | Syntax and structure only (offline) |
| `llmnet deploy --dry-run` | Full validation including cluster state |
| `llmnet run` | Validates then runs (catches runtime issues) |

## See Also

- [deploy](./deploy.md) - Deploy with `--dry-run` for deeper validation
- [run](./run.md) - Run locally to test configurations
