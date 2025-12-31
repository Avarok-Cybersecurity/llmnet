# Model Selection Guide

Choosing the right model for your edge device is crucial for balancing performance, quality, and resource constraints.

## Decision Framework

```
                    ┌─────────────────────────┐
                    │  What's your memory?    │
                    └───────────┬─────────────┘
                                │
        ┌───────────────────────┼───────────────────────┐
        │                       │                       │
   ≤8GB (Pi/Nano)         16GB (NX)            32GB+ (AGX)
        │                       │                       │
   ┌────▼────┐            ┌─────▼─────┐          ┌──────▼──────┐
   │ 0.5-3B  │            │  3-13B    │          │   8-70B     │
   │  Q4/Q8  │            │  INT8     │          │  FP16/INT4  │
   └─────────┘            └───────────┘          └─────────────┘
```

## Memory Calculator

Use this formula to estimate if a model fits:

```
Memory (GB) ≈ Parameters (B) × Bytes per param × 1.3 (overhead)
```

| Quantization | Bytes/Param | Example: 7B Model |
|--------------|-------------|-------------------|
| FP32 | 4.0 | 36.4 GB |
| FP16/BF16 | 2.0 | 18.2 GB |
| INT8 | 1.0 | 9.1 GB |
| INT4 | 0.5 | 4.55 GB |

## Recommended Models by Device

### Jetson Orin Nano (8GB)

**Best Choice: Llama 3.2 3B with INT4**

| Model | Quant | Size | Quality | Speed |
|-------|-------|------|---------|-------|
| Llama 3.2 3B | INT4 | 2GB | Good | Fast |
| Phi-3 Mini 3.8B | INT4 | 2.5GB | Very Good | Medium |
| Qwen2.5 3B | INT4 | 2GB | Good | Fast |
| TinyLlama 1.1B | FP16 | 2.2GB | Basic | Very Fast |

**Composition example:**
```jsonc
{
  "models": {
    "main": {
      "runner": "tensorrt-llm",
      "source": "meta-llama/Llama-3.2-3B-Instruct",
      "parameters": {
        "quantization": "int4_awq",
        "max_input_len": 1024,
        "max_output_len": 256
      }
    }
  }
}
```

### Jetson Orin NX (16GB)

**Best Choice: Llama 3.1 8B with INT8**

| Model | Quant | Size | Quality | Speed |
|-------|-------|------|---------|-------|
| Llama 3.1 8B | INT8 | 8GB | Excellent | Medium |
| Mistral 7B v0.3 | INT8 | 7GB | Excellent | Medium |
| CodeLlama 7B | INT8 | 7GB | Excellent (code) | Medium |
| Llama 3.2 3B | FP16 | 6GB | Good | Fast |

**Multi-model example:**
```jsonc
{
  "models": {
    "router": {
      "runner": "tensorrt-llm",
      "source": "meta-llama/Llama-3.2-3B-Instruct",
      "parameters": { "quantization": "fp16" }
    },
    "expert": {
      "runner": "tensorrt-llm",
      "source": "meta-llama/Llama-3.1-8B-Instruct",
      "parameters": { "quantization": "int8" }
    }
  }
}
```

### Jetson AGX Orin (32-64GB)

**Best Choice: Llama 3.1 70B with INT4 (64GB) or 8B with FP16 (32GB)**

| Model | Quant | Size | Quality | Speed |
|-------|-------|------|---------|-------|
| Llama 3.1 70B | INT4 | 35GB | State-of-art | Slow |
| Mixtral 8x7B | INT8 | 26GB | Excellent | Medium |
| DeepSeek Coder 33B | INT8 | 33GB | Excellent (code) | Slow |
| Llama 3.1 8B | FP16 | 16GB | Excellent | Fast |

### Raspberry Pi 5 (8GB)

**Best Choice: TinyLlama 1.1B or Qwen2.5 0.5B**

| Model | Quant | Size | Quality | Speed |
|-------|-------|------|---------|-------|
| Qwen2.5 0.5B | Q4_K_M | 0.4GB | Basic | Very Fast |
| TinyLlama 1.1B | Q4_K_M | 0.7GB | Basic | Fast |
| SmolLM 1.7B | Q4_K_M | 1.2GB | Good | Medium |
| Phi-3 Mini 3.8B | Q4_K_M | 2.5GB | Very Good | Slow |

## Use Case Recommendations

### Chatbot / Assistant

- **Budget**: TinyLlama 1.1B, Qwen2.5 3B
- **Balanced**: Llama 3.1 8B, Mistral 7B
- **Quality**: Llama 3.1 70B, GPT-4 class

### Code Generation

- **Budget**: DeepSeek Coder 1.3B
- **Balanced**: CodeLlama 7B, DeepSeek Coder 6.7B
- **Quality**: DeepSeek Coder 33B, CodeLlama 34B

### Routing / Classification

- **Any device**: TinyLlama 1.1B, Qwen2.5 0.5B
- Small, fast models are ideal for routing decisions

### RAG / Document Q&A

- Prioritize context length over model size
- Llama 3.2 3B with 8K context often beats 7B with 2K context

## Quantization Selection

### When to use INT4

- Memory-constrained devices (Orin Nano, Pi 5)
- Large models that wouldn't otherwise fit
- Non-critical applications where some quality loss is acceptable

### When to use INT8

- Balanced memory/quality trade-off
- Production deployments
- Code generation (quality matters)

### When to use FP16

- Quality-critical applications
- Sufficient memory available
- Small models (3B and under)

## Validation

Use llmnet's built-in validation to check compatibility:

```bash
# Validate against specific device
llmnet validate --device jetson-orin-nano composition.jsonc

# Sample output:
# [main-model]
#   WARN [MEMORY_PRESSURE]: Model will use 85% of available memory
#     -> Reduce batch size or context length for stable operation
```

## Performance Benchmarks

Approximate tokens/second for generation:

| Device | 1B Q4 | 3B Q4 | 7B INT8 | 13B INT8 |
|--------|-------|-------|---------|----------|
| Pi 5 | 12 t/s | 4 t/s | N/A | N/A |
| Orin Nano | 50 t/s | 25 t/s | 8 t/s | N/A |
| Orin NX | 80 t/s | 45 t/s | 20 t/s | 10 t/s |
| AGX Orin | 120 t/s | 70 t/s | 40 t/s | 25 t/s |

*Benchmarks are approximate and vary based on batch size, context length, and specific model architecture.*

## Model Sources

### HuggingFace

Most models are available from HuggingFace. Use the official repositories:

- `meta-llama/Llama-3.2-3B-Instruct`
- `mistralai/Mistral-7B-Instruct-v0.3`
- `Qwen/Qwen2.5-3B-Instruct`

### GGUF Models (for llama.cpp)

For Raspberry Pi and CPU inference, use GGUF quantized models:

- [TheBloke](https://huggingface.co/TheBloke) - Extensive GGUF collection
- Official model repos often include GGUF versions

### TensorRT-LLM Models

For Jetson devices, you can use:
- HuggingFace models directly (TensorRT-LLM compiles them)
- Pre-converted TensorRT engines from NVIDIA NGC
