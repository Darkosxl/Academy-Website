import os
import sys

from openai import OpenAI


def main() -> None:
    prompt = sys.stdin.read()
    response = OpenAI().chat.completions.create(
        model=os.environ["HARNESS_LLM_MODEL"],
        messages=[{"role": "user", "content": prompt}],
        max_tokens=256,
    )
    print(response.choices[0].message.content or "done")


if __name__ == "__main__":
    main()
