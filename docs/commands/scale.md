# llmnet scale

Change the number of replicas for a deployed pipeline. Use this to scale up for more capacity or scale down to save resources.

## Synopsis

```
llmnet scale <NAME> --replicas <COUNT> [OPTIONS]
```

## Arguments

| Argument | Type | Required | Default | Description |
|----------|------|----------|---------|-------------|
| `<NAME>` | string | yes | - | Name of the pipeline to scale |
| `--replicas` | number | yes | - | Desired number of replicas |
| `-n, --namespace` | string | no | `default` | Namespace where the pipeline lives |

## What It Does

The `scale` command adjusts how many instances of your pipeline are running across the cluster. This is called "horizontal scaling" because you're adding more copies rather than making individual copies more powerful.

When you scale:

1. **Scale up (increase replicas):** The control plane schedules additional instances onto available worker nodes. More replicas mean more capacity to handle concurrent requests.

2. **Scale down (decrease replicas):** The control plane terminates excess replicas, freeing up resources. Requests are redistributed to remaining replicas.

Think of it like:
- Adding or removing workers from a team
- Opening or closing checkout lanes at a store
- Expanding or shrinking a server pool

## Examples

### Scale Up a Pipeline

```bash
llmnet scale my-chatbot --replicas 5
```

**What happens:**
1. Control plane checks current replica count (let's say it's 2)
2. Finds 3 available worker nodes with capacity
3. Schedules 3 new replicas
4. New replicas start handling requests once healthy

Output:
```
pipeline.llmnet/my-chatbot scaled from 2 to 5 replicas in namespace default
```

### Scale Down a Pipeline

```bash
llmnet scale my-chatbot --replicas 1
```

**What happens:**
1. Control plane checks current replica count (let's say it's 5)
2. Selects 4 replicas for termination
3. Gracefully stops those replicas
4. Remaining replica continues handling all requests

Output:
```
pipeline.llmnet/my-chatbot scaled from 5 to 1 replicas in namespace default
```

### Scale a Pipeline in a Specific Namespace

```bash
llmnet scale customer-service --replicas 10 --namespace production
```

**What happens:** Scales the `customer-service` pipeline in the `production` namespace to 10 replicas. Useful when you have pipelines with the same name in different environments.

### Scale to Zero (Pause)

```bash
llmnet scale my-pipeline --replicas 0
```

**What happens:**
1. All replicas are terminated
2. The pipeline configuration remains in the cluster
3. No requests can be served (they'll fail or timeout)
4. Resources are freed on worker nodes

This is useful when you want to:
- Temporarily pause a pipeline without deleting it
- Save resources during off-peak hours
- Debug issues without serving traffic

To resume, simply scale back up:
```bash
llmnet scale my-pipeline --replicas 3
```

### Check Current Replica Count First

```bash
# See current state
llmnet get pipelines

# Output shows REPLICAS and READY columns
# NAMESPACE   NAME          REPLICAS   READY   STATUS
# default     my-chatbot    2          2/2     Running

# Then scale as needed
llmnet scale my-chatbot --replicas 5
```

## Understanding Scaling

### How Replicas Get Distributed

When you scale up, the control plane's scheduler decides where to place new replicas:

1. **Check capacity:** Which nodes have room for more pipelines?
2. **Balance load:** Try to distribute evenly across nodes
3. **Start replicas:** Tell worker nodes to start new instances

Current scheduling is simple round-robin. Future versions may consider:
- GPU availability
- Memory requirements
- Node labels and affinity rules
- Current node load

### Scaling vs. Ready Count

The `REPLICAS` and `READY` columns in `llmnet get pipelines` show different things:

| Column | Meaning |
|--------|---------|
| REPLICAS | The desired count (what you set with `scale`) |
| READY | Actual running instances (format: `running/desired`) |

Examples:
- `3/3` = All replicas running (healthy)
- `2/3` = One replica still starting or unhealthy
- `0/3` = No replicas running yet (something's wrong)

### Scaling Limitations

| Limit | Description |
|-------|-------------|
| Max replicas | Limited by available nodes and their capacity |
| Min replicas | 0 (pauses the pipeline) |
| Scaling speed | Depends on how fast nodes can start pipelines |

## Common Patterns

### Peak Hours Scaling

```bash
# Morning: scale up for busy hours
llmnet scale api-service --replicas 10 -n production

# Evening: scale down to save resources
llmnet scale api-service --replicas 2 -n production
```

### Quick Capacity Test

```bash
# Double capacity temporarily
llmnet scale my-pipeline --replicas 10

# Run load tests...

# Restore normal capacity
llmnet scale my-pipeline --replicas 5
```

### Gradual Rollout with Scaling

```bash
# Start with minimal replicas
llmnet deploy new-config.yaml  # defaults to 1 replica

# Verify it works
curl http://localhost:8080/health

# Gradually increase
llmnet scale new-pipeline --replicas 3
# test more...
llmnet scale new-pipeline --replicas 5
# test more...
llmnet scale new-pipeline --replicas 10
```

### Emergency Capacity

```bash
# Sudden traffic spike? Scale up immediately
llmnet scale my-chatbot --replicas 20 -n production

# Monitor status
watch llmnet get pipelines -n production
```

## Error Handling

### Pipeline Not Found

```bash
$ llmnet scale nonexistent --replicas 5
Error: Pipeline 'nonexistent' not found in namespace 'default'
```

**What to do:**
1. Check the pipeline name: `llmnet get pipelines`
2. Check the namespace: `llmnet get pipelines -A`

### Not Enough Nodes

```bash
$ llmnet scale my-pipeline --replicas 100
Warning: Only scheduled 12 replicas (not enough node capacity)
pipeline.llmnet/my-pipeline scaled from 3 to 12 replicas in namespace default
```

**What to do:**
1. Add more worker nodes: `llmnet serve --node-name new-worker ...`
2. Or accept the partial scaling

### Connection Error

```bash
$ llmnet scale my-pipeline --replicas 5
Error: Connection failed to http://0.0.0.0:8181: Connection refused
```

**What to do:**
1. Check the control plane is running
2. Verify your context: `llmnet context current`

## Scaling vs. Other Commands

| Goal | Command |
|------|---------|
| Run more instances | `llmnet scale --replicas N` |
| Update configuration | `llmnet delete` + `llmnet deploy` |
| Pause without deleting | `llmnet scale --replicas 0` |
| Remove completely | `llmnet delete pipeline` |

## Comparison with Other Tools

| Action | kubectl | llmnet |
|--------|---------|--------|
| Scale up | `kubectl scale deployment/name --replicas=5` | `llmnet scale name --replicas 5` |
| Scale to zero | `kubectl scale deployment/name --replicas=0` | `llmnet scale name --replicas 0` |
| Auto-scaling | `kubectl autoscale deployment/name` | Not yet supported |

## Future Features

These features are planned for future releases:

- **Auto-scaling:** Automatically adjust replicas based on request volume
- **Scheduled scaling:** Scale up/down on a schedule
- **Canary deployments:** Gradually shift traffic to new versions

## See Also

- [deploy](./deploy.md) - Deploy new pipelines (sets initial replica count)
- [get](./get.md) - Check current replica counts
- [delete](./delete.md) - Remove pipelines entirely
- [status](./status.md) - View cluster capacity
