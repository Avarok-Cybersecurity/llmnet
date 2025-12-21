# Nemotron-Orchestrator-8B Usage Guide

## Overview

**Nemotron-Orchestrator-8B** is an 8-billion parameter AI orchestration model developed by NVIDIA and the University of Hong Kong. Rather than solving tasks directly, it intelligently coordinates multiple specialized tools and AI models to solve complex, multi-turn agentic tasks efficiently.

### Key Characteristics

| Property | Value |
|----------|-------|
| Base Model | Qwen3-8B |
| Architecture | Decoder-only Transformer |
| Parameters | 8B |
| Quantization | AWQ 4-bit (compressed-tensors) |
| Context Length | 8,192 tokens |
| License | NVIDIA License (Research & Development) |

## Performance Benchmarks

| Benchmark | Orchestrator-8B | GPT-5 | Efficiency |
|-----------|-----------------|-------|------------|
| Humanity's Last Exam (HLE) | **37.1%** | 35.1% | 2.5x faster |
| FRAMES | Outperforms | Baseline | ~30% cost |
| τ²-Bench | Outperforms | Baseline | ~30% cost |
| GAIA | **#1 Ranked** | - | - |

**Cost Comparison:** ~$9.20 per query vs GPT-5's $30.20

## How It Works

The Orchestrator operates in a **multi-turn reasoning loop** (up to 50 turns):

```
1. Read user query and preferences
2. Generate reasoning (thinking)
3. Select appropriate tool
4. Output JSON tool call
5. Receive tool observation
6. Repeat until task complete
```

### Tool Categories

The model can orchestrate three types of tools:

1. **Basic Tools**: Web search (Tavily), Python code sandbox, local document search
2. **Specialized LLMs**: Math models, coding models, domain experts
3. **Generalist LLMs**: GPT-5, Claude Opus 4.1, Llama-Nemotron-Ultra-253B

## Tool Schema Format

Tools are defined as JSON objects with this structure:

```json
{
  "name": "tool_name",
  "description": "What the tool does and when to use it",
  "parameters": {
    "type": "object",
    "properties": {
      "param_name": {
        "type": "string",
        "description": "Parameter description"
      }
    },
    "required": ["param_name"]
  }
}
```

### Example Tool Definitions

```json
[
  {
    "name": "web_search",
    "description": "Search the web for current information. Use for recent events, facts, or data not in training.",
    "parameters": {
      "type": "object",
      "properties": {
        "query": {
          "type": "string",
          "description": "The search query"
        }
      },
      "required": ["query"]
    }
  },
  {
    "name": "python_executor",
    "description": "Execute Python code for calculations, data analysis, or complex computations.",
    "parameters": {
      "type": "object",
      "properties": {
        "code": {
          "type": "string",
          "description": "Python code to execute"
        }
      },
      "required": ["code"]
    }
  },
  {
    "name": "math_expert",
    "description": "Advanced mathematical reasoning model. Use for complex proofs, equations, and scientific calculations.",
    "parameters": {
      "type": "object",
      "properties": {
        "problem": {
          "type": "string",
          "description": "The mathematical problem to solve"
        }
      },
      "required": ["problem"]
    }
  }
]
```

## API Usage (vLLM OpenAI-Compatible)

### Basic Chat Completion

```bash
curl http://10.10.10.2:44443/v1/chat/completions \
  -H "Content-Type: application/json" \
  -d '{
    "model": "cyankiwi/Nemotron-Orchestrator-8B-AWQ-4bit",
    "messages": [
      {"role": "user", "content": "What is the current weather in Tokyo?"}
    ],
    "max_tokens": 1024
  }'
```

### With Tool Definitions

```bash
curl http://10.10.10.2:44443/v1/chat/completions \
  -H "Content-Type: application/json" \
  -d '{
    "model": "cyankiwi/Nemotron-Orchestrator-8B-AWQ-4bit",
    "messages": [
      {"role": "system", "content": "You are an AI orchestrator. Use the available tools to solve tasks efficiently. Prefer lower-cost tools when possible."},
      {"role": "user", "content": "Calculate the compound interest on $10,000 at 5% for 10 years"}
    ],
    "tools": [
      {
        "type": "function",
        "function": {
          "name": "python_executor",
          "description": "Execute Python code for calculations",
          "parameters": {
            "type": "object",
            "properties": {
              "code": {"type": "string", "description": "Python code to execute"}
            },
            "required": ["code"]
          }
        }
      }
    ],
    "tool_choice": "auto",
    "max_tokens": 1024
  }'
```

### Python Client Example

```python
from openai import OpenAI

client = OpenAI(
    base_url="http://10.10.10.2:44443/v1",
    api_key="not-needed"
)

# Define available tools
tools = [
    {
        "type": "function",
        "function": {
            "name": "web_search",
            "description": "Search the web for information",
            "parameters": {
                "type": "object",
                "properties": {
                    "query": {"type": "string", "description": "Search query"}
                },
                "required": ["query"]
            }
        }
    },
    {
        "type": "function",
        "function": {
            "name": "python_executor",
            "description": "Execute Python code",
            "parameters": {
                "type": "object",
                "properties": {
                    "code": {"type": "string", "description": "Python code"}
                },
                "required": ["code"]
            }
        }
    }
]

# System prompt for orchestration
system_prompt = """You are an intelligent orchestrator that coordinates tools to solve complex tasks.

Guidelines:
- Break complex problems into steps
- Use the most appropriate tool for each step
- Prefer efficient/low-cost tools when possible
- Explain your reasoning before tool calls
- Synthesize results into a final answer"""

response = client.chat.completions.create(
    model="cyankiwi/Nemotron-Orchestrator-8B-AWQ-4bit",
    messages=[
        {"role": "system", "content": system_prompt},
        {"role": "user", "content": "What's the population of France and calculate what percentage that is of the world population?"}
    ],
    tools=tools,
    tool_choice="auto",
    max_tokens=2048
)

print(response.choices[0].message)
```

