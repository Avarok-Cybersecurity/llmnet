# Edge Device Deployment

**llmnet excels as an orchestrator across heterogeneous edge devices.** Whether you're deploying to NVIDIA Jetson devices for GPU-accelerated inference or Raspberry Pi for lightweight CPU-based models, llmnet provides a unified interface to manage your LLM pipelines.

## Why Edge Deployment?

- **Low Latency**: Local inference eliminates network round-trips to cloud APIs
- **Privacy**: Keep sensitive data on-device without external transmission
- **Cost Efficiency**: No per-token API costs after initial hardware investment
- **Offline Capability**: Continue operating without internet connectivity
- **Customization**: Fine-tune models for specific use cases

## Supported Platforms

| Device | Memory | GPU | Max Model Size | Recommended Runner |
|--------|--------|-----|----------------|-------------------|
| Jetson Orin Nano | 8GB | CUDA 8.7 | 7B (INT4) | tensorrt-llm |
| Jetson Orin NX | 16GB | CUDA 8.7 | 13B (INT8) | tensorrt-llm |
| Jetson AGX Orin | 32-64GB | CUDA 8.7 | 34B+ | tensorrt-llm, vllm |
| Raspberry Pi 5 | 8GB | None | 3B (Q4) | llama-cpp |

## Quick Comparison

### NVIDIA Jetson (Orin Series)

Best for: High-performance edge inference with GPU acceleration

- **Pros**: Excellent performance-per-watt, TensorRT optimization, CUDA support
- **Cons**: Higher cost, requires NVIDIA ecosystem
- **Use Cases**: Robotics, autonomous systems, industrial AI, smart cameras

### Raspberry Pi 5

Best for: Lightweight, low-cost deployments

- **Pros**: Affordable, low power consumption, large community
- **Cons**: CPU-only inference, limited to small models
- **Use Cases**: Home automation, education, IoT gateways, prototyping

## Getting Started

1. **[NVIDIA Jetson Setup](./jetson.md)** - Deploy on Jetson Orin devices
2. **[Raspberry Pi Setup](./raspberry-pi.md)** - Deploy on Raspberry Pi
3. **[Model Selection Guide](./model-selection.md)** - Choose the right model for your device

## Memory Validation

llmnet includes built-in validation to check if your model configuration fits your device:

```bash
# Validate a composition file against a specific device
llmnet validate --device jetson-orin-nano composition.jsonc
```

The validator checks:
- Model size vs. device memory limits
- Quantization compatibility
- Runner support (TensorRT-LLM, vLLM, llama.cpp)
- Context length recommendations
- Batch size constraints

## Architecture Example

Here's how llmnet can orchestrate a multi-device edge deployment:

```
                    ┌─────────────────────┐
                    │   Control Plane     │
                    │    (llmnet serve)   │
                    └──────────┬──────────┘
                               │
           ┌───────────────────┼───────────────────┐
           │                   │                   │
    ┌──────▼──────┐    ┌───────▼───────┐   ┌──────▼──────┐
    │ Jetson Nano │    │  Jetson NX    │   │  Pi 5       │
    │ (Router 3B) │    │  (Expert 13B) │   │  (Tool 1B)  │
    │ tensorrt-llm│    │  tensorrt-llm │   │  llama-cpp  │
    └─────────────┘    └───────────────┘   └─────────────┘
```

Each device runs as a worker node, with llmnet routing requests based on task requirements.
