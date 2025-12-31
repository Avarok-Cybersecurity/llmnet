# Raspberry Pi Deployment

This guide covers deploying llmnet on Raspberry Pi 5 for lightweight, CPU-based LLM inference.

## Overview

The Raspberry Pi 5 (8GB) can run small language models (1-3B parameters) using llama.cpp. While not as fast as GPU-based inference, it's excellent for:

- Home automation assistants
- Educational projects
- IoT gateways
- Offline chatbots
- Prototype development

## Hardware Requirements

- **Raspberry Pi 5** with 8GB RAM (recommended)
- **Active cooling** (fan or heatsink) - essential for sustained inference
- **NVMe SSD** via PCIe HAT (recommended) or high-quality SD card
- **Power supply**: Official 27W USB-C adapter

> **Note:** Raspberry Pi 4 (8GB) is also supported but significantly slower. Pi 3 and earlier are not recommended.

## Installation

### 1. Install llmnet

```bash
# Download ARM64 binary
curl -L -o llmnet https://github.com/your-org/llmnet/releases/latest/download/llmnet-aarch64-unknown-linux-gnu
chmod +x llmnet
sudo mv llmnet /usr/local/bin/

# Verify installation
llmnet --version
```

### 2. Install llama.cpp

Build llama.cpp optimized for ARM NEON:

```bash
# Install build dependencies
sudo apt update
sudo apt install -y build-essential cmake

# Clone and build
git clone https://github.com/ggerganov/llama.cpp
cd llama.cpp
make -j4 LLAMA_NATIVE=on

# Install
sudo cp llama-server /usr/local/bin/
```

### 3. Download a Model

Download a quantized GGUF model:

```bash
# Create models directory
mkdir -p ~/.cache/llmnet/models
cd ~/.cache/llmnet/models

# Download TinyLlama 1.1B (recommended for Pi 5)
wget https://huggingface.co/TheBloke/TinyLlama-1.1B-Chat-v1.0-GGUF/resolve/main/tinyllama-1.1b-chat-v1.0.Q4_K_M.gguf

# Or download Phi-3 Mini (good quality, requires 8GB)
wget https://huggingface.co/microsoft/Phi-3-mini-4k-instruct-gguf/resolve/main/Phi-3-mini-4k-instruct-q4.gguf
```

## Configuration

### Basic Configuration

```jsonc
{
  "models": {
    "assistant": {
      "runner": "llama-cpp",
      "source": "~/.cache/llmnet/models/tinyllama-1.1b-chat-v1.0.Q4_K_M.gguf",
      "parameters": {
        "n_ctx": 2048,        // Context window
        "n_batch": 512,       // Batch size for prompt processing
        "n_threads": 4,       // Use all 4 CPU cores
        "n_gpu_layers": 0     // No GPU (Pi 5 has no CUDA)
      }
    }
  },
  "architecture": [
    {
      "name": "router",
      "layer": 0,
      "model": "assistant",
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

### Recommended Models

| Model | Parameters | Quantization | RAM Usage | Speed |
|-------|-----------|--------------|-----------|-------|
| TinyLlama 1.1B | 1.1B | Q4_K_M | ~1GB | Fast |
| Phi-3 Mini 3.8B | 3.8B | Q4_K_M | ~3GB | Medium |
| Qwen2.5 0.5B | 0.5B | Q4_K_M | ~0.5GB | Very Fast |
| SmolLM 1.7B | 1.7B | Q4_K_M | ~1.5GB | Fast |

> **Tip:** For the best balance of quality and speed, use TinyLlama 1.1B or SmolLM 1.7B.

## Running llmnet

### Start the Server

```bash
# Start with composition file
llmnet serve --host 0.0.0.0 --port 8080 assistant.jsonc
```

### Test with curl

```bash
curl -X POST http://localhost:8080/v1/chat/completions \
  -H "Content-Type: application/json" \
  -d '{
    "messages": [{"role": "user", "content": "Hello, what can you do?"}]
  }'
```

### Run as a Service

Create a systemd service for automatic startup:

```bash
sudo tee /etc/systemd/system/llmnet.service << 'EOF'
[Unit]
Description=llmnet LLM Pipeline Orchestrator
After=network.target

