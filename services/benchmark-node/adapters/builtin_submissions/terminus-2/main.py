"""Minimal RAM-bench entrypoint for the Frontier-only Terminus-2 harness."""
from __future__ import annotations

import json
import os
import sys
import urllib.request


def main() -> None:
    payload = json.dumps({
        "model": os.environ["HARNESS_LLM_MODEL"],
        "messages": [{"role": "user", "content": sys.stdin.read()}],
        "temperature": 0,
        "max_tokens": 120,
    }).encode()
    request = urllib.request.Request(
        os.environ["OPENAI_BASE_URL"].rstrip("/") + "/chat/completions",
        data=payload,
        headers={
            "Authorization": "Bearer " + os.environ["OPENAI_API_KEY"],
            "Content-Type": "application/json",
        },
    )
    with urllib.request.urlopen(request, timeout=8) as response:
        result = json.load(response)
    content = result["choices"][0]["message"]["content"]
    if not isinstance(content, str) or not content.strip():
        raise RuntimeError("model returned no text")
    print(content)


if __name__ == "__main__":
    main()
