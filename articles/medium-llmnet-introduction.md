# LLMNet: The Open-Source "Kubernetes of AI" Written in Pure Rust

*Route queries to the right model, every time. No Python. No complexity. Just config.*

---

If you've been experimenting with local LLMs on your DGX Spark, RTX workstation, or even a humble laptop with Ollama, you've probably hit the same wall I did: **one model isn't enough**.

Your coding assistant is great at Python but struggles with creative writing. Your fine-tuned customer service model knows your products inside-out but can't handle technical debugging. And that 70B parameter beast? Perfect for complex reasoning, but overkill (and slow) for simple questions.

The enterprise world solves this with expensive orchestration platforms. But what about the rest of us—the tinkerers, the local-first enthusiasts, the developers who want full control over their AI stack?

**Enter LLMNet.**

## What is LLMNet?

LLMNet is an open-source LLM pipeline orchestrator written in **pure Rust**. No Python runtime. No garbage collection pauses. No dependency hell. Just a single binary that turns a JSON config file into an intelligent, multi-model pipeline.

```
User Query → Router → [Expert A | Expert B | Expert C] → Refiner → Response
```

Think of it like a neural network, but each "neuron" is an entire LLM. The router analyzes incoming queries and dispatches them to specialized handlers. Those handlers can chain to refiners, validators, or even WebSocket endpoints for real-time alerts.

**And it all runs locally.**

## The Problem with Current Solutions

Let's talk about what's out there:

**LangChain/LlamaIndex**: Powerful, but Python-heavy. Great for prototyping, challenging for production. Dependency management becomes a full-time job.

**Commercial orchestrators** (AWS Bedrock, Azure AI Studio): Excellent if you're already in their ecosystem and have the budget. Not so great for local-first development or privacy-sensitive workloads.

**DIY scripts**: We've all written them. The "just route to model A or B based on keywords" approach. It works until it doesn't.

LLMNet takes a different approach: **configuration over code**.

```json
{
  "models": {
    "router": { "url": "http://localhost:11434" },
    "code-expert": { "url": "http://localhost:11435" },
    "writing-expert": { "url": "http://localhost:11436" }
  },
  "architecture": [
    {
      "name": "router",
      "layer": 0,
      "output-to": [1]
    },
    {
      "name": "code-handler",
      "layer": 1,
      "use-case": "Programming, debugging, technical questions",
      "output-to": ["output"]
    },
    {
      "name": "writing-handler",
      "layer": 1,
      "use-case": "Creative writing, emails, documentation",
      "output-to": ["output"]
    }
  ]
}
```

That's it. No code. Run `llmnet config.json` and you have an intelligent routing pipeline serving on port 8080 with a standard OpenAI-compatible API.

## Why Rust?

I get this question a lot. The AI/ML world lives in Python. Why swim upstream?

**1. Single Binary Deployment**

```bash
cargo build --release
scp target/release/llmnet your-server:~/
ssh your-server './llmnet config.json'
```

No virtual environments. No pip install. No "works on my machine." Just copy and run.

**2. Memory Safety Without Garbage Collection**

When you're running multiple LLM inference servers, the orchestration layer shouldn't be fighting for resources. Rust's ownership model means predictable memory usage and no GC pauses during critical routing decisions.

**3. Fearless Concurrency**

LLMNet handles hundreds of concurrent requests without breaking a sweat. Rust's async runtime (Tokio) combined with its thread-safety guarantees means we can maximize throughput on your hardware.

**4. Long-Term Maintainability**

The Rust compiler catches entire categories of bugs at compile time. When we add features, we're confident we haven't broken existing functionality. The type system is our test suite's best friend.

## For the DGX Spark and Local Enthusiasts

If you're running local inference—whether on NVIDIA's DGX Spark, a multi-GPU workstation, or even Apple Silicon with MLX—LLMNet is built for you.

### The Local LLM Stack

```
┌─────────────────────────────────────────────────┐
│                   LLMNet                         │
│         (Orchestration & Routing)                │
└─────────────────────────────────────────────────┘
                      │
        ┌─────────────┼─────────────┐
        ▼             ▼             ▼
   ┌─────────┐   ┌─────────┐   ┌─────────┐
   │ Ollama  │   │  vLLM   │   │  TGI    │
   │(Llama 8B)│   │(Qwen 72B)│   │(Mistral)│
   └─────────┘   └─────────┘   └─────────┘
```

