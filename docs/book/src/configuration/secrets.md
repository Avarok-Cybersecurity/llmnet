# Secrets

Secrets allow you to securely load credentials from various sources without hardcoding them in your composition files.

## Secret Sources

### Environment File

Load from a `.env` file:

```json
{
  "secrets": {
    "api-creds": {
      "source": "env-file",
      "path": "~/.config/llmnet/.env",
      "variables": ["API_KEY", "API_SECRET"]
    }
  }
}
```

The `.env` file format:
```env
# Comment lines are ignored
API_KEY=sk-abc123...
API_SECRET="quoted values work too"
```

| Property | Type | Default | Description |
|----------|------|---------|-------------|
| `path` | string | required | Path to .env file (~ is expanded) |
| `variables` | array | `[]` | Variables to load (empty = load all) |

### System Environment

Load from a system environment variable:

```json
{
  "secrets": {
    "hf-token": {
      "source": "env",
      "variable": "HF_TOKEN"
    }
  }
}
```

| Property | Type | Description |
|----------|------|-------------|
| `variable` | string | Environment variable name |

### HashiCorp Vault

Load from Vault KV v2:

```json
{
  "secrets": {
    "vault-creds": {
      "source": "vault",
      "address": "https://vault.example.com",
      "path": "secret/data/llmnet/api",
      "variables": ["API_KEY", "DB_PASSWORD"],
      "token-env": "VAULT_TOKEN"
    }
  }
}
```

| Property | Type | Default | Description |
|----------|------|---------|-------------|
| `address` | string | required | Vault server URL |
| `path` | string | required | KV v2 path |
| `variables` | array | `[]` | Variables to load (empty = load all) |
| `token-env` | string | `VAULT_TOKEN` | Env var containing Vault token |

## Using Secrets

Reference secrets using the `$secrets.{name}.{variable}` syntax:

```json
{
  "functions": {
    "send-to-api": {
      "type": "rest",
      "method": "POST",
      "url": "https://api.example.com/data",
      "headers": {
        "Authorization": "Bearer $secrets.api-creds.API_KEY",
        "X-Secret": "$secrets.api-creds.API_SECRET"
      }
    }
  },
  "models": {
    "openai": {
      "type": "external",
      "interface": "openai-api",
      "url": "https://api.openai.com/v1",
      "api-key": "$secrets.api-creds.OPENAI_KEY"
    }
  }
}
```

## Best Practices

1. **Never commit secrets**: Keep `.env` files out of version control
2. **Use specific variables**: List only the variables you need
3. **Prefer Vault for production**: More secure than file-based secrets
4. **Validate early**: Use `llmnet validate` to catch missing secrets

## Example: Multi-Source Secrets

```json
{
  "secrets": {
    "local-dev": {
      "source": "env-file",
      "path": "./.env.local",
      "variables": ["DEV_API_KEY"]
    },
    "ci-token": {
      "source": "env",
      "variable": "CI_API_TOKEN"
    },
    "production": {
      "source": "vault",
      "address": "https://vault.prod.example.com",
      "path": "secret/data/llmnet/prod",
      "variables": ["PROD_API_KEY", "PROD_DB_URL"]
    }
  }
}
```
