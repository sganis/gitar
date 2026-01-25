#!/usr/bin/env python3
import os
import sys
import json
import argparse
import requests
from typing import Any, Dict, Optional, List

TIMEOUT = 30

# -----------------------------------------------------------------------------
# Provider registry
# -----------------------------------------------------------------------------
# type:
# - "openai_compat":  GET {base}/models, POST {base}/chat/completions
# - "anthropic":      GET https://api.anthropic.com/v1/models, POST /v1/messages
# - "gemini":         GET https://generativelanguage.googleapis.com/v1beta/models?key=...
#                     POST https://generativelanguage.googleapis.com/v1beta/models/{model}:generateContent?key=...
#
# cost:
# FREE
#   └── Ollama (local)
# VERY CHEAP
#   ├── Groq
#   ├── OpenRouter (free tier)
#   ├── Together.ai
#   ├── DeepInfra
# NOT SO CHEAP
#   ├── Mistral
#   ├── Fireworks
#   ├── xAI (Grok)
# PREMIUM
#   ├── OpenAI
#   ├── Claude (Anthropic)
#   └── Gemini (Google)

PROVIDERS: Dict[str, Dict[str, Any]] = {
    # OpenAI-compatible
    "openai": {
        "type": "openai_compat",
        "base_url": "https://api.openai.com/v1",
        "api_key_env": "OPENAI_API_KEY",
    },
    "xai": {
        "type": "openai_compat",
        "base_url": "https://api.x.ai/v1",
        "api_key_env": "XAI_API_KEY",
    },
    "mistral": {
        "type": "openai_compat",
        "base_url": "https://api.mistral.ai/v1",
        "api_key_env": "MISTRAL_API_KEY",
    },
    "groq": {
        "type": "openai_compat",
        "base_url": "https://api.groq.com/openai/v1",
        "api_key_env": "GROQ_API_KEY",
    },
    "together": {
        "type": "openai_compat",
        "base_url": "https://api.together.xyz/v1",
        "api_key_env": "TOGETHER_API_KEY",
    },
    "deepinfra": {
        "type": "openai_compat",
        "base_url": "https://api.deepinfra.com/v1/openai",
        "api_key_env": "DEEPINFRA_API_KEY",
    },
    "openrouter": {
        "type": "openai_compat",
        "base_url": "https://openrouter.ai/api/v1",
        "api_key_env": "OPENROUTER_API_KEY",
        "extra_headers": {
            # Optional but recommended by OpenRouter
            "HTTP-Referer": os.environ.get("OPENROUTER_HTTP_REFERER", ""),
            "X-Title": os.environ.get("OPENROUTER_X_TITLE", "gitar-llm-test"),
        },
    },
    "ollama": {
        "type": "openai_compat",
        "base_url": os.environ.get("OLLAMA_BASE_URL", "http://localhost:11434/v1"),
        "api_key_env": None,  # local default: no key
    },

    # Native APIs (not OpenAI-compatible)
    "claude": {
        "type": "anthropic",
        "base_url": "https://api.anthropic.com",
        "api_key_env": "ANTHROPIC_API_KEY",
        "anthropic_version": os.environ.get("ANTHROPIC_VERSION", "2023-06-01"),
    },
    "gemini": {
        "type": "gemini",
        "base_url": "https://generativelanguage.googleapis.com",
        "api_key_env": "GEMINI_API_KEY",
        # Uses ?key=... query param, not Bearer auth.
    },
}

# -----------------------------------------------------------------------------
# Utilities
# -----------------------------------------------------------------------------
def die(msg: str, code: int = 1) -> None:
    print(f"ERROR: {msg}", file=sys.stderr)
    sys.exit(code)

def safe_json(resp: requests.Response) -> Any:
    try:
        return resp.json()
    except Exception:
        return {"_non_json_body": resp.text[:4000]}

def pretty(obj: Any) -> str:
    return json.dumps(obj, indent=2, ensure_ascii=False)[:8000]

def build_openai_compat_headers(api_key: Optional[str], extra: Optional[Dict[str, str]] = None) -> Dict[str, str]:
    h = {"Accept": "application/json", "Content-Type": "application/json"}
    if api_key:
        h["Authorization"] = f"Bearer {api_key}"
    if extra:
        for k, v in extra.items():
            if v:
                h[k] = v
    return h

def pick_model_from_openai_models(models_json: Any) -> Optional[str]:
    data = models_json.get("data") if isinstance(models_json, dict) else None
    if not isinstance(data, list) or not data:
        return None
    ids = [m.get("id") for m in data if isinstance(m, dict) and isinstance(m.get("id"), str)]
    if not ids:
        return None
    prefer = ["gpt", "llama", "qwen", "deepseek", "mistral", "mixtral", "instruct", "sonar", "grok"]
    for p in prefer:
        for mid in ids:
            if p in mid.lower():
                return mid
    return ids[0]

