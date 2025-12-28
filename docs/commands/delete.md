# llmnet delete

Remove resources from your LLMNet cluster. Use this to remove pipelines that are no longer needed or unregister nodes from the cluster.

## Synopsis

```
llmnet delete <RESOURCE> <NAME> [OPTIONS]
```

## Resources

| Resource | Aliases | Description |
|----------|---------|-------------|
| `pipeline` | `pl` | Delete a deployed pipeline |
| `node` | `no` | Unregister a node from the cluster |

## What It Does

The `delete` command removes resources from the cluster:

- **Deleting a pipeline** stops all running replicas and removes the pipeline configuration from the control plane
- **Deleting a node** unregisters it from the cluster (the node process itself keeps running, but won't receive new work)

Think of it like:
- `rm` for files, but for LLM pipelines
- Removing a machine from a load balancer pool
- Cleaning up resources you no longer need

## Commands

### llmnet delete pipeline

Remove a deployed pipeline.

```
llmnet delete pipeline <NAME> [OPTIONS]
```

**Arguments:**

| Argument | Type | Required | Default | Description |
|----------|------|----------|---------|-------------|
| `<NAME>` | string | yes | - | Name of the pipeline to delete |
| `-n, --namespace` | string | no | `default` | Namespace where the pipeline lives |

### llmnet delete node

Unregister a node from the cluster.

```
llmnet delete node <NAME>
```

**Arguments:**

| Argument | Type | Required | Description |
|----------|------|----------|-------------|
| `<NAME>` | string | yes | Name of the node to unregister |

## Examples

### Delete a Pipeline from Default Namespace

```bash
llmnet delete pipeline my-chatbot
```

**What happens:**
1. The control plane locates `my-chatbot` in the `default` namespace
2. All running replicas are terminated
3. The pipeline is removed from the cluster registry
4. Resources on worker nodes are freed

Output:
```
pipeline.llmnet/my-chatbot deleted from namespace default
```

### Delete a Pipeline from a Specific Namespace

```bash
llmnet delete pipeline customer-service --namespace production
```

**What happens:** Same as above, but targets the `production` namespace. Useful when you have pipelines with the same name in different namespaces.

Output:
```
pipeline.llmnet/customer-service deleted from namespace production
```

### Use Short Aliases

```bash
# These are equivalent
llmnet delete pipeline my-pipeline
llmnet delete pl my-pipeline

# With namespace
llmnet delete pl my-pipeline -n staging
```

### Unregister a Node

```bash
llmnet delete node gpu-worker-2
```

**What happens:**
1. The node is marked as unregistered in the control plane
2. The control plane stops assigning new pipeline replicas to this node
3. Existing pipelines continue running (they're not automatically migrated)
4. The node process itself keeps running unless you stop it separately

Output:
```
node.llmnet/gpu-worker-2 deleted
```

> **Note:** Deleting a node doesn't stop the `llmnet serve` process on that machine. It just removes it from the cluster's registry. To fully decommission a node:
> 1. First, scale down or migrate pipelines running on it
> 2. Delete the node from the cluster: `llmnet delete node <name>`
> 3. Stop the `llmnet serve` process on the machine

### Delete Multiple Resources

```bash
# Delete pipelines one by one (there's no bulk delete yet)
llmnet delete pipeline test-1 -n dev
llmnet delete pipeline test-2 -n dev
llmnet delete pipeline test-3 -n dev
```

## What Gets Deleted

### When Deleting a Pipeline

| What | Deleted? |
|------|----------|
| Pipeline configuration | Yes |
| Running replicas | Yes (terminated) |
| Request history | Yes |
| Underlying model data | No (models are external) |
| Worker node configuration | No |

### When Deleting a Node

| What | Deleted? |
|------|----------|
| Node registration | Yes |
| Pipelines running on node | No (orphaned) |
| Node's server process | No (keeps running) |
| Heartbeat monitoring | Yes |

## Error Handling

### Pipeline Not Found

```bash
$ llmnet delete pipeline nonexistent
Error: Pipeline 'nonexistent' not found in namespace 'default'
```

**What to do:**
1. Check the pipeline name: `llmnet get pipelines`
2. Check if it's in a different namespace: `llmnet get pipelines -A`

### Node Not Found

```bash
$ llmnet delete node unknown-node
Error: Node 'unknown-node' not found
```

**What to do:** List registered nodes: `llmnet get nodes`

### Connection Errors

```bash
$ llmnet delete pipeline my-pipeline
Error: Connection failed to http://0.0.0.0:8181: Connection refused
```

**What to do:**
1. Check that the control plane is running: `llmnet serve --control-plane`
2. Verify your current context: `llmnet context current`

## Common Patterns

### Clean Slate for Development

```bash
# Delete all pipelines in dev namespace
for pipeline in $(llmnet get pipelines -n dev -o names); do
  llmnet delete pipeline "$pipeline" -n dev
done
```

### Decommissioning a Node

```bash
# 1. First, cordon the node (prevent new scheduling) - future feature
# 2. Wait for pipelines to drain or manually migrate them
# 3. Delete the node
llmnet delete node old-worker

# 4. On the node machine, stop the server
# (Ctrl+C or kill the process)
```

### Teardown Before Redeployment

```bash
# Delete old version
llmnet delete pipeline api-service -n production

# Deploy new version
llmnet deploy new-config.yaml -n production
```

## Differences from Other Tools

| Action | kubectl | llmnet |
|--------|---------|--------|
| Delete deployment | `kubectl delete deployment name` | `llmnet delete pipeline name` |
| Delete node | `kubectl delete node name` | `llmnet delete node name` |
| Force delete | `--force --grace-period=0` | Not yet supported |
| Delete by file | `kubectl delete -f file.yaml` | Not yet supported |

## See Also

- [deploy](./deploy.md) - Deploy new pipelines
- [get](./get.md) - List resources before deleting
- [scale](./scale.md) - Scale down replicas (alternative to delete)
- [context](./context.md) - Ensure you're connected to the right cluster
