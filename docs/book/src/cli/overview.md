# CLI Overview

llmnet provides a command-line interface for managing LLM pipelines.

## Basic Usage

```bash
llmnet [OPTIONS] <COMMAND>
```

## Commands

| Command | Description |
|---------|-------------|
| `run` | Run a pipeline with a single prompt |
| `serve` | Start the HTTP server |
| `validate` | Validate a composition file |
| `deploy` | Deploy to a cluster |
| `status` | Show cluster status |

## Global Options

| Option | Description |
|--------|-------------|
| `-h, --help` | Print help |
| `-V, --version` | Print version |
| `--config <FILE>` | Config file path |
| `--verbose` | Verbose output |

## Quick Examples

```bash
# Validate configuration
llmnet validate pipeline.json

# Start server on port 8080
llmnet serve pipeline.json --port 8080

# Single prompt
llmnet run pipeline.json "What is machine learning?"
```
