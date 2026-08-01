#!/usr/bin/env python3
"""Run-scoped OpenAI-compatible gateway backed by AWS Bedrock Converse.

The worker owns this process and its AWS credentials. Sandboxed submissions see
only the Unix socket (usually through a loopback relay) and a short-lived bearer
token. Request/response bodies are deliberately never logged.
"""
from __future__ import annotations

import argparse
import collections
import contextlib
import hmac
import json
import os
import socketserver
import threading
import time
import uuid
from http.server import BaseHTTPRequestHandler
from pathlib import Path
from typing import Any

MAX_BODY_BYTES = 2 * 1024 * 1024
MAX_OUTPUT_TOKENS = 512
MAX_MESSAGES = 12
MAX_TOOL_RESULT_BYTES = 8 * 1024


class GatewayError(Exception):
    def __init__(self, status: int, message: str):
        super().__init__(message)
        self.status = status


def _text(content: Any, limit: int = 256 * 1024) -> str:
    if content is None:
        return ""
    if isinstance(content, str):
        value = content
    elif isinstance(content, list):
        value = "\n".join(
            str(part.get("text", ""))
            for part in content
            if isinstance(part, dict) and part.get("type") in (None, "text")
        )
    else:
        raise GatewayError(400, "message content must be text")
    raw = value.encode("utf-8")
    return raw[:limit].decode("utf-8", "ignore")


def _merge_message(messages: list[dict[str, Any]], role: str, blocks: list[dict[str, Any]]) -> None:
    if messages and messages[-1]["role"] == role:
        messages[-1]["content"].extend(blocks)
    else:
        messages.append({"role": role, "content": blocks})


def openai_to_bedrock(body: dict[str, Any]) -> tuple[list[dict[str, str]], list[dict[str, Any]], dict[str, Any] | None]:
    source = body.get("messages")
    if not isinstance(source, list) or not source:
        raise GatewayError(400, "messages must be a non-empty array")
    system: list[dict[str, str]] = []
    for message in source:
        if isinstance(message, dict) and message.get("role") in ("system", "developer"):
            value = _text(message.get("content"))
            if value:
                system.append({"text": value})
    source = [
        message for message in source
        if not isinstance(message, dict) or message.get("role") not in ("system", "developer")
    ][-MAX_MESSAGES:]
    # Converse histories must begin with a genuine user turn. Truncating at an
    # assistant/tool boundary would otherwise create an invalid orphaned history.
    while source and (not isinstance(source[0], dict) or source[0].get("role") != "user"):
        source = source[1:]
    if not source:
        raise GatewayError(400, "messages require a user turn")

    messages: list[dict[str, Any]] = []
    for message in source:
        if not isinstance(message, dict):
            raise GatewayError(400, "each message must be an object")
        role = message.get("role")
        if role == "tool":
            tool_id = message.get("tool_call_id")
            if not isinstance(tool_id, str) or not tool_id:
                raise GatewayError(400, "tool messages require tool_call_id")
            block = {"toolResult": {
                "toolUseId": tool_id,
                "content": [{"text": _text(message.get("content"), MAX_TOOL_RESULT_BYTES)}],
                "status": "success",
            }}
            _merge_message(messages, "user", [block])
            continue
        if role not in ("user", "assistant"):
            raise GatewayError(400, f"unsupported message role: {role}")
        blocks: list[dict[str, Any]] = []
        text = _text(message.get("content"))
        if text:
            blocks.append({"text": text})
        if role == "assistant":
            calls = message.get("tool_calls") or []
            if not isinstance(calls, list):
                raise GatewayError(400, "tool_calls must be an array")
            for call in calls:
                fn = call.get("function") if isinstance(call, dict) else None
                if not isinstance(fn, dict) or not isinstance(fn.get("name"), str):
                    raise GatewayError(400, "invalid tool call")
                try:
                    arguments = json.loads(fn.get("arguments") or "{}")
                except json.JSONDecodeError as exc:
                    raise GatewayError(400, "tool arguments must be valid JSON") from exc
                blocks.append({"toolUse": {
                    "toolUseId": str(call.get("id") or uuid.uuid4().hex),
                    "name": fn["name"],
                    "input": arguments,
                }})
        if not blocks:
            blocks.append({"text": " "})
        _merge_message(messages, role, blocks)

    tools = body.get("tools") or []
    tool_config: dict[str, Any] | None = None
    if tools:
        if not isinstance(tools, list):
            raise GatewayError(400, "tools must be an array")
        converted = []
        for tool in tools:
            fn = tool.get("function") if isinstance(tool, dict) else None
            if not isinstance(tool, dict) or tool.get("type") != "function" or not isinstance(fn, dict):
                raise GatewayError(400, "only function tools are supported")
            name = fn.get("name")
            if not isinstance(name, str) or not name:
                raise GatewayError(400, "tool name is required")
            converted.append({"toolSpec": {
                "name": name,
                "description": str(fn.get("description") or name)[:1024],
                "inputSchema": {"json": fn.get("parameters") or {"type": "object"}},
            }})
        tool_config = {"tools": converted}
        choice = body.get("tool_choice")
        if choice == "required":
            tool_config["toolChoice"] = {"any": {}}
        elif isinstance(choice, dict):
            fn = choice.get("function") or {}
            if isinstance(fn.get("name"), str):
                tool_config["toolChoice"] = {"tool": {"name": fn["name"]}}
        elif choice in (None, "auto"):
            tool_config["toolChoice"] = {"auto": {}}
    return system, messages, tool_config


