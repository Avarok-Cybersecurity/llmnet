# Installation

## From Source

```bash
# Clone the repository
git clone https://github.com/your-org/llmnet.git
cd llmnet

# Build in release mode
cargo build --release

# The binary is at target/release/llmnet
./target/release/llmnet --help
```

## Prerequisites

- **Rust 1.75+**: Install via [rustup](https://rustup.rs/)
- **LLM Backend** (one of):
  - [Ollama](https://ollama.ai/) - Local models
  - [vLLM](https://docs.vllm.ai/) - Production GPU inference
  - [llama.cpp](https://github.com/ggerganov/llama.cpp) - CPU/Metal inference
  - External API (OpenAI-compatible)

## Verify Installation

```bash
llmnet --version
llmnet validate examples/basic-router.json
```
