# llmnet status

Get a quick health overview of your LLMNet cluster. This command shows you the overall state of nodes, pipelines, and namespaces at a glance.

## Synopsis

```
llmnet status
```

No additional arguments.

## What It Does

The `status` command queries the control plane and displays a summary of your cluster's health:

- **Nodes:** How many worker machines are registered and healthy
- **Pipelines:** How many pipeline instances are running vs. expected
- **Namespaces:** How many logical groupings exist

Think of it like:
- A dashboard's health indicator
- A quick "is everything okay?" check
- The summary view before diving into details

## Example

```bash
llmnet status
```

**Output:**
```
Cluster Status
==============

Nodes:     3/4 ready
Pipelines: 8/10 ready
Namespaces: 3
```

### Understanding the Output

| Line | Meaning |
|------|---------|
| `Nodes: 3/4 ready` | 3 out of 4 registered nodes are healthy |
| `Pipelines: 8/10 ready` | 8 out of 10 pipeline replicas are running |
| `Namespaces: 3` | 3 namespaces exist (e.g., default, staging, production) |

## What "Ready" Means

### Ready Nodes

A node is "ready" when:
- It's registered with the control plane
- It's passing health checks
- It hasn't missed heartbeats
- It can accept new pipeline assignments

A node is "not ready" when:
- It failed health checks
- It missed too many heartbeats
- Network connectivity is lost
- It's being terminated

### Ready Pipelines

A pipeline is "ready" when:
- All configured replicas are running
- Each replica is passing health checks
- Replicas are responding to requests

Pipelines become "not ready" when:
- Some replicas are still starting up
- Replicas crashed or are unhealthy
- Node capacity prevented scheduling all replicas

## Common Patterns

### Quick Health Check

```bash
# First thing in the morning
llmnet status

# If something looks wrong, dig deeper
llmnet get nodes
llmnet get pipelines -A
```

### Before Deployment

```bash
# Check cluster health before deploying
llmnet status

# If everything looks good, deploy
llmnet deploy new-pipeline.yaml
```

### After Making Changes

```bash
# After scaling up
llmnet scale my-pipeline --replicas 10
llmnet status  # Verify all replicas are ready
```

### Monitoring Script

```bash
#!/bin/bash
# health-check.sh

status=$(llmnet status)
echo "$status"

# Alert if not all nodes are ready
if echo "$status" | grep -q "Nodes:.*0/"; then
  echo "WARNING: No nodes ready!"
  exit 1
fi
```

### Continuous Monitoring

```bash
# Watch status every 5 seconds
watch -n 5 llmnet status
```

## Interpreting Common Scenarios

### Everything Healthy

```
Nodes:     4/4 ready
Pipelines: 12/12 ready
Namespaces: 3
```

All systems operational.

### Some Pipelines Not Ready

```
Nodes:     4/4 ready
Pipelines: 8/12 ready
Namespaces: 3
```

**What this means:** 4 pipeline replicas aren't running. Could be:
- Still starting up after a recent scale/deploy
- Some replicas crashed
- Health checks failing

**What to do:**
```bash
llmnet get pipelines -A  # See which pipelines have issues
```

### Node Down

```
Nodes:     3/4 ready
Pipelines: 9/12 ready
Namespaces: 3
```

**What this means:** One worker node isn't responding. Pipelines on that node may be affected.

**What to do:**
```bash
llmnet get nodes  # See which node is not ready
# Check that machine's network, process, etc.
```

### Cluster Empty

```
Nodes:     0/0 ready
Pipelines: 0/0 ready
Namespaces: 1
```

**What this means:** No nodes or pipelines are deployed. Just the default namespace exists.

**What to do:**
```bash
# Start worker nodes
llmnet serve --node-name worker-1 --control-plane-url http://...

# Deploy a pipeline
llmnet deploy my-pipeline.json
```

### Control Plane Just Started

```
Nodes:     0/0 ready
Pipelines: 0/0 ready
Namespaces: 1
```

A fresh control plane has:
- No registered nodes yet
- No deployed pipelines
- Only the `default` namespace

## Error Handling

### Connection Refused

```bash
$ llmnet status
Error: Connection failed to http://0.0.0.0:8181: Connection refused
```

**What to do:**
1. Check control plane is running: `llmnet serve --control-plane`
2. Check your context: `llmnet context current`

### Timeout

```bash
$ llmnet status
Error: Request timeout
```

**What to do:**
1. Check network connectivity
2. Verify the control plane is responsive
3. Try again—could be temporary

## Differences from Other Commands

| Command | Purpose |
|---------|---------|
| `llmnet status` | Quick overview of cluster health |
| `llmnet get nodes` | Detailed list of all nodes |
| `llmnet get pipelines` | Detailed list of all pipelines |
| `llmnet context current` | Shows which cluster you're connected to |

Use `status` for a quick glance, then `get` commands for details.

## Comparison with Other Tools

| Action | kubectl | llmnet |
|--------|---------|--------|
| Cluster overview | `kubectl cluster-info` | `llmnet status` |
| Node list | `kubectl get nodes` | `llmnet get nodes` |
| Component status | `kubectl get componentstatuses` | `llmnet status` |

## See Also

- [get](./get.md) - Detailed resource listings
- [context](./context.md) - Ensure you're checking the right cluster
- [serve](./serve.md) - Start the control plane