def extract_openai_chat_text(chat_json: Any) -> Optional[str]:
    # {"choices":[{"message":{"content":"..."}}]}
    if not isinstance(chat_json, dict):
        return None
    choices = chat_json.get("choices")
    if not isinstance(choices, list) or not choices:
        return None
    msg = choices[0].get("message")
    if isinstance(msg, dict) and isinstance(msg.get("content"), str):
        return msg["content"]
    return None

# -----------------------------------------------------------------------------
# OpenAI-compatible flow
# -----------------------------------------------------------------------------
def openai_compat_list_models(base_url: str, headers: Dict[str, str]) -> Any:
    url = base_url.rstrip("/") + "/models"
    r = requests.get(url, headers=headers, timeout=TIMEOUT)
    if r.status_code != 200:
        die(f"GET {url} -> {r.status_code}\n{r.text[:2000]}")
    return r.json()

def openai_compat_chat(base_url: str, headers: Dict[str, str], model: str, prompt: str, max_tokens: int) -> Any:
    url = base_url.rstrip("/") + "/chat/completions"
    payload = {
        "model": model,
        "messages": [{"role": "user", "content": prompt}],
        "temperature": 0,
        "max_tokens": max_tokens,
    }
    r = requests.post(url, headers=headers, data=json.dumps(payload), timeout=TIMEOUT)
    if r.status_code != 200:
        die(f"POST {url} -> {r.status_code}\n{r.text[:4000]}")
    return r.json()

# -----------------------------------------------------------------------------
# Anthropic (Claude) flow
# -----------------------------------------------------------------------------
def anthropic_headers(api_key: str, anthropic_version: str) -> Dict[str, str]:
    return {
        "Accept": "application/json",
        "Content-Type": "application/json",
        "x-api-key": api_key,
        "anthropic-version": anthropic_version,
    }

def anthropic_list_models(base_url: str, headers: Dict[str, str]) -> Any:
    url = base_url.rstrip("/") + "/v1/models"
    r = requests.get(url, headers=headers, timeout=TIMEOUT)
    if r.status_code != 200:
        die(f"GET {url} -> {r.status_code}\n{r.text[:2000]}")
    return r.json()

def anthropic_pick_model(models_json: Any) -> Optional[str]:
    # {"data":[{"id":"claude-..."},...]}
    if not isinstance(models_json, dict):
        return None
    data = models_json.get("data")
    if not isinstance(data, list) or not data:
        return None
    ids = [m.get("id") for m in data if isinstance(m, dict) and isinstance(m.get("id"), str)]
    if not ids:
        return None
    prefer = ["sonnet", "opus", "haiku", "claude"]
    for p in prefer:
        for mid in ids:
            if p in mid.lower():
                return mid
    return ids[0]

def anthropic_chat(base_url: str, headers: Dict[str, str], model: str, prompt: str, max_tokens: int) -> Any:
    url = base_url.rstrip("/") + "/v1/messages"
    payload = {
        "model": model,
        "max_tokens": max_tokens,
        "messages": [{"role": "user", "content": prompt}],
    }
    r = requests.post(url, headers=headers, data=json.dumps(payload), timeout=TIMEOUT)
    if r.status_code != 200:
        die(f"POST {url} -> {r.status_code}\n{r.text[:4000]}")
    return r.json()

def anthropic_extract_text(msg_json: Any) -> Optional[str]:
    # {"content":[{"type":"text","text":"..."}]}
    if not isinstance(msg_json, dict):
        return None
    content = msg_json.get("content")
    if not isinstance(content, list) or not content:
        return None
    first = content[0]
    if isinstance(first, dict) and isinstance(first.get("text"), str):
        return first["text"]
    return None

# -----------------------------------------------------------------------------
# Gemini (AI Studio) flow
# -----------------------------------------------------------------------------
def gemini_list_models(base_url: str, api_key: str) -> Any:
    url = base_url.rstrip("/") + "/v1beta/models"
    r = requests.get(url, params={"key": api_key}, timeout=TIMEOUT)
    if r.status_code != 200:
        die(f"GET {url} -> {r.status_code}\n{r.text[:2000]}")
    return r.json()

def gemini_pick_model(models_json: Any) -> Optional[str]:
    # {"models":[{"name":"models/gemini-..."}]}
    if not isinstance(models_json, dict):
        return None
    models = models_json.get("models")
    if not isinstance(models, list) or not models:
        return None
    names = [m.get("name") for m in models if isinstance(m, dict) and isinstance(m.get("name"), str)]
    if not names:
        return None
    prefer = ["gemini-2", "gemini-1.5", "pro", "flash"]
    for p in prefer:
        for n in names:
            if p in n.lower():
                return n
    return names[0]

def gemini_chat(base_url: str, api_key: str, model_name: str, prompt: str, max_tokens: int) -> Any:
    # model_name typically like "models/gemini-1.5-flash"
    model_short = model_name.split("/", 1)[1] if model_name.startswith("models/") else model_name
    url = base_url.rstrip("/") + f"/v1beta/models/{model_short}:generateContent"
    payload = {
        "contents": [
            {"role": "user", "parts": [{"text": prompt}]}
        ],
        # generationConfig is optional; keep minimal
        "generationConfig": {
            "maxOutputTokens": max_tokens,
            "temperature": 0,
        },
    }
    r = requests.post(url, params={"key": api_key}, json=payload, timeout=TIMEOUT)
    if r.status_code != 200:
        die(f"POST {url} -> {r.status_code}\n{r.text[:4000]}")
    return r.json()

