# Nemotron Router (Enterprise Multi-Layer)

> **Config file:** [`examples/nemotron-router.json`](../../examples/nemotron-router.json)

A sophisticated enterprise pipeline with temporal routing, edge case handling, and WebSocket alerts.

## Topology

```
┌────────────────┐
│ Nemotron       │
│ Router (L0)    │
└───────┬────────┘
        │
        ▼
┌───────────────────────────────────────────┐
│                Layer 1                     │
├─────────────────────┬─────────────────────┤
│ Q3 2024 Fine-Tune   │ Q4 2024 Fine-Tune   │
│ (company-2024-q3)   │ (company-2024-q4)   │
└─────────────────────┴──────────┬──────────┘
                                 │
              ┌──────────────────┴──────────────────┐
              │                                      │
              ▼                                      ▼
    ┌─────────────────┐                    ┌─────────────────┐
    │ Final Output    │                    │ Q4 Edge Case    │
    │ (output)        │                    │ Handler (L2)    │
    └─────────────────┘                    └────────┬────────┘
                                                    │
                                                    ▼
                                           ┌───────────────────┐
                                           │ Final Output      │
                                           └───────────────────┘
              │
              │ if $OutputCustomKey
              ▼
    ┌─────────────────┐
    │ WebSocket Alert │
    │ (ws://...)      │
    └─────────────────┘
```

## Key Features

1. **Temporal routing:** Different fine-tuned models for different quarters
2. **Edge case handling:** Specialized layer for unusual Q4 scenarios
3. **Conditional WebSocket:** Alerts only when specific header is set
4. **Enterprise pattern:** Time-partitioned knowledge bases

## Configuration

```json
{
  "models": {
    "nemotron-router": {
      "type": "external",
      "interface": "openai-api",
      "url": "http://10.10.10.2:44443"
    }
  },
  "architecture": [
    {
      "name": "router-layer",
      "layer": 0,
      "model": "nemotron-router",
      "output-to": [1],
      "extra-options": {
        "UseHeaderKeys": ["OutputCustomKey"]
      }
    },
    {
      "name": "company-2024-q3-fine-tune",
      "layer": 1,
      "use-case": "If the query is about company information during 2024 Q3",
      "output-to": ["final-output"]
    },
    {
      "name": "company-2024-q4-fine-tune",
      "layer": 1,
      "use-case": "If the query is about company information during 2024 Q4",
      "output-to": ["final-output", "company-2024-q4-edge-case"]
    },
    {
      "name": "company-2024-q4-edge-case",
      "layer": 2,
      "use-case": "If the Q4 response indicates an edge case needing specialized processing"
    },
    {
      "name": "final-output",
      "adapter": "output"
    },
    {
      "name": "output-to-ws",
      "if": "$OutputCustomKey",
      "adapter": "ws",
      "url": "ws://localhost:3000"
    }
  ]
}
```

## Real-Life Use Cases

### 1. Financial Reporting System

**Scenario:** Investment firm with quarter-specific knowledge and compliance requirements.

```
Query: "What were our Q3 2024 earnings?"
  → Router identifies Q3 2024 context
  → Q3 Fine-tuned model (trained on Q3 data) responds
  → Response includes accurate Q3 figures
```

| Handler | Training Data | Specialty |
|---------|--------------|-----------|
| Q3 Fine-Tune | Q3 2024 financials | Q3 earnings, guidance, events |
| Q4 Fine-Tune | Q4 2024 financials | Q4 results, year-end figures |
| Edge Case | Unusual scenarios | Restatements, corrections |

**Why temporal routing:** Financial data changes quarterly. Models trained on specific periods provide more accurate answers.

### 2. Regulatory Compliance

**Scenario:** Insurance company with evolving regulations.

```
Query: "What are the current coverage requirements for auto policies?"
  → Router identifies relevant regulatory period
  → Period-specific model applies correct regulations
  → Edge case handler catches regulatory exceptions
```

### 3. Product Knowledge Base

**Scenario:** Tech company with version-specific documentation.

| Handler | Coverage |
|---------|----------|
| v3.x Handler | Legacy product features |
| v4.x Handler | Current product features |
| Migration Edge Case | v3→v4 migration issues |

```json
{
  "name": "product-v3-handler",
  "use-case": "Questions about v3.x features and configuration"
},
{
  "name": "product-v4-handler",
  "use-case": "Questions about v4.x features and configuration"
},
{
  "name": "migration-specialist",
  "layer": 2,
  "use-case": "Complex migration scenarios between versions"
}
```