def bedrock_to_openai(response: dict[str, Any], model: str) -> dict[str, Any]:
    output = ((response.get("output") or {}).get("message") or {})
    text_parts: list[str] = []
    tool_calls: list[dict[str, Any]] = []
    for block in output.get("content") or []:
        if "text" in block:
            text_parts.append(str(block["text"]))
        if "toolUse" in block:
            tool = block["toolUse"]
            tool_calls.append({
                "id": str(tool.get("toolUseId") or uuid.uuid4().hex),
                "type": "function",
                "function": {
                    "name": str(tool.get("name") or "tool"),
                    "arguments": json.dumps(tool.get("input") or {}, separators=(",", ":")),
                },
            })
    message: dict[str, Any] = {"role": "assistant", "content": "".join(text_parts) or None}
    if tool_calls:
        message["tool_calls"] = tool_calls
    stop = response.get("stopReason")
    finish = "tool_calls" if tool_calls else "length" if stop == "max_tokens" else "stop"
    usage = response.get("usage") or {}
    return {
        "id": f"bedrock-{uuid.uuid4().hex}",
        "object": "chat.completion",
        "created": int(time.time()),
        "model": model,
        "choices": [{"index": 0, "message": message, "finish_reason": finish}],
        "usage": {
            "prompt_tokens": int(usage.get("inputTokens") or 0),
            "completion_tokens": int(usage.get("outputTokens") or 0),
            "total_tokens": int(usage.get("totalTokens") or 0),
        },
    }


