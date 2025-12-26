# Dual Expert Router (1-2-1 Topology)

> **Config file:** [`examples/dual-expert.json`](../../examples/dual-expert.json)

Route queries to specialized models based on intent detection.

## Topology

```
                    ┌──────────────────┐
                    │  Sales Expert    │
┌────────┐         ╱└──────────────────┘╲        ┌──────────┐
│ Router │────────<                      >──────▶│  Output  │
└────────┘         ╲┌──────────────────┐╱        └──────────┘
                    │ Support Expert   │
                    └──────────────────┘
```

**Layers:** 2 (router + handlers)
**Nodes:** 1 router + 2 experts + output
**Routing:** Intent-based selection

## How It Works

1. User query arrives at the router (layer 0)
2. Router analyzes the query and selects the best expert
3. Selected expert processes the query
4. Response returns to user

The router receives a prompt like:
```
Here is the user prompt: "What's my order status?"

Based on the prompt, please choose from one of these models:
[
  {"name": "sales-expert", "use-case": "Product questions, pricing..."},
  {"name": "support-expert", "use-case": "Order status, shipping..."}
]
```

## Configuration

```json
{
  "models": {
    "router": { "url": "http://localhost:8000" },
    "sales-llm": { "url": "http://localhost:8001" },
    "support-llm": { "url": "http://localhost:8002" }
  },
  "architecture": [
    {
      "name": "router",
      "layer": 0,
      "model": "router",
      "output-to": [1]
    },
    {
      "name": "sales-expert",
      "layer": 1,
      "model": "sales-llm",
      "use-case": "Product questions, pricing, recommendations",
      "output-to": ["output"]
    },
    {
      "name": "support-expert",
      "layer": 1,
      "model": "support-llm",
      "use-case": "Order status, shipping issues, returns",
      "output-to": ["output"]
    },
    { "name": "output", "adapter": "output" }
  ]
}
```

## Real-Life Use Cases

### 1. E-Commerce Customer Service

**Scenario:** Online retailer with distinct sales and support functions.

| Expert | Handles |
|--------|---------|
| Sales | Product info, recommendations, pricing, availability |
| Support | Orders, shipping, returns, complaints |

```json
{
  "name": "sales-expert",
  "use-case": "Product questions, recommendations, pricing, availability, comparisons"
},
{
  "name": "support-expert",
  "use-case": "Order tracking, shipping delays, returns, refunds, account issues"
}
```

**Why it works:** Sales queries benefit from a model trained on product catalogs. Support queries need access to order systems and policies.

### 2. Healthcare Triage

**Scenario:** Medical information system routing between clinical and administrative.

| Expert | Handles |
|--------|---------|
| Clinical | Symptoms, medications, conditions, treatment info |
| Administrative | Appointments, insurance, billing, records |

```json
{
  "name": "clinical-expert",
  "use-case": "Medical symptoms, medications, conditions, treatments, drug interactions"
},
{
  "name": "admin-expert",
  "use-case": "Appointments, insurance coverage, billing questions, medical records"
}
```

**Why it works:** Clinical queries require medical knowledge; administrative queries need scheduling/billing context.

### 3. Educational Platform

**Scenario:** Learning platform with subject-specific tutors.

| Expert | Handles |
|--------|---------|
| STEM Tutor | Math, physics, chemistry, programming |
| Humanities Tutor | History, literature, philosophy, writing |

```json
{
  "name": "stem-tutor",
  "use-case": "Mathematics, physics, chemistry, biology, programming, engineering"
},
{
  "name": "humanities-tutor",
  "use-case": "History, literature, philosophy, languages, creative writing, arts"
}
```

**Why it works:** STEM requires precise, structured responses. Humanities benefits from nuanced, contextual discussion.

### 4. Legal Services

**Scenario:** Law firm automating initial client inquiries.

| Expert | Handles |
|--------|---------|
| Corporate | Business law, contracts, mergers, compliance |
| Personal | Family law, estate planning, personal injury |

```json
{
  "name": "corporate-expert",
  "use-case": "Business formation, contracts, mergers, intellectual property, compliance"
},
{
  "name": "personal-expert",
  "use-case": "Divorce, custody, wills, personal injury, real estate transactions"
}
```

### 5. IT Helpdesk

**Scenario:** Tech support with hardware vs software specialists.

| Expert | Handles |
|--------|---------|
| Hardware | Device issues, peripherals, physical setup |
| Software | Applications, OS issues, configuration |

```json
{
  "name": "hardware-expert",
  "use-case": "Computer hardware, printers, monitors, cables, physical setup"
},
{
  "name": "software-expert",
  "use-case": "Application errors, OS configuration, software installation, updates"
}
```

### 6. Financial Advisory

**Scenario:** Bank routing between investment and banking queries.

| Expert | Handles |
|--------|---------|
| Investment | Stocks, funds, retirement, portfolio |
| Banking | Accounts, loans, credit cards, transfers |

```json
{
  "name": "investment-advisor",
  "use-case": "Stocks, bonds, mutual funds, retirement accounts, portfolio management"
},
{
  "name": "banking-specialist",
  "use-case": "Checking accounts, loans, credit cards, wire transfers, interest rates"
}
```

## Choosing Your Router Model

The router doesn't need to be powerful—just good at classification:

| Router Choice | Best For |
|--------------|----------|
| Small/Fast model (Gemma, Phi) | High volume, simple categorization |
| Medium model (Llama 8B) | Nuanced intent detection |
| Specialized classifier | Production with tight latency requirements |

## Cost Optimization

This topology enables intelligent cost management:

```
Cheap router (Gemma 2B) → Analyzes intent
     ↓
Expensive expert (GPT-4) → Only when needed
     or
Cheap expert (Llama 8B) → For simpler queries
```

## When to Use This Topology

- You have distinct query categories
- Different experts provide better answers for different domains
- You want to optimize cost by using appropriate models
- You need to scale different functions independently

## Extending the Pattern

Add more experts as needed:

```json
{
  "name": "billing-expert",
  "layer": 1,
  "use-case": "Billing, payments, invoices, pricing"
}
```

**Next step:** [Multi-Layer Pipeline](./multi-layer-pipeline.md) for processing chains.
