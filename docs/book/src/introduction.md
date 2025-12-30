# Introduction

**llmnet** is a CLI tool for orchestrating LLM pipelines using small, efficient models routed intelligently.

> *The future is small models routed to — intelligently*

## What is llmnet?

llmnet allows you to define multi-model pipelines where:
- A **router** model intelligently directs requests to specialized handlers
- **Handler** models are specialized for specific domains or tasks
- **Hooks** execute custom logic before and after LLM calls
- **Secrets** are securely loaded from environment files, variables, or vaults

## Key Features

- **Declarative Configuration**: Define your entire pipeline in a single JSON file
- **Intelligent Routing**: Router models select the best handler for each request
- **Conditional Routing**: Route based on input characteristics (word count, hop count, etc.)
- **Hooks System**: Execute pre/post processing with REST, Shell, WebSocket, or gRPC functions
- **Secret Management**: Load credentials from env files, system environment, or HashiCorp Vault
- **Multi-Layer Pipelines**: Chain handlers through multiple processing layers
- **Local & Remote Models**: Use Ollama, vLLM, llama.cpp, or external APIs

## Architecture Overview

```
┌─────────────────────────────────────────────────────────────────┐
│                         User Request                            │
└─────────────────────────────────────────────────────────────────┘
                                │
                                ▼
┌─────────────────────────────────────────────────────────────────┐
│                      Layer 0: Router                            │
│  ┌─────────────────────────────────────────────────────────┐   │
│  │ Pre-hooks → Router LLM → Post-hooks                      │   │
│  └─────────────────────────────────────────────────────────┘   │
└─────────────────────────────────────────────────────────────────┘
                                │
                    ┌───────────┴───────────┐
                    ▼                       ▼
┌─────────────────────────────┐  ┌─────────────────────────────┐
│    Layer 1: Sales Agent     │  │   Layer 1: Support Agent    │
│  ┌───────────────────────┐  │  │  ┌───────────────────────┐  │
│  │ Pre → LLM → Post      │  │  │  │ Pre → LLM → Post      │  │
│  └───────────────────────┘  │  │  └───────────────────────┘  │
└─────────────────────────────┘  └─────────────────────────────┘
                    │                       │
                    └───────────┬───────────┘
                                ▼
                    ┌─────────────────────────────┐
                    │         Output              │
                    └─────────────────────────────┘
```

## Getting Started

Jump to the [Quick Start](./getting-started/quick-start.md) guide to create your first pipeline in minutes.