def openai_to_responses(
    body: dict[str, Any],
) -> tuple[str, list[dict[str, Any]], list[dict[str, Any]], Any | None]:
    """Translate Chat Completions input to the Bedrock Mantle Responses API."""
    source = body.get("messages")
    if not isinstance(source, list) or not source:
        raise GatewayError(400, "messages must be a non-empty array")

    instructions: list[str] = []
    for message in source:
        if isinstance(message, dict) and message.get("role") in ("system", "developer"):
            value = _text(message.get("content"))
            if value:
                instructions.append(value)

    source = [
        message for message in source
        if not isinstance(message, dict) or message.get("role") not in ("system", "developer")
    ][-MAX_MESSAGES:]
    while source and (not isinstance(source[0], dict) or source[0].get("role") != "user"):
        source = source[1:]
    if not source:
        raise GatewayError(400, "messages require a user turn")

    inputs: list[dict[str, Any]] = []
    for message in source:
        if not isinstance(message, dict):
            raise GatewayError(400, "each message must be an object")
        role = message.get("role")
        if role == "tool":
            call_id = message.get("tool_call_id")
            if not isinstance(call_id, str) or not call_id:
                raise GatewayError(400, "tool messages require tool_call_id")
            inputs.append({
                "type": "function_call_output",
                "call_id": call_id,
                "output": _text(message.get("content"), MAX_TOOL_RESULT_BYTES),
            })
            continue
        if role not in ("user", "assistant"):
            raise GatewayError(400, f"unsupported message role: {role}")
        value = _text(message.get("content"))
        if value:
            inputs.append({"type": "message", "role": role, "content": value})
        if role == "assistant":
            calls = message.get("tool_calls") or []
            if not isinstance(calls, list):
                raise GatewayError(400, "tool_calls must be an array")
            for call in calls:
                fn = call.get("function") if isinstance(call, dict) else None
                if not isinstance(fn, dict) or not isinstance(fn.get("name"), str):
                    raise GatewayError(400, "invalid tool call")
                arguments = fn.get("arguments") or "{}"
                if not isinstance(arguments, str):
                    raise GatewayError(400, "tool arguments must be a JSON string")
                try:
                    json.loads(arguments)
                except json.JSONDecodeError as exc:
                    raise GatewayError(400, "tool arguments must be valid JSON") from exc
                inputs.append({
                    "type": "function_call",
                    "call_id": str(call.get("id") or f"call_{uuid.uuid4().hex}"),
                    "name": fn["name"],
                    "arguments": arguments,
                })

    tools = body.get("tools") or []
    converted_tools: list[dict[str, Any]] = []
    if not isinstance(tools, list):
        raise GatewayError(400, "tools must be an array")
    for tool in tools:
        fn = tool.get("function") if isinstance(tool, dict) else None
        if not isinstance(tool, dict) or tool.get("type") != "function" or not isinstance(fn, dict):
            raise GatewayError(400, "only function tools are supported")
        name = fn.get("name")
        if not isinstance(name, str) or not name:
            raise GatewayError(400, "tool name is required")
        converted_tools.append({
            "type": "function",
            "name": name,
            "description": str(fn.get("description") or name)[:1024],
            "parameters": fn.get("parameters") or {"type": "object"},
        })

    tool_choice: Any | None = None
    choice = body.get("tool_choice")
    if converted_tools and choice in (None, "auto", "required", "none"):
        tool_choice = choice or "auto"
    elif converted_tools and isinstance(choice, dict):
        fn = choice.get("function") or {}
        if isinstance(fn.get("name"), str):
            tool_choice = {"type": "function", "name": fn["name"]}
    return "\n\n".join(instructions), inputs, converted_tools, tool_choice


def responses_to_openai(response: dict[str, Any], model: str) -> dict[str, Any]:
    text_parts: list[str] = []
    tool_calls: list[dict[str, Any]] = []
    for item in response.get("output") or []:
        if not isinstance(item, dict):
            continue
        if item.get("type") == "message":
            for block in item.get("content") or []:
                if isinstance(block, dict) and block.get("type") == "output_text":
                    text_parts.append(str(block.get("text") or ""))
                elif isinstance(block, dict) and block.get("type") == "refusal":
                    text_parts.append(str(block.get("refusal") or ""))
        elif item.get("type") == "function_call":
            tool_calls.append({
                "id": str(item.get("call_id") or item.get("id") or f"call_{uuid.uuid4().hex}"),
                "type": "function",
                "function": {
                    "name": str(item.get("name") or "tool"),
                    "arguments": str(item.get("arguments") or "{}"),
                },
            })
    message: dict[str, Any] = {"role": "assistant", "content": "".join(text_parts) or None}
    if tool_calls:
        message["tool_calls"] = tool_calls
    incomplete = response.get("incomplete_details") or {}
    finish = (
        "tool_calls" if tool_calls
        else "length" if incomplete.get("reason") == "max_output_tokens"
        else "stop"
    )
    usage = response.get("usage") or {}
    return {
        "id": str(response.get("id") or f"bedrock-{uuid.uuid4().hex}"),
        "object": "chat.completion",
        "created": int(response.get("created_at") or time.time()),
        "model": model,
        "choices": [{"index": 0, "message": message, "finish_reason": finish}],
        "usage": {
            "prompt_tokens": int(usage.get("input_tokens") or 0),
            "completion_tokens": int(usage.get("output_tokens") or 0),
            "total_tokens": int(usage.get("total_tokens") or 0),
        },
    }


