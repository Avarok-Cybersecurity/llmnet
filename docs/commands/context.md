# llmnet context

Manage connections to LLMNet clusters. Contexts let you switch between different environments (local, staging, production) without retyping URLs every time.

## Synopsis

```
llmnet context <ACTION> [ARGS]
```

## Actions

| Action | Description |
|--------|-------------|
| `list` | Show all configured contexts |
| `current` | Show the currently active context |
| `use <name>` | Switch to a different context |
| `add <name> --url <url>` | Add a new context |
| `delete <name>` | Remove a context |

## What Is a Context?

A context is a saved connection to an LLMNet control plane. Instead of typing `--url http://192.168.1.100:8181` every time, you save it once as a named context and switch to it with `llmnet context use`.

Think of it like:
- SSH config entries (`Host myserver`)
- Git remotes (`origin`, `upstream`)
- kubectl contexts for different clusters
- Database connection bookmarks

### Where Contexts Are Stored

Contexts are saved in `~/.llmnet/config` as YAML:

```yaml
current-context: production
contexts:
  production:
    name: production
    url: http://prod-cluster.example.com:8181
    api_key: sk-prod-key-123
  staging:
    name: staging
    url: http://staging.internal:8181
  dev:
    name: dev
    url: http://localhost:8181
local:
  port: 8181
  bind_addr: "0.0.0.0"
```

## Commands

### llmnet context list

Show all configured contexts.

```
llmnet context list
```

**Output:**
```
CURRENT   NAME        URL
*         production  http://prod-cluster.example.com:8181
          staging     http://staging.internal:8181
          dev         http://localhost:8181
          local       http://0.0.0.0:8181
```

The `*` indicates the currently active context.

### llmnet context current

Show which context is currently active.

```
llmnet context current
```

**Output:**
```
Current context: production
URL: http://prod-cluster.example.com:8181
```

### llmnet context use

Switch to a different context.

```
llmnet context use <NAME>
```

**Arguments:**

| Argument | Type | Required | Description |
|----------|------|----------|-------------|
| `<NAME>` | string | yes | Name of the context to switch to |

### llmnet context add

Add a new context.

```
llmnet context add <NAME> --url <URL> [--api-key <KEY>]
```

**Arguments:**

| Argument | Type | Required | Description |
|----------|------|----------|-------------|
| `<NAME>` | string | yes | Name for this context |
| `--url` | string | yes | URL of the control plane |
| `--api-key` | string | no | API key for authentication |

### llmnet context delete

Remove a saved context.

```
llmnet context delete <NAME>
```

**Arguments:**

| Argument | Type | Required | Description |
|----------|------|----------|-------------|
| `<NAME>` | string | yes | Name of the context to delete |

## Examples

### Add a Remote Cluster

```bash
llmnet context add my-cluster --url http://192.168.1.100:8181
```

**What happens:**
1. A new context named `my-cluster` is created
2. It's saved to `~/.llmnet/config`
3. You can now switch to it with `context use`

Output:
```
Context 'my-cluster' added (http://192.168.1.100:8181)
```

### Add a Cluster with API Key

```bash
llmnet context add production \
  --url https://api.llmnet.example.com:8181 \
  --api-key "sk-prod-secret-key"
```

**What happens:** Same as above, but the API key is stored for authentication. The key will be sent with every request to this cluster.

### Switch Between Contexts

```bash
# Check where you are
llmnet context current
# Output: Current context: local

# Switch to production
llmnet context use production

# Verify the switch
llmnet context current
# Output: Current context: production

# Now all commands go to production
llmnet get pipelines
```

### Use the Local Context

The `local` context is special—it always exists and points to your local control plane:

```bash
llmnet context use local
```