def gemini_extract_text(resp_json: Any) -> Optional[str]:
    # Often: {"candidates":[{"content":{"parts":[{"text":"..."}]}}]}
    if not isinstance(resp_json, dict):
        return None
    cands = resp_json.get("candidates")
    if not isinstance(cands, list) or not cands:
        return None
    content = cands[0].get("content")
    if not isinstance(content, dict):
        return None
    parts = content.get("parts")
    if not isinstance(parts, list) or not parts:
        return None
    if isinstance(parts[0], dict) and isinstance(parts[0].get("text"), str):
        return parts[0]["text"]
    return None

# -----------------------------------------------------------------------------
# Main
# -----------------------------------------------------------------------------
def main() -> None:
    ap = argparse.ArgumentParser(description="Unified LLM tester (OpenAI-compat + Claude + Gemini) using raw requests")
    ap.add_argument("provider", choices=sorted(PROVIDERS.keys()))
    ap.add_argument("--list", action="store_true", help="List all models and exit")
    ap.add_argument("--model", help="Override model name/id")
    ap.add_argument("--prompt", default="Reply with exactly: OK", help="Prompt to send")
    ap.add_argument("--max-tokens", type=int, default=64, help="Max output tokens")
    ap.add_argument("--raw", action="store_true", help="Print full JSON response (truncated)")
    args = ap.parse_args()

    cfg = PROVIDERS[args.provider]
    ptype = cfg["type"]

    print(f"==> Provider: {args.provider} ({ptype})")

    # -----------------------------
    # OpenAI-compatible providers
    # -----------------------------
    if ptype == "openai_compat":
        base = cfg["base_url"]
        key_env = cfg.get("api_key_env")
        api_key = os.environ.get(key_env) if key_env else None
        if key_env and not api_key:
            die(f"Missing env var {key_env}")

        headers = build_openai_compat_headers(api_key, cfg.get("extra_headers"))

        models = openai_compat_list_models(base, headers)
        data = models.get("data", []) if isinstance(models, dict) else []
        print(f"Models found: {len(data)}")

        if args.list:
            for m in data:
                mid = m.get("id")
                if mid:
                    print(mid)
            return

        model = args.model or pick_model_from_openai_models(models)
        if not model:
            die("Could not choose a model. Use --model")

        print(f"Using model: {model}")
        out = openai_compat_chat(base, headers, model, args.prompt, args.max_tokens)
        text = extract_openai_chat_text(out)

        if args.raw:
            print(pretty(out))
        else:
            print("Reply:", repr(text) if text is not None else "<no text parsed>")

        print("✅ SUCCESS")
        return

    # -----------------------------
    # Claude (Anthropic)
    # -----------------------------
    if ptype == "anthropic":
        base = cfg["base_url"]
        key_env = cfg["api_key_env"]
        api_key = os.environ.get(key_env)
        if not api_key:
            die(f"Missing env var {key_env}")

        headers = anthropic_headers(api_key, cfg.get("anthropic_version", "2023-06-01"))

        models = anthropic_list_models(base, headers)
        data = models.get("data", []) if isinstance(models, dict) else []
        print(f"Models found: {len(data)}")

        if args.list:
            for m in data:
                mid = m.get("id")
                if mid:
                    print(mid)
            return

        model = args.model or anthropic_pick_model(models)
        if not model:
            die("Could not choose a model. Use --model")

        print(f"Using model: {model}")
        out = anthropic_chat(base, headers, model, args.prompt, args.max_tokens)
        text = anthropic_extract_text(out)

        if args.raw:
            print(pretty(out))
        else:
            print("Reply:", repr(text) if text is not None else "<no text parsed>")

        print("✅ SUCCESS")
        return

    # -----------------------------
    # Gemini (Google AI Studio)
    # -----------------------------
    if ptype == "gemini":
        base = cfg["base_url"]
        key_env = cfg["api_key_env"]
        api_key = os.environ.get(key_env)
        if not api_key:
            die(f"Missing env var {key_env}")

        models = gemini_list_models(base, api_key)
        model_list = models.get("models", []) if isinstance(models, dict) else []
        print(f"Models found: {len(model_list)}")

        if args.list:
            for m in model_list:
                name = m.get("name")
                if name:
                    print(name)
            return

        model = args.model or gemini_pick_model(models)
        if not model:
            die("Could not choose a model. Use --model (e.g., models/gemini-1.5-flash)")

        print(f"Using model: {model}")
        out = gemini_chat(base, api_key, model, args.prompt, args.max_tokens)
        text = gemini_extract_text(out)

        if args.raw:
            print(pretty(out))
        else:
            print("Reply:", repr(text) if text is not None else "<no text parsed>")

        print("✅ SUCCESS")
        return

    die(f"Unknown provider type: {ptype}")

if __name__ == "__main__":
    main()