def responses_input_to_chat(
    instructions: str, inputs: list[dict[str, Any]]
) -> list[dict[str, Any]]:
    """Rebuild sanitized Chat Completions messages from validated Responses input."""
    messages: list[dict[str, Any]] = []
    if instructions:
        messages.append({"role": "system", "content": instructions})
    for item in inputs:
        kind = item.get("type")
        if kind == "message":
            messages.append({"role": item["role"], "content": item["content"]})
        elif kind == "function_call":
            call = {
                "id": item["call_id"],
                "type": "function",
                "function": {"name": item["name"], "arguments": item["arguments"]},
            }
            if messages and messages[-1].get("role") == "assistant":
                messages[-1].setdefault("tool_calls", []).append(call)
            else:
                messages.append({"role": "assistant", "content": None, "tool_calls": [call]})
        elif kind == "function_call_output":
            messages.append({
                "role": "tool",
                "tool_call_id": item["call_id"],
                "content": item["output"],
            })
    return messages


def responses_tools_to_chat(tools: list[dict[str, Any]]) -> list[dict[str, Any]]:
    return [{
        "type": "function",
        "function": {
            "name": tool["name"],
            "description": tool.get("description") or tool["name"],
            "parameters": tool.get("parameters") or {"type": "object"},
        },
    } for tool in tools]


def responses_tool_choice_to_chat(choice: Any | None) -> Any | None:
    if isinstance(choice, dict) and choice.get("type") == "function":
        return {"type": "function", "function": {"name": choice["name"]}}
    return choice


class Stats:
    def __init__(self) -> None:
        self.lock = threading.Lock()
        self.started = time.monotonic()
        self.completions: collections.deque[float] = collections.deque(maxlen=10_000)
        self.requests = self.errors = self.input_tokens = self.output_tokens = 0
        self.latency_ms = 0

    def record(self, response: dict[str, Any] | None, latency_ms: int, error: bool = False) -> None:
        with self.lock:
            self.requests += 1
            self.latency_ms += latency_ms
            if error:
                self.errors += 1
                return
            usage = (response or {}).get("usage") or {}
            self.input_tokens += int(
                usage.get("inputTokens") or usage.get("input_tokens")
                or usage.get("prompt_tokens") or 0
            )
            self.output_tokens += int(
                usage.get("outputTokens") or usage.get("output_tokens")
                or usage.get("completion_tokens") or 0
            )
            self.completions.append(time.monotonic())

    def snapshot(self) -> dict[str, Any]:
        now = time.monotonic()
        with self.lock:
            while self.completions and self.completions[0] < now - 120:
                self.completions.popleft()
            return {
                "requests": self.requests,
                "errors": self.errors,
                "input_tokens": self.input_tokens,
                "output_tokens": self.output_tokens,
                "latency_ms": self.latency_ms,
                "completed_last_30s": sum(t >= now - 30 for t in self.completions),
                "completion_ages_seconds": [round(now - t, 3) for t in self.completions],
                "uptime_seconds": round(now - self.started, 3),
            }


class ThreadingUnixServer(socketserver.ThreadingMixIn, socketserver.UnixStreamServer):
    daemon_threads = True


