from flask import Flask, request, Response
from llama_cpp import Llama
import json
import random
import re

# ------------------------------
# Model setup
# ------------------------------
MODEL_PATH = "models/llama-2-7b-chat.Q5_K_M.gguf"
llm = Llama(MODEL_PATH, n_ctx=4096, n_gpu_layers=50, use_mmap=True, verbose=False)

# Warmup: run a dummy inference to load model weights and initialize GPU layers
try:
    _ = llm(
        "[INST] <<SYS>> Warmup. <</SYS>> Hello! [/INST]",
        max_tokens=8,
        temperature=0.0,
        stream=False
    )
except Exception as e:
    print(f"Warmup failed: {e}")

app = Flask(__name__)
@app.route("/healthcheck", methods=["GET"])
def healthcheck():
    return "OK", 200

SYSTEM_PREFIX = "[INST] <<SYS>> You are a helpful, friendly assistant. <</SYS>> "
SYSTEM_SUFFIX = " [/INST]"

# ------------------------------
# Helpers
# ------------------------------
def clean_response(response: str) -> str:
    """Minimal cleanup: remove system tokens and fix known actions."""
    # Remove system tokens
    for token in ["[INST]", "[/INST]", "<<SYS>>", "<</SYS>>", "User:", "user:", "Bot:", "bot:"]:
        response = response.replace(token, "")

    # Replace known LLaMA action tokens with proper formatting
    actions = {
        "thinking": "*thinking*",
        # add more as needed
    }
    for key, val in actions.items():
        response = response.replace(key, val)

    # Collapse multiple consecutive newlines or trailing spaces
    response = re.sub(r'\s+\n', '\n', response)
    response = re.sub(r'\n\s+', '\n', response)
    response = response.strip()

    return response

# ------------------------------
# Flask route
# ------------------------------
@app.route("/chat", methods=["POST"])
def chat():
    data = request.json or {}
    message = data.get("message", "")
    nickname = data.get("nickname", "")

    # Randomly prepend nickname
    use_nickname = random.choice([True, False, False])
    prompt = f"{nickname}, {message}" if use_nickname and nickname else message
    system_prompt = f"{SYSTEM_PREFIX}{prompt}{SYSTEM_SUFFIX}"

    def generate():
        try:
            # Generate full response (no streaming)
            resp = llm(
                system_prompt,
                max_tokens=128,
                temperature=0.7,
                stream=False
            )

            # Handle different possible return types
            if isinstance(resp, dict) and "choices" in resp:
                text = resp["choices"][0]["text"]
            elif isinstance(resp, dict) and "text" in resp:
                text = resp["text"]
            else:
                text = str(resp)  # fallback

            text = clean_response(text)
            yield text

        except Exception as e:
            yield f"Error generating response: {e}"

    return Response(generate(), mimetype="text/plain")

# ------------------------------
# Run Flask
# ------------------------------
if __name__ == "__main__":
    app.run(host="127.0.0.1", port=5005)