**What happens:** Commands will now target `http://0.0.0.0:8181` (or whatever you've configured in your local settings).

### List All Contexts

```bash
llmnet context list
```

**Output:**
```
CURRENT   NAME        URL
          local       http://0.0.0.0:8181
*         production  https://prod.example.com:8181
          staging     http://staging.internal:8181
          dev         http://localhost:8181
```

### Delete a Context

```bash
llmnet context delete old-cluster
```

**What happens:**
1. The context is removed from `~/.llmnet/config`
2. If it was the current context, current-context is cleared

Output:
```
Context 'old-cluster' deleted
```

## Common Patterns

### Development Workflow

```bash
# Start of day: work on local
llmnet context use local
llmnet serve --control-plane &
llmnet deploy dev-config.json

# Ready to test in staging
llmnet context use staging
llmnet deploy dev-config.json -n testing

# Looks good, deploy to production
llmnet context use production
llmnet deploy prod-config.yaml -n production
```

### Quick Context Check Before Deploy

```bash
# Always check where you are before destructive operations!
llmnet context current

# Make sure you're not in production when testing
llmnet context use staging
llmnet delete pipeline experimental-feature
```

### Setup Script for New Developers

```bash
#!/bin/bash
# setup-contexts.sh

echo "Setting up LLMNet contexts..."

llmnet context add dev --url http://dev.internal:8181
llmnet context add staging --url http://staging.internal:8181
llmnet context add production --url https://prod.example.com:8181 --api-key "$PROD_API_KEY"

echo "Available contexts:"
llmnet context list

echo "Switching to dev..."
llmnet context use dev
```

### Multiple Teams, Multiple Clusters

```bash
# Data team's cluster
llmnet context add data-team --url http://data-cluster:8181

# ML team's cluster
llmnet context add ml-team --url http://ml-cluster:8181

# Customer-facing cluster
llmnet context add customer --url https://customer-api:8181

# Switch between as needed
llmnet context use data-team
llmnet get pipelines
```

## The Config File

You can also edit `~/.llmnet/config` directly:

```yaml
# Current active context
current-context: staging

# Named contexts
contexts:
  production:
    name: production
    url: https://prod.example.com:8181
    api_key: sk-prod-key-123
    description: Production cluster

  staging:
    name: staging
    url: http://staging.internal:8181
    description: Staging environment

  dev:
    name: dev
    url: http://localhost:8181

# Local settings (used when current-context is "local" or unset)
local:
  port: 8181
  bind_addr: "0.0.0.0"
```

## Error Handling

### Context Not Found

```bash
$ llmnet context use nonexistent
Error: Context 'nonexistent' not found
```

**What to do:**
1. List available contexts: `llmnet context list`
2. Add the context if needed: `llmnet context add nonexistent --url ...`

### Connection Failed

```bash
$ llmnet context use production
$ llmnet get pipelines
Error: Connection failed to https://prod.example.com:8181: Connection refused
```

**What to do:**
1. Check that the control plane is running at that URL
2. Verify network connectivity
3. Check if the URL is correct: `llmnet context current`

### No Current Context

```bash
$ llmnet get pipelines
Error: No current context set
```

**What to do:** Either:
- Use local: `llmnet context use local`
- Add and use a context: `llmnet context add my-cluster --url ...`

## Security Notes

### API Keys

- API keys are stored in plain text in `~/.llmnet/config`
- Set appropriate file permissions: `chmod 600 ~/.llmnet/config`
- Consider using environment variables for production keys
- Never commit the config file to version control

### Network Security

- Use HTTPS for production clusters when possible
- Consider VPN or private networks for internal clusters
- API keys are sent in request headers

## Comparison with Other Tools

| Action | kubectl | llmnet |
|--------|---------|--------|
| List contexts | `kubectl config get-contexts` | `llmnet context list` |
| Current context | `kubectl config current-context` | `llmnet context current` |
| Switch context | `kubectl config use-context` | `llmnet context use` |
| Add context | `kubectl config set-context` | `llmnet context add` |
| Delete context | `kubectl config delete-context` | `llmnet context delete` |

## See Also

- [serve](./serve.md) - Start a local control plane
- [status](./status.md) - Check cluster connectivity
- [deploy](./deploy.md) - Deploy to the current context
- [get](./get.md) - List resources in the current context