class GatewayHandler(BaseHTTPRequestHandler):
    server_version = "ExposureBedrockGateway/1"

    def log_message(self, _format: str, *_args: Any) -> None:
        return

    def _json(self, status: int, payload: Any, headers: dict[str, str] | None = None) -> None:
        raw = json.dumps(payload, separators=(",", ":")).encode()
        self.send_response(status)
        self.send_header("Content-Type", "application/json")
        self.send_header("Content-Length", str(len(raw)))
        for key, value in (headers or {}).items():
            self.send_header(key, value)
        self.end_headers()
        self.wfile.write(raw)

    def _authorized(self) -> bool:
        supplied = self.headers.get("Authorization", "").removeprefix("Bearer ")
        return bool(supplied) and hmac.compare_digest(supplied, self.server.gateway_token)

    def do_GET(self) -> None:
        if self.path == "/health":
            self._json(200, {"ok": True})
        elif self.path == "/metrics" and self._authorized():
            self._json(200, self.server.stats.snapshot())
        else:
            self._json(404 if self._authorized() else 401, {"error": {"message": "not found"}})

    def do_POST(self) -> None:
        if not self._authorized():
            self._json(401, {"error": {"message": "invalid gateway token"}})
            return
        if self.path.rstrip("/") != "/v1/chat/completions":
            self._json(404, {"error": {"message": "not found"}})
            return
        try:
            length = int(self.headers.get("Content-Length", "0"))
            if not 0 < length <= MAX_BODY_BYTES:
                raise GatewayError(413, "request body is empty or too large")
            body = json.loads(self.rfile.read(length))
            if not isinstance(body, dict):
                raise GatewayError(400, "request body must be an object")
            if body.get("stream"):
                raise GatewayError(400, "streaming is not supported by this benchmark gateway")
            requested_tokens = body.get("max_completion_tokens", body.get("max_tokens"))
            max_tokens = min(MAX_OUTPUT_TOKENS, max(1, int(requested_tokens or MAX_OUTPUT_TOKENS)))
            if not self.server.slots.acquire(timeout=2):
                raise GatewayError(429, "gateway concurrency is full")
            started = time.monotonic()
            try:
                if self.server.api_style == "responses":
                    instructions, inputs, tools, tool_choice = openai_to_responses(body)
                    request: dict[str, Any] = {
                        "model": self.server.model_id,
                        "input": inputs,
                        "max_output_tokens": max_tokens,
                        "reasoning": {"effort": self.server.reasoning_effort},
                    }
                    if instructions:
                        request["instructions"] = instructions
                    if tools:
                        request["tools"] = tools
                        request["tool_choice"] = tool_choice
                    result = self.server.client.responses.create(**request)
                    response = result.model_dump(exclude_none=True)
                elif self.server.api_style == "chat":
                    instructions, inputs, tools, tool_choice = openai_to_responses(body)
                    request = {
                        "model": self.server.model_id,
                        "messages": responses_input_to_chat(instructions, inputs),
                        "max_completion_tokens": max_tokens,
                        "reasoning_effort": self.server.reasoning_effort,
                    }
                    if tools:
                        request["tools"] = responses_tools_to_chat(tools)
                        request["tool_choice"] = responses_tool_choice_to_chat(tool_choice)
                    result = self.server.client.chat.completions.create(**request)
                    response = result.model_dump(exclude_none=True)
                else:
                    system, messages, tool_config = openai_to_bedrock(body)
                    request = {
                        "modelId": self.server.model_id,
                        "messages": messages,
                        "inferenceConfig": {"maxTokens": max_tokens},
                        "performanceConfig": {"latency": self.server.latency},
                        "requestMetadata": {
                            "feature": "agentic-harness",
                            "version": self.server.benchmark_version,
                        },
                    }
                    if system:
                        request["system"] = system
                    if tool_config:
                        request["toolConfig"] = tool_config
                    response = self.server.client.converse(**request)
            finally:
                self.server.slots.release()
            latency = int((time.monotonic() - started) * 1000)
            self.server.stats.record(response, latency)
            metrics = response.get("metrics") or {}
            usage = response.get("usage") or {}
            converted = (
                responses_to_openai(response, self.server.profile_name)
                if self.server.api_style == "responses"
                else bedrock_to_openai(response, self.server.profile_name)
                if self.server.api_style == "converse"
                else {**response, "model": self.server.profile_name}
            )
            self._json(200, converted, {
                "X-Bedrock-Latency-Ms": str(metrics.get("latencyMs") or latency),
                "X-Bedrock-Input-Tokens": str(
                    usage.get("inputTokens") or usage.get("input_tokens")
                    or usage.get("prompt_tokens") or 0
                ),
                "X-Bedrock-Output-Tokens": str(
                    usage.get("outputTokens") or usage.get("output_tokens")
                    or usage.get("completion_tokens") or 0
                ),
            })
        except GatewayError as exc:
            self.server.stats.record(None, 0, error=True)
            self._json(exc.status, {"error": {"message": str(exc), "type": "invalid_request_error"}})
        except (ValueError, TypeError, json.JSONDecodeError) as exc:
            self.server.stats.record(None, 0, error=True)
            self._json(400, {"error": {"message": str(exc), "type": "invalid_request_error"}})
        except Exception as exc:
            self.server.stats.record(None, 0, error=True)
            upstream_response = getattr(exc, "response", None)
            code = (
                upstream_response.get("Error", {}).get("Code", "bedrock_error")
                if isinstance(upstream_response, dict)
                else "bedrock_error"
            )
            upstream_status = getattr(exc, "status_code", None)
            if code == "bedrock_error":
                code = getattr(exc, "code", None) or type(exc).__name__
            status = 429 if upstream_status == 429 or code in ("ThrottlingException", "TooManyRequestsException") else 502
            self._json(status, {"error": {"message": f"Bedrock request failed: {code}", "type": "api_error"}})


