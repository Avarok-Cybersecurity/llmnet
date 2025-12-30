# Multiplication Calculator with Hooks

This example demonstrates hooks in action with a fun arithmetic pipeline:

1. User asks: "What's 2*4?"
2. LLM returns JSON with the result
3. Post-hook validates the answer
4. If valid, a second LLM doubles it
5. Post-hook validates again
6. Final result is output

## The Composition

```json
{
  "secrets": {
    "validator": {
      "source": "env",
      "variable": "VALIDATOR_API_KEY"
    }
  },
  "functions": {
    "validate-math": {
      "type": "rest",
      "method": "POST",
      "url": "http://localhost:8090/validate",
      "body": {
        "expression": "$INPUT",
        "result": "$OUTPUT"
      },
      "timeout": 5
    }
  },
  "models": {
    "calculator": {
      "type": "external",
      "interface": "openai-api",
      "url": "http://localhost:11434/v1",
      "api-key": "ollama"
    }
  },
  "architecture": [
    {
      "name": "router",
      "layer": 0,
      "model": "calculator",
      "adapter": "openai-api",
      "output-to": [1]
    },
    {
      "name": "initial-calc",
      "layer": 1,
      "model": "calculator",
      "adapter": "openai-api",
      "use-case": "Calculate the initial multiplication and return JSON only",
      "context": "You are a calculator. Return ONLY a JSON object with the format: {\"result\": <number>}. No explanation.",
      "hooks": {
        "post": [
          {
            "function": "validate-math",
            "mode": "transform",
            "on_failure": "abort"
          }
        ]
      },
      "output-to": [2]
    },
    {
      "name": "doubler",
      "layer": 2,
      "model": "calculator",
      "adapter": "openai-api",
      "use-case": "Double the previous result",
      "context": "You receive a number. Double it and return ONLY a JSON object: {\"result\": <number>}. No explanation.",
      "hooks": {
        "post": [
          {
            "function": "validate-math",
            "mode": "transform",
            "on_failure": "abort"
          }
        ]
      },
      "output-to": ["output"]
    },
    {
      "name": "output",
      "adapter": "output"
    }
  ]
}
```

## The Validation Server

Here's a simple validation server in Python that checks if the LLM's math is correct:

```python
# validator_server.py
from flask import Flask, request, jsonify
import json
import re

app = Flask(__name__)

def extract_result(text):
    """Extract the result from JSON or plain number."""
    try:
        data = json.loads(text)
        return data.get("result")
    except:
        # Try to find a number in the text
        match = re.search(r"(\d+(?:\.\d+)?)", text)
        return float(match.group(1)) if match else None

def evaluate_expression(expr):
    """Safely evaluate a simple math expression."""
    # Only allow basic multiplication
    match = re.match(r".*?(\d+)\s*\*\s*(\d+).*", expr)
    if match:
        return int(match.group(1)) * int(match.group(2))

    # If input is just a number (for doubling), return it doubled
    match = re.search(r'"?result"?\s*:\s*(\d+)', expr)
    if match:
        return int(match.group(1)) * 2

    return None

@app.route("/validate", methods=["POST"])
def validate():
    data = request.json
    expression = data.get("expression", "")
    output = data.get("result", "")

    # Extract the result from LLM output
    llm_result = extract_result(output)
    if llm_result is None:
        return jsonify({"error": "Could not parse result"}), 400

    # Calculate expected result
    expected = evaluate_expression(expression)

    # For doubling, check if it's double the input
    if expected is None:
        # Maybe it's a doubling operation
        match = re.search(r'"?result"?\s*:\s*(\d+)', expression)
        if match:
            expected = int(match.group(1)) * 2

    # Validate
    if expected is not None and abs(llm_result - expected) < 0.01:
        # Return the validated result (transform mode)
        return jsonify({"result": int(llm_result)})
    else:
        return jsonify({
            "error": f"Math error: expected {expected}, got {llm_result}"
        }), 400

if __name__ == "__main__":
    app.run(port=8090)
```

## Running the Example

### 1. Start the Validator Server

```bash
pip install flask
python validator_server.py
```

### 2. Start Ollama

```bash
ollama serve
ollama pull llama3.2:3b
```

### 3. Run the Pipeline

```bash
llmnet serve calculator.json
```

### 4. Test It

```bash
# Ask "What's 2*4?"
curl -X POST http://localhost:8080/v1/chat/completions \
  -H "Content-Type: application/json" \
  -d '{"messages": [{"role": "user", "content": "What is 2*4?"}]}'
```

Expected flow:
1. Router → initial-calc
2. initial-calc LLM returns `{"result": 8}`
3. validate-math hook confirms 2*4=8 ✓
4. Pass to doubler
5. doubler LLM returns `{"result": 16}`
6. validate-math hook confirms 8*2=16 ✓
7. Output: `{"result": 16}`

## Key Concepts Demonstrated

### Transform Hooks
The `validate-math` function uses `"mode": "transform"`, meaning:
- It waits for the validation to complete
- If validation passes, the response is used as the new output
- If validation fails (HTTP 4xx/5xx), the pipeline aborts

### Hook Chaining
Each layer has its own post-hook validation, creating a chain of verified computations.

### Abort on Failure
Using `"on_failure": "abort"` ensures that math errors stop the pipeline rather than propagating incorrect values.

## Extending the Example

### Add Logging
```json
{
  "functions": {
    "log-calculation": {
      "type": "rest",
      "method": "POST",
      "url": "http://localhost:8091/log",
      "body": {
        "input": "$INPUT",
        "output": "$OUTPUT",
        "node": "$NODE",
        "timestamp": "$TIMESTAMP"
      }
    }
  }
}
```

Then add to each node:
```json
"hooks": {
  "post": [
    {"function": "log-calculation", "mode": "observe"},
    {"function": "validate-math", "mode": "transform", "on_failure": "abort"}
  ]
}
```

### Add Conditional Validation
Only validate for longer expressions:
```json
{
  "function": "validate-math",
  "mode": "transform",
  "on_failure": "abort",
  "if": "$WORD_COUNT >= 3"
}
```