Each inference server runs independently. LLMNet sits on top, routing queries to the right model based on:

- **LLM-based intent detection**: The router model analyzes the query
- **Rule-based conditions**: Route by word count, input length, or custom headers
- **Cost optimization**: Simple queries go to fast models; complex ones get the big guns

### Example: DGX Spark Multi-Model Setup

Say you have a DGX Spark with 128GB of unified memory. You could run:

- **Nemotron 8B** as router (fast, good at classification)
- **Llama 3.3 70B** for complex reasoning
- **CodeLlama 34B** for programming tasks
- **Mistral 7B** for quick, simple responses

```json
{
  "architecture": [
    {
      "name": "router",
      "layer": 0,
      "extra-options": { "model_override": "nemotron:8b" }
    },
    {
      "name": "simple-handler",
      "layer": 1,
      "if": "$WORD_COUNT < 20",
      "use-case": "Quick questions, greetings, simple lookups"
    },
    {
      "name": "code-handler",
      "layer": 1,
      "use-case": "Programming, debugging, code review"
    },
    {
      "name": "reasoning-handler",
      "layer": 1,
      "use-case": "Complex analysis, research, detailed explanations"
    }
  ]
}
```

The `if` condition uses **system variables** that LLMNet tracks automatically:

| Variable | Description |
|----------|-------------|
| `$WORD_COUNT` | Words in the input |
| `$INPUT_LENGTH` | Character count |
| `$PREV_NODE` | Previous node in the chain |
| `$HOP_COUNT` | Number of hops so far |

This means you can build sophisticated routing logic without writing a single line of code.

## The Vision: Kubernetes of AI

We're just getting started. The current version handles orchestration beautifully, but the roadmap is ambitious:

### Coming Soon

**Model Auto-Scaling**: Spin up additional inference containers based on queue depth. When traffic spikes, LLMNet will automatically provision more capacity.

**Declarative Model Deployment**: Define your desired state (3 replicas of Llama 70B, 1 replica of CodeLlama), and LLMNet makes it happen.

**Health Checks & Failover**: If an inference server goes down, traffic automatically reroutes to healthy nodes.

**Resource Quotas**: Prevent any single user or pipeline from monopolizing your GPU cluster.

**Multi-Cluster Federation**: Connect multiple LLMNet deployments across machines or data centers.

The goal? **What Kubernetes did for containers, LLMNet will do for AI inference.**

Imagine:

```yaml
apiVersion: llmnet/v1
kind: Pipeline
metadata:
  name: customer-service
spec:
  replicas: 3
  models:
    - name: router
      model: nemotron-8b
      resources:
        gpu: 1
    - name: handler
      model: llama-70b
      resources:
        gpu: 4
      minReplicas: 2
      maxReplicas: 10
```

That's the future we're building toward.

## Getting Started Today

LLMNet is open source and ready to use:

```bash
# Clone and build
git clone https://github.com/Avarok-Cybersecurity/llmnet.git
cd llmnet
cargo build --release

# Validate your config
./target/release/llmnet --dry-run examples/basic-chatbot.json

# Run it
./target/release/llmnet examples/openrouter-pipeline.json
```

The repository includes several example configurations:

- **basic-chatbot.json**: Simple single-model proxy
- **dual-expert.json**: Sales vs support routing
- **multi-layer-pipeline.json**: Handlers + refinement layer
- **conditional-routing.json**: Rule-based routing with system variables
- **openrouter-pipeline.json**: Cloud models via OpenRouter

Each example has [detailed documentation](https://github.com/Avarok-Cybersecurity/llmnet/tree/master/docs/examples) explaining real-world use cases.

## Join the Movement

The AI world is consolidating around a few massive players. But the best software has always come from communities of developers who believe in open, local-first, privacy-respecting solutions.

LLMNet is our contribution to that future.

**If you're building local AI pipelines, we'd love to hear from you.** Open an issue, submit a PR, or just star the repo to follow along.

The future is small models, routed intelligently. **Welcome to LLMNet.**

---

*LLMNet is open source under the MIT license. Star us on [GitHub](https://github.com/Avarok-Cybersecurity/llmnet).*