def run(socket_path: Path) -> None:
    model_id = (
        os.environ.get("BEDROCK_MODEL_ID", "").strip()
        or os.environ.get("BEDROCK_INFERENCE_PROFILE_ARN", "").strip()
    )
    token = os.environ.get("BEDROCK_GATEWAY_TOKEN", "").strip()
    region = os.environ.get("AWS_REGION", "").strip()
    if not model_id or not token or not region:
        raise SystemExit("AWS_REGION, BEDROCK_MODEL_ID and BEDROCK_GATEWAY_TOKEN are required")
    max_concurrency = max(1, min(128, int(os.environ.get("BEDROCK_MAX_CONCURRENCY", "32"))))
    if model_id.startswith(("openai.", "xai.")):
        try:
            from openai import OpenAI
        except ImportError as exc:
            raise SystemExit("Install worker/requirements.txt before starting the gateway") from exc
        api_key = os.environ.get("AWS_BEARER_TOKEN_BEDROCK", "").strip()
        if not api_key:
            raise SystemExit("AWS_BEARER_TOKEN_BEDROCK is required for Bedrock Mantle")
        client = OpenAI(
            api_key=api_key,
            base_url=f"https://bedrock-mantle.{region}.api.aws/openai/v1",
            timeout=15,
            max_retries=1,
        )
        api_style = "chat" if model_id.startswith("xai.") else "responses"
    else:
        try:
            import boto3
            from botocore.config import Config
        except ImportError as exc:
            raise SystemExit("Install worker/requirements.txt before starting the gateway") from exc
        client = boto3.client(
            "bedrock-runtime",
            region_name=region,
            config=Config(
                connect_timeout=3,
                read_timeout=15,
                max_pool_connections=max(64, max_concurrency),
                retries={"mode": "standard", "max_attempts": 2},
            ),
        )
        api_style = "converse"
    socket_path.parent.mkdir(parents=True, exist_ok=True)
    with contextlib.suppress(FileNotFoundError):
        socket_path.unlink()
    server = ThreadingUnixServer(str(socket_path), GatewayHandler)
    server.client = client
    server.api_style = api_style
    server.model_id = model_id
    server.reasoning_effort = os.environ.get("BEDROCK_REASONING_EFFORT", "none").strip().lower()
    if server.reasoning_effort not in ("none", "low", "medium", "high"):
        raise SystemExit("BEDROCK_REASONING_EFFORT must be none, low, medium, or high")
    server.latency = os.environ.get("BEDROCK_LATENCY", "standard").strip().lower()
    if server.latency not in ("standard", "optimized"):
        raise SystemExit("BEDROCK_LATENCY must be standard or optimized")
    server.profile_name = os.environ.get("BEDROCK_PROFILE_NAME", "bedrock-harness")[:120]
    server.benchmark_version = os.environ.get("HARNESS_BENCHMARK_VERSION", "harness-2026-sprint-v2")
    server.gateway_token = token
    server.slots = threading.BoundedSemaphore(max_concurrency)
    server.stats = Stats()
    try:
        server.serve_forever(poll_interval=0.2)
    finally:
        server.server_close()
        with contextlib.suppress(Exception):
            client.close()
        with contextlib.suppress(FileNotFoundError):
            socket_path.unlink()


if __name__ == "__main__":
    parser = argparse.ArgumentParser()
    parser.add_argument("--socket", type=Path, required=True)
    args = parser.parse_args()
    run(args.socket)