## Tool Call Output Format

The model outputs tool calls in JSON format wrapped in its response:

```json
{
  "name": "web_search",
  "arguments": {
    "query": "current population of France 2025"
  }
}
```

Or using the `<toolcall>` XML wrapper (Nemotron-style):

```xml
<toolcall>
{"name": "python_executor", "arguments": {"code": "result = 67.75 / 8000 * 100\nprint(f'{result:.2f}%')"}}
</toolcall>
```

## System Prompt Recommendations

### For General Orchestration

```
You are an AI orchestrator that coordinates multiple tools and models to solve complex tasks efficiently.

Your available tools include:
- Web search for current information
- Python executor for calculations and data processing
- Specialized models for math, coding, and reasoning

Guidelines:
1. Analyze the task and break it into sub-problems
2. Select the most appropriate tool for each step
3. Consider cost and latency when choosing tools
4. Explain your reasoning in <think> tags
5. Synthesize tool outputs into a coherent final answer

User preferences: {preferences}
```

### For Cost-Optimized Tasks

```
You are a cost-efficient AI orchestrator. Minimize computational costs while maintaining accuracy.

Priority order for tool selection:
1. Basic tools (search, code execution) - lowest cost
2. Specialized small models - medium cost
3. Large generalist models - highest cost (use only when necessary)

Always prefer the cheapest tool that can adequately solve each sub-task.
```

### For Quality-Optimized Tasks

```
You are an accuracy-focused AI orchestrator. Prioritize correctness over efficiency.

For complex tasks:
- Use multiple tools to verify results
- Prefer specialized models for domain-specific problems
- Cross-reference information from multiple sources
- Show your work and reasoning process
```

## Multi-Turn Orchestration Loop

For implementing a full orchestration loop:

```python
import json
from openai import OpenAI

client = OpenAI(base_url="http://10.10.10.2:44443/v1", api_key="none")

def execute_tool(name: str, arguments: dict) -> str:
    """Execute a tool and return its output."""
    if name == "web_search":
        # Implement actual web search
        return f"Search results for: {arguments['query']}"
    elif name == "python_executor":
        # Implement sandboxed Python execution
        exec_globals = {}
        exec(arguments['code'], exec_globals)
        return str(exec_globals.get('result', 'Executed'))
    return "Tool not found"

def orchestrate(user_query: str, tools: list, max_turns: int = 10):
    messages = [
        {"role": "system", "content": "You are an AI orchestrator..."},
        {"role": "user", "content": user_query}
    ]

    for turn in range(max_turns):
        response = client.chat.completions.create(
            model="cyankiwi/Nemotron-Orchestrator-8B-AWQ-4bit",
            messages=messages,
            tools=tools,
            max_tokens=1024
        )

        assistant_message = response.choices[0].message
        messages.append(assistant_message)

        # Check if model wants to call tools
        if assistant_message.tool_calls:
            for tool_call in assistant_message.tool_calls:
                result = execute_tool(
                    tool_call.function.name,
                    json.loads(tool_call.function.arguments)
                )
                messages.append({
                    "role": "tool",
                    "tool_call_id": tool_call.id,
                    "content": result
                })
        else:
            # No more tool calls, return final answer
            return assistant_message.content

    return "Max turns reached"
```

## Inference Parameters

| Parameter | Recommended Value | Description |
|-----------|-------------------|-------------|
| `temperature` | 0.6 - 0.7 | Balance creativity and consistency |
| `top_p` | 0.9 | Nucleus sampling threshold |
| `max_tokens` | 1024 - 2048 | Sufficient for reasoning + tool calls |
| `repetition_penalty` | 1.05 | Prevent repetitive outputs |

## Best Practices

1. **Define Clear Tool Descriptions**: The model routes based on descriptions, so be specific about each tool's capabilities and use cases.

2. **Include Cost/Latency Metadata**: When wrapping LLMs as tools, include performance characteristics in descriptions.

3. **Set User Preferences**: Specify if the user prefers speed, cost-efficiency, or accuracy.

4. **Limit Turn Count**: Set reasonable max turns (10-50) to prevent infinite loops.

5. **Provide Examples**: Few-shot examples of tool usage can improve routing decisions.

6. **Handle Tool Failures**: Implement fallback logic when tools return errors.

## Resources

- **Model**: [huggingface.co/nvidia/Nemotron-Orchestrator-8B](https://huggingface.co/nvidia/Nemotron-Orchestrator-8B)
- **Quantized Version**: [huggingface.co/cyankiwi/Nemotron-Orchestrator-8B-AWQ-4bit](https://huggingface.co/cyankiwi/Nemotron-Orchestrator-8B-AWQ-4bit)
- **Training Framework**: [github.com/NVlabs/ToolOrchestra](https://github.com/NVlabs/ToolOrchestra)
- **Dataset**: [huggingface.co/datasets/nvidia/ToolScale](https://huggingface.co/datasets/nvidia/ToolScale)
- **Paper**: [arxiv.org/abs/2511.21689](https://arxiv.org/abs/2511.21689)

## Current Deployment

```
Endpoint: http://10.10.10.2:44443
Model: cyankiwi/Nemotron-Orchestrator-8B-AWQ-4bit
Runner: vLLM 0.13.0rc2
GPU Memory: 5.99 GiB
KV Cache: 98.6 GiB (~717K tokens)
Max Concurrency: 87x at 8K context
```
