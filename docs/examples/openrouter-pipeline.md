# OpenRouter Pipeline (Cloud-Native 1-2-1)

> **Config file:** [`examples/openrouter-pipeline.json`](../../examples/openrouter-pipeline.json)

A production-ready dual expert setup using OpenRouter's free tier models.

## Topology

```
                    ┌─────────────────────┐
                    │  Llama 3.3 70B      │
                    │  (Creative)         │
┌────────────┐     ╱└─────────────────────┘╲     ┌──────────┐
│ Gemma 27B  │────<                        >────▶│  Output  │
│ (Router)   │     ╲┌─────────────────────┐╱     └──────────┘
└────────────┘      │  Qwen3 Coder        │
                    │  (Technical)        │
                    └─────────────────────┘
```

**Models Used:**
- Router: `google/gemma-3-27b-it:free`
- Creative: `meta-llama/llama-3.3-70b-instruct:free`
- Technical: `qwen/qwen3-coder:free`

## How It Works

This example demonstrates a complete cloud deployment using OpenRouter:

1. All models accessed via single API endpoint (openrouter.ai)
2. Model selection via `model_override` in extra-options
3. Free tier models for cost-effective operation
4. Creative vs technical specialization

## Configuration

```json
{
  "models": {
    "router-gemma": {
      "type": "external",
      "interface": "openai-api",
      "url": "https://openrouter.ai/api",
      "api-key": "$OPENROUTER_API_KEY_FREE"
    },
    "handler-llama": {
      "type": "external",
      "interface": "openai-api",
      "url": "https://openrouter.ai/api",
      "api-key": "$OPENROUTER_API_KEY_FREE"
    }
  },
  "architecture": [
    {
      "name": "router",
      "layer": 0,
      "model": "router-gemma",
      "output-to": [1],
      "extra-options": {
        "model_override": "google/gemma-3-27b-it:free"
      }
    },
    {
      "name": "creative-handler",
      "layer": 1,
      "model": "handler-llama",
      "use-case": "Creative writing, storytelling, poetry",
      "extra-options": {
        "model_override": "meta-llama/llama-3.3-70b-instruct:free"
      }
    },
    {
      "name": "technical-handler",
      "layer": 1,
      "use-case": "Technical questions, coding, math, science",
      "extra-options": {
        "model_override": "qwen/qwen3-coder:free"
      }
    }
  ]
}
```

## Real-Life Use Cases

### 1. Content Creation Platform

**Scenario:** Marketing agency with both copywriting and technical documentation needs.

| Handler | Content Type |
|---------|-------------|
| Creative (Llama) | Blog posts, social media, ad copy, storytelling |
| Technical (Qwen) | API docs, tutorials, technical specs, READMEs |

**Query examples:**
- "Write a compelling product description for our new app" → Creative
- "Document this REST API endpoint with examples" → Technical

### 2. Developer Assistant

**Scenario:** Coding copilot that also helps with documentation and planning.

| Handler | Task Type |
|---------|----------|
| Creative (Llama) | Architecture discussions, brainstorming, explanations |
| Technical (Qwen) | Code generation, debugging, refactoring |

**Query examples:**
- "Explain the tradeoffs between microservices and monolith" → Creative
- "Write a Python function to merge two sorted lists" → Technical

### 3. Educational Tutoring

**Scenario:** Learning platform for programming and creative writing.

| Handler | Subject |
|---------|---------|
| Creative (Llama) | Essay writing, storytelling, language arts |
| Technical (Qwen) | Programming, math problems, science concepts |

### 4. Customer Communication

**Scenario:** Support team drafting responses of different types.

| Handler | Response Type |
|---------|--------------|
| Creative (Llama) | Apology letters, thank you notes, marketing emails |
| Technical (Qwen) | Troubleshooting guides, setup instructions, FAQs |

### 5. Research Assistant

**Scenario:** Academic tool for different types of writing tasks.

| Handler | Task |
|---------|------|
| Creative (Llama) | Literature reviews, abstract writing, grant narratives |
| Technical (Qwen) | Data analysis explanations, methodology sections, code |

## Model Selection Strategy

### Why Gemma for Routing?
- Fast inference (small enough for quick decisions)
- Good at classification tasks
- Free tier available
- Reliable intent detection

### Why Llama for Creative?
- Excellent at narrative and engaging content
- Strong reasoning for complex topics
- 70B parameter model = high quality output
- Free tier via OpenRouter

### Why Qwen Coder for Technical?
- Specialized for code generation
- Strong at structured/precise outputs
- Optimized for programming tasks
- Free tier available

## Cost Optimization with OpenRouter

OpenRouter provides:
- **Free tier models:** No cost for development/testing
- **Pay-per-token:** Only pay for what you use
- **Model flexibility:** Switch models without code changes

```json
// Development (free)
"model_override": "google/gemma-3-27b-it:free"

// Production (paid, higher limits)
"model_override": "anthropic/claude-3-sonnet"
```

## Environment Setup

```bash
# Get your API key from https://openrouter.ai
export OPENROUTER_API_KEY_FREE="sk-or-..."

# Run the pipeline
llmnet examples/openrouter-pipeline.json
```

## Testing the Pipeline

```bash
# Creative query
curl -X POST http://localhost:8080/v1/chat/completions \
  -H "Content-Type: application/json" \
  -d '{"model": "llmnet", "messages": [{"role": "user", "content": "Write a haiku about programming"}]}'

# Technical query
curl -X POST http://localhost:8080/v1/chat/completions \
  -H "Content-Type: application/json" \
  -d '{"model": "llmnet", "messages": [{"role": "user", "content": "How do I reverse a linked list in Python?"}]}'
```

## Extending for Production

### Add More Handlers

```json
{
  "name": "analysis-handler",
  "layer": 1,
  "use-case": "Data analysis, statistics, research methodology",
  "extra-options": {
    "model_override": "anthropic/claude-3-haiku"
  }
}
```

### Upgrade to Paid Models

```json
{
  "extra-options": {
    "model_override": "anthropic/claude-3-opus"  // Premium model
  }
}
```

### Add Response Refinement

See [Multi-Layer Pipeline](./multi-layer-pipeline.md) for adding a refinement layer.

**Next step:** [Conditional Routing](./conditional-routing.md) for rule-based decisions.
