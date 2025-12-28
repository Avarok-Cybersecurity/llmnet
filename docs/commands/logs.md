# llmnet logs

View logs from a running pipeline. This command streams log output from pipeline replicas for debugging and monitoring.

> **Note:** This feature is planned for a future release. The command is defined but log streaming is not yet implemented.

## Synopsis

```
llmnet logs <NAME> [OPTIONS]
```

## Arguments

| Argument | Type | Required | Default | Description |
|----------|------|----------|---------|-------------|
| `<NAME>` | string | yes | - | Name of the pipeline |
| `-n, --namespace` | string | no | `default` | Namespace where the pipeline lives |
| `-f, --follow` | flag | no | false | Stream logs continuously (like `tail -f`) |
| `--tail` | number | no | `100` | Number of recent lines to show |

## What It Will Do

When implemented, the `logs` command will:

1. **Connect to the control plane** to find pipeline replicas
2. **Stream log output** from running replicas
3. **Display logs** with timestamps and replica identifiers
4. **Support following** for real-time monitoring

Think of it like:
- `kubectl logs` for Kubernetes pods
- `docker logs` for containers
- `tail -f` for log files

## Planned Examples

### View Recent Logs

```bash
llmnet logs my-chatbot
```

**Expected output:**
```
[2024-01-15 10:23:45] [replica-1] INFO Request received from 192.168.1.100
[2024-01-15 10:23:45] [replica-1] INFO Routing to: code-handler
[2024-01-15 10:23:46] [replica-1] INFO Response sent (1.2s)
[2024-01-15 10:23:50] [replica-2] INFO Request received from 192.168.1.101
...
```

### Follow Logs in Real-Time

```bash
llmnet logs my-chatbot --follow
```

**What it will do:** Continuously stream new log entries as they appear. Press Ctrl+C to stop.

### View Logs from Specific Namespace

```bash
llmnet logs customer-service --namespace production
```

**What it will do:** Show logs from the `customer-service` pipeline in the `production` namespace.

### Limit Number of Lines

```bash
llmnet logs my-pipeline --tail 50
```

**What it will do:** Show only the last 50 lines of logs.

### Combine Options

```bash
llmnet logs api-service -n production --follow --tail 20
```

**What it will do:** Show the last 20 lines, then continue streaming new entries.

## Current Status

Running the command currently shows:

```bash
$ llmnet logs my-pipeline
WARN Log streaming not yet implemented for pipeline 'default/my-pipeline'
```

## Workarounds

Until log streaming is implemented, you can:

### Check Worker Node Logs Directly

SSH into your worker nodes and view the llmnet process output:

```bash
# On the worker node
journalctl -u llmnet -f

# Or if running in foreground
# Check the terminal where llmnet serve is running
```

### Use Verbose Mode When Running

For local development with `llmnet run`:

```bash
llmnet -vvv run config.json
```

This increases log verbosity on your local terminal.

### Monitor via HTTP

Check pipeline health endpoints:

```bash
# Health check
curl http://worker-node:8080/health

# Request metrics (if available)
curl http://control-plane:8181/v1/status
```

## Planned Features

When fully implemented, logs will support:

| Feature | Description |
|---------|-------------|
| Multi-replica aggregation | Combine logs from all replicas |
| Filtering by log level | `--level=error` to show only errors |
| Filtering by time | `--since=1h` for last hour |
| JSON output | `--output=json` for parsing |
| Color coding | Different colors for different replicas |

## Comparison with Other Tools

| Action | kubectl | llmnet (planned) |
|--------|---------|------------------|
| View logs | `kubectl logs pod-name` | `llmnet logs pipeline-name` |
| Follow logs | `kubectl logs -f pod-name` | `llmnet logs -f pipeline-name` |
| Tail lines | `kubectl logs --tail=100` | `llmnet logs --tail 100` |
| All replicas | `kubectl logs -l app=name` | Automatic |

## See Also

- [get](./get.md) - List pipelines and their status
- [status](./status.md) - Quick cluster health overview
- [serve](./serve.md) - View logs where the server runs
