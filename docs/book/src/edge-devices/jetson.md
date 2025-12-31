# NVIDIA Jetson Deployment

This guide covers deploying llmnet on NVIDIA Jetson devices, including Orin Nano, Orin NX, and AGX Orin.

## Prerequisites

- NVIDIA Jetson device with JetPack 6.0+ installed
- At least 8GB of unified memory (Orin Nano minimum)
- SD card or NVMe storage with 32GB+ free space

## Installation

### 1. Install llmnet

```bash
# Download the ARM64 binary
curl -L -o llmnet https://github.com/your-org/llmnet/releases/latest/download/llmnet-aarch64-unknown-linux-gnu
chmod +x llmnet
sudo mv llmnet /usr/local/bin/
```

### 2. Install TensorRT-LLM (Recommended)

TensorRT-LLM provides the best performance on Jetson devices:

```bash
# Using NVIDIA's L4T TensorRT container
docker pull nvcr.io/nvidia/l4t-tensorrt:r36.4.0-runtime
```

Or install natively:
```bash
pip3 install tensorrt_llm --extra-index-url https://pypi.nvidia.com
```

### 3. Alternative: Install llama.cpp

For CPU-based inference (useful for testing):

```bash
git clone https://github.com/ggerganov/llama.cpp
cd llama.cpp
make -j$(nproc)
sudo cp llama-server /usr/local/bin/
```

## Device-Specific Configurations

### Jetson Orin Nano (8GB)

The Orin Nano is memory-constrained. Use INT4 quantization:

```jsonc
{
  "models": {
    "router": {
      "runner": "tensorrt-llm",
      "source": "meta-llama/Llama-3.2-3B-Instruct",
      "parameters": {
        "max_batch_size": 4,
        "max_input_len": 1024,
        "max_output_len": 256,
        "quantization": "int4_awq",
        "kv_cache_free_gpu_mem_fraction": 0.85
      }
    }
  },
  "architecture": [
    {
      "name": "router",
      "layer": 0,
      "model": "router",
      "adapter": "openai-api",
      "output-to": ["output"]
    },
    {
      "name": "output",
      "adapter": "output"
    }
  ]
}
```

**Recommended models for Orin Nano:**
- Llama 3.2 3B (INT4)
- Phi-3 Mini 3.8B (INT4)
- Qwen2.5 3B (INT4)
- TinyLlama 1.1B (INT8 or FP16)

### Jetson Orin NX (16GB)

With more memory, you can run larger models:

```jsonc
{
  "models": {
    "expert": {
      "runner": "tensorrt-llm",
      "source": "meta-llama/Llama-3.1-8B-Instruct",
      "parameters": {
        "max_batch_size": 8,
        "max_input_len": 2048,
        "max_output_len": 512,
        "quantization": "int8",
        "kv_cache_free_gpu_mem_fraction": 0.80
      }
    }
  }
}
```

**Recommended models for Orin NX:**
- Llama 3.1 8B (INT8)
- Mistral 7B (INT8)
- Llama 3.2 3B (FP16)
- CodeLlama 7B (INT8)

### Jetson AGX Orin (32-64GB)

The AGX Orin can handle larger models:

```jsonc
{
  "models": {
    "large-expert": {
      "runner": "tensorrt-llm",
      "source": "meta-llama/Llama-3.1-70B-Instruct",
      "parameters": {
        "max_batch_size": 16,
        "max_input_len": 4096,
        "max_output_len": 1024,
        "quantization": "int4_awq",
        "tp_size": 1
      }
    }
  }
}
```

**Recommended models for AGX Orin:**
- Llama 3.1 70B (INT4) - 64GB variant
- Llama 3.1 8B (FP16)
- Mixtral 8x7B (INT8)
- DeepSeek Coder 33B (INT8)

## Running as a Worker Node

Start llmnet as a worker that connects to a control plane:

```bash
# Start worker on Jetson
llmnet worker \
  --control-plane http://192.168.1.100:8080 \
  --host 0.0.0.0 \
  --port 8081 \
  --composition edge-router.jsonc
```

## Performance Tuning

### Power Modes

Set the Jetson to maximum performance mode:

```bash
# Orin Nano/NX
sudo nvpmodel -m 0
sudo jetson_clocks

# Check current mode
nvpmodel -q
```

### Memory Optimization

1. **Disable GUI** to free memory:
   ```bash
   sudo systemctl set-default multi-user.target
   sudo reboot
   ```

2. **Increase swap** for loading larger models:
   ```bash
   sudo fallocate -l 8G /swapfile
   sudo chmod 600 /swapfile
   sudo mkswap /swapfile
   sudo swapon /swapfile
   ```

3. **Use NVMe storage** for model caching to avoid SD card bottlenecks.

### Batch Size Guidelines

| Device | Recommended Batch Size |
|--------|----------------------|
| Orin Nano | 1-4 |
| Orin NX | 4-8 |
| AGX Orin | 8-32 |

## Troubleshooting

### Out of Memory Errors

```
RuntimeError: CUDA out of memory
```

**Solutions:**
1. Reduce `max_batch_size`
2. Lower `max_input_len` and `max_output_len`
3. Use stronger quantization (INT4 instead of INT8)
4. Increase `kv_cache_free_gpu_mem_fraction`

### Model Loading Slow

First-time model loading includes TensorRT engine compilation:

```
Building TensorRT engine... (this may take several minutes)
```

This is normal. Subsequent loads will be faster as the engine is cached.

### Docker GPU Access

Ensure Docker has GPU access:

```bash
docker run --rm --gpus all nvidia/cuda:12.0-base nvidia-smi
```

If this fails, install nvidia-container-toolkit:
```bash
sudo apt-get install nvidia-container-toolkit
sudo systemctl restart docker
```

## Example: Multi-Device Pipeline

Deploy a router on Orin Nano and expert on Orin NX:

**Orin Nano (Router):**
```bash
llmnet serve --host 0.0.0.0 --port 8080 router.jsonc
```

**Orin NX (Expert Worker):**
```bash
llmnet worker \
  --control-plane http://orin-nano:8080 \
  --port 8081 \
  expert.jsonc
```

This creates a distributed pipeline where the Nano routes requests to the NX for complex reasoning.
