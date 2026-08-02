"""Minimal RAM-bench entrypoint for the Frontier-only Terminus-2 harness."""
import os
import sys

from openai import OpenAI


def main() -> None:
    response = OpenAI().chat.completions.create(
        model=os.environ["HARNESS_LLM_MODEL"],
        messages=[{"role": "user", "content": sys.stdin.read()}],
        max_tokens=120,
    )
    print(response.choices[0].message.content or "done")


if __name__ == "__main__":
    main()
