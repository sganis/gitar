#!/usr/bin/env python3
import os
import sys
import json
import argparse
import requests

TIMEOUT = 30

PROVIDERS = {
    "openai": {
        "base_url": "https://api.openai.com/v1",
        "api_key_env": "OPENAI_API_KEY",
    },
    "groq": {
        "base_url": "https://api.groq.com/openai/v1",
        "api_key_env": "GROQ_API_KEY",
    },
    "together": {
        "base_url": "https://api.together.xyz/v1",
        "api_key_env": "TOGETHER_API_KEY",
    },
    "deepinfra": {
        "base_url": "https://api.deepinfra.com/v1/openai",
        "api_key_env": "DEEPINFRA_API_KEY",
    },
    "openrouter": {
        "base_url": "https://openrouter.ai/api/v1",
        "api_key_env": "OPENROUTER_API_KEY",
        "extra_headers": {
            "HTTP-Referer": os.environ.get("OPENROUTER_HTTP_REFERER", ""),
            "X-Title": os.environ.get("OPENROUTER_X_TITLE", "gitar-test"),
        }
    },
    "ollama": {
        "base_url": os.environ.get("OLLAMA_BASE_URL", "http://localhost:11434/v1"),
        "api_key_env": None,
    },
}

def build_headers(api_key, extra=None):
    h = {
        "Content-Type": "application/json",
        "Accept": "application/json",
    }
    if api_key:
        h["Authorization"] = f"Bearer {api_key}"
    if extra:
        for k, v in extra.items():
            if v:
                h[k] = v
    return h

def pick_model(models_json):
    data = models_json.get("data", [])
    if not data:
        return None
    ids = [m["id"] for m in data if "id" in m]
    if not ids:
        return None

    prefer = ["gpt", "llama", "qwen", "deepseek", "mistral", "mixtral", "instruct"]
    for p in prefer:
        for mid in ids:
            if p in mid.lower():
                return mid
    return ids[0]

def main():
    ap = argparse.ArgumentParser()
    ap.add_argument("provider", choices=PROVIDERS.keys())
    ap.add_argument("--model", help="Override model name")
    ap.add_argument("--list", action="store_true", help="List all models and exit")
    args = ap.parse_args()

    cfg = PROVIDERS[args.provider]
    base_url = cfg["base_url"].rstrip("/")
    api_key = os.environ.get(cfg["api_key_env"]) if cfg["api_key_env"] else None

    if cfg["api_key_env"] and not api_key:
        print(f"ERROR: Missing env var {cfg['api_key_env']}")
        sys.exit(1)

    headers = build_headers(api_key, cfg.get("extra_headers"))

    # -----------------------------
    # 1) GET /models
    # -----------------------------
    print(f"==> Testing {args.provider}")
    print(f"GET {base_url}/models")

    r = requests.get(f"{base_url}/models", headers=headers, timeout=TIMEOUT)
    print("Status:", r.status_code)

    if r.status_code != 200:
        print("Response:", r.text[:1000])
        sys.exit(1)

    models_json = r.json()
    data = models_json.get("data", [])

    print(f"Models found: {len(data)}")
    
    # If --list: just print and exit
    if args.list:
        for m in data:
            print(m.get("id"))
        print("\nDone.")
        return
    
    model = args.model or pick_model(models_json)
    if not model:
        print("ERROR: Could not auto-pick a model. Use --model")
        sys.exit(1)

    print("Using model:", model)

    # -----------------------------
    # 2) POST /chat/completions
    # -----------------------------
    print(f"POST {base_url}/chat/completions")

    payload = {
        "model": model,
        "messages": [
            {"role": "user", "content": "Reply with exactly: OK"}
        ],
        "temperature": 0,
        "max_tokens": 16
    }

    r = requests.post(
        f"{base_url}/chat/completions",
        headers=headers,
        data=json.dumps(payload),
        timeout=TIMEOUT
    )

    print("Status:", r.status_code)

    if r.status_code != 200:
        print("Response:", r.text[:2000])
        sys.exit(1)

    data = r.json()
    text = data["choices"][0]["message"]["content"]
    print("Model reply:", repr(text))

    print("\nSUCCESS")

if __name__ == "__main__":
    main()
