# Functions

Functions are reusable operations that can be called from hooks. They support REST APIs, shell commands, WebSockets, and gRPC.

## Function Types

### REST

HTTP requests to external services.

```json
{
  "functions": {
    "log-request": {
      "type": "rest",
      "method": "POST",
      "url": "https://api.example.com/log",
      "headers": {
        "Authorization": "Bearer $secrets.api.TOKEN",
        "Content-Type": "application/json"
      },
      "body": {
        "node": "$NODE",
        "input": "$INPUT",
        "output": "$OUTPUT"
      },
      "timeout": 10
    }
  }
}
```

| Property | Type | Default | Description |
|----------|------|---------|-------------|
| `method` | string | `GET` | HTTP method: GET, POST, PUT, PATCH, DELETE |
| `url` | string | required | Target URL (supports variable substitution) |
| `headers` | object | `{}` | HTTP headers |
| `body` | object | optional | JSON body for POST/PUT/PATCH |
| `timeout` | number | `30` | Timeout in seconds |

### Shell

Execute local commands.

```json
{
  "functions": {
    "validate-json": {
      "type": "shell",
      "command": "python",
      "args": ["validate.py", "--input", "$OUTPUT"],
      "env": {
        "PYTHONPATH": "/app/lib"
      },
      "cwd": "/app/scripts",
      "timeout": 30
    }
  }
}
```

| Property | Type | Default | Description |
|----------|------|---------|-------------|
| `command` | string | required | Executable to run |
| `args` | array | `[]` | Command arguments |
| `env` | object | `{}` | Environment variables |
| `cwd` | string | optional | Working directory |
| `timeout` | number | `30` | Timeout in seconds |

### WebSocket

Send messages to WebSocket servers.

```json
{
  "functions": {
    "notify-dashboard": {
      "type": "websocket",
      "url": "wss://dashboard.example.com/ws",
      "headers": {
        "Authorization": "Bearer $secrets.ws.TOKEN"
      },
      "message": {
        "event": "node_complete",
        "node": "$NODE",
        "output": "$OUTPUT"
      }
    }
  }
}
```

| Property | Type | Default | Description |
|----------|------|---------|-------------|
| `url` | string | required | WebSocket URL |
| `headers` | object | `{}` | Connection headers |
| `message` | object | optional | JSON message to send |

### gRPC

Call gRPC services.

```json
{
  "functions": {
    "check-quota": {
      "type": "grpc",
      "address": "quota-service:50051",
      "service": "QuotaService",
      "method": "CheckQuota",
      "request": {
        "user_id": "$USER_ID",
        "tokens": "$TOKEN_COUNT"
      },
      "timeout": 5
    }
  }
}
```

| Property | Type | Default | Description |
|----------|------|---------|-------------|
| `address` | string | required | gRPC server address |
| `service` | string | required | Service name |
| `method` | string | required | Method name |
| `request` | object | optional | Request payload |
| `timeout` | number | `30` | Timeout in seconds |

## Variable Substitution

Functions support variable substitution in strings:

| Variable | Description |
|----------|-------------|
| `$INPUT` | Current input content |
| `$OUTPUT` | Current output (post-hooks only) |
| `$NODE` | Current node name |
| `$PREV_NODE` | Previous node name |
| `$TIMESTAMP` | ISO 8601 timestamp |
| `$REQUEST_ID` | Unique request identifier |
| `$secrets.{name}.{var}` | Secret value |

## Transform Response Format

For transform-mode hooks, the function should return the value to use:

```json
// Good: Returns the new value directly
"Hello, transformed output!"

// Also good: Object that replaces output
{"result": 42, "status": "validated"}
```

The returned value replaces the current data. For observe-mode hooks, the return value is ignored.