### 4. Legal Case Management

**Scenario:** Law firm with case-specific knowledge bases.

| Handler | Knowledge Base |
|---------|---------------|
| Active Cases | Current case files, recent filings |
| Archived Cases | Historical precedents, closed cases |
| Edge Case | Cross-jurisdictional issues |

### 5. Healthcare with Protocol Updates

**Scenario:** Hospital system with evolving treatment protocols.

```
Query: "What's the recommended dosage for Drug X?"
  → Router identifies relevant protocol version
  → Correct protocol handler responds
  → Alert sent if dose exceeds safety threshold
```

```json
{
  "name": "2024-protocols",
  "use-case": "Current 2024 treatment protocols"
},
{
  "name": "legacy-protocols",
  "use-case": "Historical protocols for reference"
},
{
  "name": "safety-alert",
  "if": "$AlertRequired",
  "adapter": "ws",
  "url": "ws://safety-dashboard:3000"
}
```

## WebSocket Integration

The WebSocket output enables real-time notifications:

### Configuration

```json
{
  "name": "output-to-ws",
  "if": "$OutputCustomKey",
  "adapter": "ws",
  "url": "ws://localhost:3000"
}
```

### Use Cases

| Scenario | WebSocket Alert |
|----------|----------------|
| High-priority queries | Dashboard notification |
| Compliance flags | Audit log stream |
| Edge cases detected | Specialist queue |
| Error conditions | Monitoring system |

### Triggering WebSocket Output

Set the header in your request:
```bash
curl -X POST http://localhost:8080/v1/chat/completions \
  -H "Content-Type: application/json" \
  -H "X-Output-Custom-Key: true" \
  -d '{"model":"llmnet","messages":[{"role":"user","content":"Q4 edge case query"}]}'
```

## Edge Case Pattern

The edge case handler demonstrates hierarchical escalation:

```
Normal Query → Handler → Output
Edge Case Query → Handler → Edge Case Processor → Output
```

### When to Use Edge Case Handlers

| Trigger | Example |
|---------|---------|
| Unusual input patterns | Very long queries, special characters |
| Low confidence responses | Handler uncertainty detected |
| Cross-boundary queries | Spans multiple time periods |
| Compliance triggers | Potential regulatory issues |

### Configuration Pattern

```json
{
  "name": "primary-handler",
  "layer": 1,
  "output-to": ["output", "edge-case-handler"]
},
{
  "name": "edge-case-handler",
  "layer": 2,
  "if": "$EdgeCaseFlag",
  "output-to": ["output"]
}
```

## Using Nemotron as Router

NVIDIA's Nemotron-Orchestrator-8B excels at:
- Intent classification
- Multi-step reasoning
- Cost-aware routing

### Why Nemotron?

| Feature | Benefit |
|---------|---------|
| Small size (8B) | Fast inference, low cost |
| Orchestration training | Optimized for routing decisions |
| Instruction following | Reliable node selection |

### Setup

```bash
# Run Nemotron via vLLM
python -m vllm.entrypoints.openai.api_server \
  --model nvidia/Nemotron-Orchestrator-8B \
  --port 44443
```

See [nemotron-router-8b.md](../nemotron-router-8b.md) for detailed setup.

## Extending the Pattern

### Add More Time Periods

```json
{
  "name": "company-2025-q1-fine-tune",
  "layer": 1,
  "use-case": "If the query is about company information during 2025 Q1"
}
```

### Add Domain-Specific Edge Cases

```json
{
  "name": "regulatory-edge-case",
  "layer": 2,
  "if": "$PREV_NODE == \"compliance-handler\"",
  "use-case": "Complex regulatory scenarios requiring legal review"
}
```

### Multiple WebSocket Endpoints

```json
{
  "name": "audit-ws",
  "if": "$AuditRequired",
  "adapter": "ws",
  "url": "ws://audit-system:3000"
},
{
  "name": "alert-ws",
  "if": "$HighPriority",
  "adapter": "ws",
  "url": "ws://alert-system:3001"
}
```

## Summary

The Nemotron Router pattern is ideal for:
- **Temporal knowledge:** Different models for different time periods
- **Hierarchical processing:** Normal path + edge case escalation
- **Real-time integration:** WebSocket alerts for monitoring
- **Enterprise compliance:** Audit trails and specialized handling

**Related guides:**
- [Conditional Routing](./conditional-routing.md) - System variables and conditions
- [Multi-Layer Pipeline](./multi-layer-pipeline.md) - Refinement layers
