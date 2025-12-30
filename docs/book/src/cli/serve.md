# serve

Start the HTTP server for the pipeline.

## Usage

```bash
llmnet serve <COMPOSITION> [OPTIONS]
```

## Options

| Option | Default | Description |
|--------|---------|-------------|
| `--port` | 8080 | HTTP port |
| `--host` | 127.0.0.1 | Bind address |

## Example

```bash
llmnet serve pipeline.json --port 3000 --host 0.0.0.0
```

## Endpoints

| Endpoint | Method | Description |
|----------|--------|-------------|
| `/health` | GET | Health check |
| `/v1/chat/completions` | POST | Chat completion |
