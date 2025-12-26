# Multi-Layer Pipeline (1-2-1 with Refinement)

> **Config file:** [`examples/multi-layer-pipeline.json`](../../examples/multi-layer-pipeline.json)

Add a refinement layer to polish responses before returning to users.

## Topology

```
                    ┌─────────────────────┐
                    │  Technical Handler  │
┌────────────┐     ╱└─────────────────────┘╲     ┌────────────┐     ┌────────┐
│   Router   │────<                        >────▶│  Refiner   │────▶│ Output │
└────────────┘     ╲┌─────────────────────┐╱     └────────────┘     └────────┘
                    │  Creative Handler   │
                    └─────────────────────┘

     Layer 0              Layer 1                   Layer 2          Output
```

**Layers:** 3 (router → handlers → refiner)
**Nodes:** 1 router + 2 handlers + 1 refiner + output
**Routing:** Intent-based, then all responses pass through refinement

## How It Works

1. Router analyzes query and selects handler
2. Handler generates initial response
3. **Refiner polishes the response** for clarity and quality
4. Refined response returns to user

The refinement layer adds:
- Consistent tone and formatting
- Error checking and correction
- Quality assurance

## Configuration

```json
{
  "architecture": [
    {
      "name": "router",
      "layer": 0,
      "output-to": [1]
    },
    {
      "name": "technical-handler",
      "layer": 1,
      "use-case": "Technical questions, coding, math",
      "output-to": [2]
    },
    {
      "name": "creative-handler",
      "layer": 1,
      "use-case": "Creative writing, storytelling",
      "output-to": [2]
    },
    {
      "name": "response-refiner",
      "layer": 2,
      "use-case": "Polish and refine responses for clarity",
      "output-to": ["output"]
    },
    { "name": "output", "adapter": "output" }
  ]
}
```

## Real-Life Use Cases

### 1. Enterprise Customer Service

**Scenario:** Bank requiring consistent, professional responses across all channels.

```
Query → Router → Domain Expert → Brand Voice Refiner → Response
```

| Layer | Role | Function |
|-------|------|----------|
| Router | Intent classifier | Route to correct department |
| Handlers | Domain experts | Investment, banking, loans |
| Refiner | Brand voice | Ensure professional, compliant language |

**Refiner prompt context:**
```
"Ensure responses follow brand guidelines: professional tone,
avoid jargon, include relevant disclaimers, suggest next steps"
```

### 2. Technical Documentation

**Scenario:** API documentation with consistent formatting.

```
Query → Router → Code/Concept Expert → Documentation Formatter → Response
```

| Layer | Role | Function |
|-------|------|----------|
| Router | Query classifier | Code vs concept question |
| Handlers | Technical experts | Generate accurate content |
| Refiner | Doc formatter | Apply consistent markdown, add examples |

**Why it works:** Experts focus on accuracy; refiner handles presentation.

### 3. Multi-Language Support

**Scenario:** Global company needing localized responses.

```
Query → Router → Expert → Localizer/Translator → Response
```

| Layer | Role | Function |
|-------|------|----------|
| Router | Route by topic | Select appropriate expert |
| Handlers | Domain experts | Answer in base language |
| Refiner | Localizer | Translate and culturally adapt |

### 4. Educational Content

**Scenario:** Learning platform adapting content to student level.

```
Query → Router → Subject Expert → Level Adapter → Response
```

| Layer | Role | Function |
|-------|------|----------|
| Router | Subject classifier | Math, science, history |
| Handlers | Subject experts | Generate detailed answers |
| Refiner | Level adapter | Simplify for student's grade level |

### 5. Legal Document Review

**Scenario:** Law firm reviewing and standardizing document responses.

```
Query → Router → Practice Area Expert → Compliance Reviewer → Response
```

| Layer | Role | Function |
|-------|------|----------|
| Router | Practice area | Corporate, litigation, IP |
| Handlers | Legal experts | Draft responses |
| Refiner | Compliance | Check for regulatory issues, add disclaimers |

### 6. Healthcare Communication

**Scenario:** Hospital patient communication requiring medical accuracy and accessibility.

```
Query → Router → Clinical Expert → Patient Communication Refiner → Response
```

| Layer | Role | Function |
|-------|------|----------|
| Router | Query type | Clinical vs administrative |
| Handlers | Medical experts | Provide accurate medical info |
| Refiner | Communication specialist | Make accessible, add caveats |

### 7. Content Marketing

**Scenario:** Agency producing on-brand content at scale.

```
Brief → Router → Content Creator → Brand Editor → Final Content
```

| Layer | Role | Function |
|-------|------|----------|
| Router | Content type | Blog, social, email |
| Handlers | Content specialists | Draft initial content |
| Refiner | Brand editor | Apply style guide, optimize for channel |

## Benefits of the Refinement Layer

### Quality Assurance
- Catch and fix errors from initial response
- Ensure completeness and accuracy
- Add missing context or caveats

### Consistency
- Uniform tone across all handlers
- Standardized formatting
- Brand voice alignment

### Compliance
- Add required disclaimers
- Remove prohibited content
- Ensure regulatory compliance

### Cost Efficiency
- Use smaller models for refinement
- Handlers can be larger/specialized
- Refiner catches issues before user sees them

## Model Selection

| Layer | Model Size | Reasoning |
|-------|-----------|-----------|
| Router | Small (7-8B) | Fast classification |
| Handlers | Large (70B+) | High quality domain expertise |
| Refiner | Medium (27B) | Good editing, fast enough |

## Configuration Options

### Conditional Refinement

Only refine long responses:
```json
{
  "name": "response-refiner",
  "layer": 2,
  "if": "$WORD_COUNT > 100",
  "output-to": ["output"]
}
```

### Multiple Refinement Paths

Different refiners for different handlers:
```json
{
  "name": "technical-refiner",
  "layer": 2,
  "if": "$PREV_NODE == \"technical-handler\"",
  "use-case": "Format code, add examples"
},
{
  "name": "creative-refiner",
  "layer": 2,
  "if": "$PREV_NODE == \"creative-handler\"",
  "use-case": "Polish prose, enhance narrative"
}
```

See [Conditional Routing](./conditional-routing.md) for more on conditions.

## Testing

```bash
# Technical query (goes through technical-handler → refiner)
curl -X POST http://localhost:8080/v1/chat/completions \
  -d '{"model":"llmnet","messages":[{"role":"user","content":"Explain recursion"}]}'

# Creative query (goes through creative-handler → refiner)
curl -X POST http://localhost:8080/v1/chat/completions \
  -d '{"model":"llmnet","messages":[{"role":"user","content":"Write a poem about AI"}]}'
```

**Next step:** [Conditional Routing](./conditional-routing.md) for rule-based decisions.
