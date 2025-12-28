# llmnet get

List resources in your LLMNet cluster. Use this command to see what pipelines are deployed, which nodes are available, and what namespaces exist.

## Synopsis

```
llmnet get <RESOURCE> [OPTIONS]
```

## Resources

| Resource | Aliases | Description |
|----------|---------|-------------|
| `pipelines` | `pipeline`, `pl` | List deployed LLM pipelines |
| `nodes` | `node`, `no` | List registered worker nodes |
| `namespaces` | `namespace`, `ns` | List available namespaces |

## What It Does

The `get` command queries your LLMNet control plane and displays information about different types of resources. It's your primary way to see what's running in your cluster.

Think of it like:
- `ls` for files, but for LLM pipelines
- A dashboard view of your AI infrastructure
- An inventory of what's deployed where

## Commands

### llmnet get pipelines

List all deployed LLM pipelines.

```
llmnet get pipelines [OPTIONS]
```

**Options:**

| Option | Type | Default | Description |
|--------|------|---------|-------------|
| `-n, --namespace` | string | none | Filter to a specific namespace |
| `-A, --all-namespaces` | flag | false | Show pipelines from all namespaces |

### llmnet get nodes

List all registered worker nodes in the cluster.

```
llmnet get nodes
```

No additional options.

### llmnet get namespaces

List all namespaces.

```
llmnet get namespaces
```

No additional options.

## Examples

### List All Pipelines

```bash
llmnet get pipelines
```

**Output:**
```
NAMESPACE   NAME              REPLICAS   READY   STATUS
default     basic-chatbot     1          1/1     Running
default     coding-assistant  2          2/2     Running
production  customer-service  3          3/3     Running
```

**What each column means:**
- **NAMESPACE**: The logical grouping the pipeline belongs to
- **NAME**: The pipeline's identifier
- **REPLICAS**: How many instances are configured to run
- **READY**: How many instances are actually running and healthy (ready/total)
- **STATUS**: Current state (Running, Pending, Unknown)

### List Pipelines in a Specific Namespace

```bash
llmnet get pipelines --namespace production
```

**Output:**
```
NAMESPACE    NAME              REPLICAS   READY   STATUS
production   customer-service  3          3/3     Running
production   sales-bot         2          2/2     Running
```

**What happens:** Only shows pipelines in the `production` namespace. Useful when you have many pipelines and want to focus on a specific environment.

### List Pipelines Across All Namespaces

```bash
llmnet get pipelines --all-namespaces
```

or using the short form:

```bash
llmnet get pipelines -A
```

**Output:**
```
NAMESPACE    NAME              REPLICAS   READY   STATUS
default      basic-chatbot     1          1/1     Running
default      test-pipeline     1          0/1     Pending
production   customer-service  3          3/3     Running
staging      new-feature       1          1/1     Running
```

**What happens:** Shows every pipeline regardless of namespace. Helpful for getting a complete picture of your cluster.

### Use Short Aliases

```bash
# These are all equivalent
llmnet get pipelines
llmnet get pipeline
llmnet get pl
```

**What happens:** LLMNet supports abbreviated resource names for faster typing.

### List All Nodes

```bash
llmnet get nodes
```

**Output:**
```
NAME          STATUS   ADDRESS              PIPELINES
gpu-worker-1  Ready    192.168.1.101:8080   3
gpu-worker-2  Ready    192.168.1.102:8080   2
cpu-worker-1  Ready    192.168.1.103:8080   1
```

**What each column means:**
- **NAME**: The node's identifier (set when starting the worker)
- **STATUS**: Current health (Ready, NotReady, Unknown)
- **ADDRESS**: IP and port where the node is running
- **PIPELINES**: Number of pipeline replicas running on this node

### List All Namespaces

```bash
llmnet get namespaces
```

or:

```bash
llmnet get ns
```

**Output:**
```
NAME
default
production
staging
development
```

**What happens:** Shows all namespaces that exist in the cluster. The `default` namespace always exists.

## Understanding the Output

### Pipeline Status Values

| Status | Meaning |
|--------|---------|
| `Running` | All replicas are up and healthy |
| `Pending` | Replicas are being started or waiting for resources |
| `Unknown` | Can't determine status (often means connectivity issues) |

### Node Status Values

| Status | Meaning |
|--------|---------|
| `Ready` | Node is healthy and can accept new pipelines |
| `NotReady` | Node failed health checks, won't receive new work |
| `Unknown` | Haven't heard from node recently (missed heartbeats) |

### Ready Column Format

The `READY` column shows `running/total`:
- `3/3` = All 3 replicas are running (healthy)
- `2/3` = Only 2 of 3 replicas are running (something's wrong)
- `0/1` = The pipeline isn't running at all yet (starting up or failed)

## Common Patterns

### Quick Health Check

```bash
# See if everything is running
llmnet get pipelines -A
llmnet get nodes
```

### Monitoring Script

```bash
#!/bin/bash
# check-cluster.sh

echo "=== Pipelines ==="
llmnet get pipelines -A

echo ""
echo "=== Nodes ==="
llmnet get nodes

echo ""
echo "=== Namespaces ==="
llmnet get ns
```

### Find Problem Pipelines

```bash
# Look for pipelines not fully ready
llmnet get pipelines -A | grep -v "Running"
```

### Check Specific Environment

```bash
# Before deploying to production, check current state
llmnet get pipelines -n production

# After deploying, verify it's running
llmnet get pipelines -n production
```

## When There Are No Resources

### No Pipelines Deployed

```bash
$ llmnet get pipelines
No resources found.
```

**What to do:** Deploy a pipeline with `llmnet deploy`

### No Nodes Registered

```bash
$ llmnet get nodes
No resources found.
```

**What to do:** Start worker nodes with `llmnet serve` and configure them to register with the control plane.

## Troubleshooting

### "Connection refused" Error

```bash
$ llmnet get pipelines
Error: Connection failed to http://0.0.0.0:8181: Connection refused
```

**Fix:**
1. Check that the control plane is running: `llmnet serve --control-plane`
2. Verify your current context: `llmnet context current`

### Stale Data

The `get` command shows a snapshot in time. If you're watching for changes, run the command again or use a watch loop:

```bash
# Refresh every 2 seconds
watch -n 2 llmnet get pipelines
```

### Pipeline Shows "Pending" for Too Long

If a pipeline stays in Pending status:

1. Check if there are available nodes: `llmnet get nodes`
2. Verify nodes have capacity
3. Check control plane logs for scheduling errors

## See Also

- [deploy](./deploy.md) - Deploy new pipelines
- [delete](./delete.md) - Remove pipelines or nodes
- [scale](./scale.md) - Change replica counts
- [status](./status.md) - Get overall cluster health
- [context](./context.md) - Manage which cluster you're connected to
