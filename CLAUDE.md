# CLAUDE.md

This file provides guidance to Claude Code (claude.ai/code) when working with code in this repository.

## Build & Test Commands

```bash
cargo build --release              # Build optimized binary
cargo test                         # Run all tests (unit + integration)
cargo test --lib                   # Library tests only
cargo test <test_name>             # Run single test
cargo test --test hooks_integration  # Run specific integration test file
cargo clippy                       # Lint
```

## Architecture Overview

llmnet orchestrates LLM pipelines where requests flow through layered nodes with intelligent routing. Think of it as a neural network where each "neuron" is an LLM.

```
User Query → Router (Layer 0) → [Expert A | Expert B] (Layer 1) → Refiner (Layer 2) → Output
```

### Module Structure

| Module | Purpose |
|--------|---------|
| `config/` | Composition parsing, validation, secrets, functions. SBIO pattern: pure parsing functions + thin I/O wrappers |
| `runtime/` | Pipeline execution engine: `processor.rs` (main loop), `router.rs` (node selection), `hooks.rs` (pre/post hooks), `node.rs` (adapters) |
| `cluster/` | K8s-inspired control plane: pipelines, nodes, heartbeats, scoring, autoscaling |
| `cli/` | kubectl-like commands: serve, deploy, get, scale, delete, context |
| `adapters/` | Protocol handlers: OpenAI API, output, WebSocket |
| `server/` | HTTP endpoints and state management |

### Key Abstractions

**CompositionConfig** (`config/composition.rs`): Top-level config with models, architecture nodes, secrets, functions.

**ArchitectureNode** (`config/architecture.rs`): Pipeline node with layer, model reference, adapter type, output-to targets, conditions, hooks.

**PipelineProcessor** (`runtime/processor.rs`): Main orchestration loop - routes through layers, executes hooks, tracks request state.

**PipelineRequest** (`runtime/request.rs`): Request context with content, trace history, system variables ($INPUT, $OUTPUT, $NODE, $WORD_COUNT, etc.).

### Execution Flow

1. Request enters at Layer 0 (router)
2. Router LLM selects target node based on use-case descriptions
3. Pre-hooks execute (observe: fire-and-forget, transform: blocking)
4. Selected node's LLM called
5. Post-hooks execute
6. Route to next layer via `output-to`
7. Repeat until output node reached

### SBIO Pattern (Separation of Business logic and I/O)

Applied throughout for testability:
- **Pure functions**: `parse_composition()`, `validate_composition()`, `build_routing_prompt()`, `extract_node_selection()`, `evaluate_condition()`
- **I/O wrappers**: `load_composition_file()` - thin shell over pure functions

Example: `config/mod.rs` has `load_composition_file()` (I/O) calling `strip_jsonc_comments()` + `parse_composition()` + `validate_composition()` (all pure).

### Hooks & Functions System

**Functions** (`config/functions.rs`): REST, Shell, WebSocket, gRPC operations with variable substitution.

**Hooks** (`runtime/hooks.rs`): Pre/post execution on nodes with modes:
- `observe`: Non-blocking, doesn't affect pipeline
- `transform`: Blocking, result can modify data
- `on_failure`: `continue` or `abort`

### Variable Substitution

Available in conditions, hooks, functions:
- `$INPUT`, `$OUTPUT`, `$NODE`, `$PREV_NODE`
- `$WORD_COUNT`, `$INPUT_LENGTH`, `$HOP_COUNT`
- `$TIMESTAMP`, `$REQUEST_ID`
- `$secrets.{name}.{variable}`

## Testing Approach

- Unit tests co-located with implementation (SBIO makes this easy)
- Integration tests in `tests/` (e.g., `hooks_integration.rs` with thin Axum HTTP servers)
- Focus on pure functions; minimal mocking due to SBIO separation