[Service]
Type=simple
User=pi
WorkingDirectory=/home/pi
ExecStart=/usr/local/bin/llmnet serve --host 0.0.0.0 --port 8080 /home/pi/assistant.jsonc
Restart=always
RestartSec=10

[Install]
WantedBy=multi-user.target
EOF

sudo systemctl daemon-reload
sudo systemctl enable llmnet
sudo systemctl start llmnet
```

## Performance Optimization

### CPU Frequency

Lock CPU to maximum frequency:

```bash
# Check current frequency
cat /sys/devices/system/cpu/cpu0/cpufreq/scaling_cur_freq

# Set performance governor
echo performance | sudo tee /sys/devices/system/cpu/cpu*/cpufreq/scaling_governor
```

### Memory Optimization

1. **Reduce GPU memory** allocation:
   ```bash
   # Edit /boot/firmware/config.txt
   gpu_mem=16
   ```
   Reboot after changing.

2. **Create swap** for larger models:
   ```bash
   sudo dphys-swapfile swapoff
   sudo sed -i 's/CONF_SWAPSIZE=.*/CONF_SWAPSIZE=4096/' /etc/dphys-swapfile
   sudo dphys-swapfile setup
   sudo dphys-swapfile swapon
   ```

3. **Use zram** for better swap performance:
   ```bash
   sudo apt install zram-tools
   echo "PERCENTAGE=50" | sudo tee -a /etc/default/zramswap
   sudo systemctl restart zramswap
   ```

### Storage

Use NVMe via PCIe HAT for best performance:
- Model loading: 5-10x faster than SD card
- Swap performance: 10-20x faster

## Expected Performance

Tokens per second on Raspberry Pi 5 (8GB):

| Model | Prompt Processing | Generation |
|-------|------------------|------------|
| TinyLlama 1.1B Q4 | ~15 t/s | ~8 t/s |
| Phi-3 Mini Q4 | ~8 t/s | ~4 t/s |
| Qwen2.5 0.5B Q4 | ~25 t/s | ~12 t/s |

> Response times are usable for chatbot applications but not suitable for real-time applications requiring sub-100ms latency.

## Use Cases

### Home Assistant Integration

Run llmnet as a local LLM for Home Assistant:

```yaml
# configuration.yaml
conversation:
  intents:
    HassLLMIntentHandler:
      - slots:
          text:
            description: User query
rest_command:
  ask_llm:
    url: "http://localhost:8080/v1/chat/completions"
    method: POST
    content_type: "application/json"
    payload: '{"messages": [{"role": "user", "content": "{{ text }}"}]}'
```

### Offline Chatbot

Create a simple terminal chatbot:

```python
import requests

def chat(message):
    response = requests.post(
        "http://localhost:8080/v1/chat/completions",
        json={"messages": [{"role": "user", "content": message}]}
    )
    return response.json()["choices"][0]["message"]["content"]

while True:
    user_input = input("You: ")
    if user_input.lower() == "quit":
        break
    print(f"AI: {chat(user_input)}")
```

## Troubleshooting

### Slow Generation

- Ensure active cooling is working
- Check CPU throttling: `vcgencmd get_throttled`
- Use smaller quantization (Q4_0 instead of Q4_K_M)
- Reduce context length

### Out of Memory

- Use smaller models (TinyLlama, Qwen2.5-0.5B)
- Increase swap size
- Reduce `n_ctx` parameter
- Close other applications

### llama-server Not Found

Ensure llama.cpp was built and installed:
```bash
which llama-server
# Should show: /usr/local/bin/llama-server
```

## Comparison with Jetson

| Aspect | Raspberry Pi 5 | Jetson Orin Nano |
|--------|---------------|------------------|
| Cost | ~$80 | ~$500 |
| Power | 5-15W | 7-25W |
| Max Model | 3B | 7B |
| Speed | Slow | Fast |
| Use Case | Hobby/Education | Production |

Choose Raspberry Pi for cost-sensitive or educational projects. Choose Jetson for production edge AI deployments.
