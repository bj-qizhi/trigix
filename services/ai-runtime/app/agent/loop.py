# Copyright © 2026 北京祺智科技有限公司. All rights reserved.
# https://www.qzso.com/ · managecode@gmail.com

"""The agent tool-use loop.

The LLM is behind the `LLM` protocol so the loop can be driven by a real
Anthropic model in production and by a scripted fake model in tests — the loop
logic (tool dispatch, result feedback, iteration, termination) is exercised
without any API key.
"""

from __future__ import annotations

import asyncio
import json
import random
from dataclasses import dataclass, field
from typing import Any, Protocol

from .tools import Tool

# Transient LLM API failures worth retrying. Both the Anthropic and OpenAI SDKs
# expose the HTTP code on `.status_code`; connection/timeout errors carry none.
_RETRYABLE_STATUS = frozenset({408, 409, 429, 500, 502, 503, 504, 529})


def _is_transient(exc: Exception) -> bool:
    status = getattr(exc, "status_code", None)
    if isinstance(status, int):
        return status in _RETRYABLE_STATUS
    name = type(exc).__name__
    return any(k in name for k in ("Timeout", "Connection", "Overloaded"))


async def _call_with_retries(fn, *, max_attempts: int = 3, base_delay: float = 0.5):
    """Run a blocking SDK call off the event loop, retrying transient API errors
    (HTTP 429/5xx, connection/timeout) with exponential backoff + jitter.
    Non-transient errors propagate immediately."""
    attempt = 0
    while True:
        try:
            return await asyncio.to_thread(fn)
        except Exception as exc:
            attempt += 1
            if attempt >= max_attempts or not _is_transient(exc):
                raise
            delay = base_delay * (2 ** (attempt - 1))
            await asyncio.sleep(delay + random.uniform(0, delay * 0.25))


@dataclass
class ToolCall:
    id: str
    name: str
    input: dict


@dataclass
class LLMResponse:
    # When the model is done, `text` is set and `tool_calls` is empty.
    text: str | None
    tool_calls: list[ToolCall]
    # Raw assistant content blocks, appended verbatim to the message history.
    assistant_content: list
    # {"input_tokens": int, "output_tokens": int} for this single turn.
    usage: dict = field(default_factory=dict)


class LLM(Protocol):
    async def respond(
        self, system: str, messages: list, tool_schemas: list
    ) -> LLMResponse: ...


@dataclass
class AgentResult:
    output: str
    steps: list = field(default_factory=list)
    # Token usage summed across every model turn in the loop.
    usage: dict = field(default_factory=lambda: {"input_tokens": 0, "output_tokens": 0})


async def run_agent_loop(
    llm: LLM,
    system: str,
    user_message: str,
    tools: list[Tool],
    max_iterations: int = 6,
) -> AgentResult:
    """Drive the model: while it requests tools, execute them and feed the
    results back; stop when it returns a final text answer or the iteration
    budget is exhausted."""
    tool_map = {t.name: t for t in tools}
    schemas = [
        {"name": t.name, "description": t.description, "input_schema": t.input_schema}
        for t in tools
    ]
    messages: list[dict[str, Any]] = [{"role": "user", "content": user_message}]
    steps: list[dict] = []
    usage = {"input_tokens": 0, "output_tokens": 0}

    def _accumulate(turn: dict) -> None:
        usage["input_tokens"] += int(turn.get("input_tokens", 0) or 0)
        usage["output_tokens"] += int(turn.get("output_tokens", 0) or 0)

    for _ in range(max(1, max_iterations)):
        resp = await llm.respond(system, messages, schemas)
        _accumulate(resp.usage)
        if not resp.tool_calls:
            return AgentResult(output=resp.text or "", steps=steps, usage=usage)

        messages.append({"role": "assistant", "content": resp.assistant_content})

        async def _run_one(call: ToolCall) -> str:
            tool = tool_map.get(call.name)
            if tool is None:
                return f"error: unknown tool '{call.name}'"
            try:
                return await tool.run(call.input)
            except Exception as exc:  # surface tool errors back to the model
                return f"error: {exc}"

        # A turn may request several tools at once; run them concurrently
        # (gather preserves order) instead of awaiting each in sequence.
        outputs = await asyncio.gather(*(_run_one(c) for c in resp.tool_calls))

        results = []
        for call, content in zip(resp.tool_calls, outputs):
            steps.append({"tool": call.name, "input": call.input, "output": content})
            results.append(
                {"type": "tool_result", "tool_use_id": call.id, "content": content}
            )
        messages.append({"role": "user", "content": results})

    return AgentResult(
        output="(agent reached the maximum number of steps without a final answer)",
        steps=steps,
        usage=usage,
    )


class AnthropicLLM:
    """Production `LLM` backed by the Anthropic messages API with tool use."""

    def __init__(self, client: Any, model: str, max_tokens: int) -> None:
        self._client = client
        self._model = model
        self._max_tokens = max_tokens

    async def respond(
        self, system: str, messages: list, tool_schemas: list
    ) -> LLMResponse:
        kwargs: dict[str, Any] = {
            "model": self._model,
            "max_tokens": self._max_tokens,
            "messages": messages,
        }
        # Prompt caching: mark the static prefix (tools + system, sent
        # identically on every loop turn) as a cache breakpoint so turns 2..N
        # reuse it. The breakpoint on `system` also caches the tools that
        # precede it; with no system, anchor it on the last tool instead. Below
        # the model's minimum cacheable size Anthropic ignores the marker, so
        # this is always safe.
        if system:
            kwargs["system"] = [
                {"type": "text", "text": system, "cache_control": {"type": "ephemeral"}}
            ]
        if tool_schemas:
            tools = list(tool_schemas)
            if not system:
                tools[-1] = {**tools[-1], "cache_control": {"type": "ephemeral"}}
            kwargs["tools"] = tools
        # The SDK call is synchronous; run it off the event loop, with retries.
        msg = await _call_with_retries(lambda: self._client.messages.create(**kwargs))

        usage = {}
        u = getattr(msg, "usage", None)
        if u is not None:
            usage = {
                "input_tokens": getattr(u, "input_tokens", 0) or 0,
                "output_tokens": getattr(u, "output_tokens", 0) or 0,
            }

        text_parts: list[str] = []
        tool_calls: list[ToolCall] = []
        assistant_content: list[dict] = []
        for block in msg.content:
            if block.type == "text":
                text_parts.append(block.text)
                assistant_content.append({"type": "text", "text": block.text})
            elif block.type == "tool_use":
                tool_calls.append(ToolCall(id=block.id, name=block.name, input=block.input))
                assistant_content.append(
                    {
                        "type": "tool_use",
                        "id": block.id,
                        "name": block.name,
                        "input": block.input,
                    }
                )

        if msg.stop_reason == "tool_use":
            return LLMResponse(
                text=None, tool_calls=tool_calls, assistant_content=assistant_content, usage=usage
            )
        return LLMResponse(
            text="".join(text_parts), tool_calls=[], assistant_content=assistant_content, usage=usage
        )


# ── OpenAI-compatible backend ───────────────────────────────────────────────
#
# Any chat-completions API that speaks the OpenAI tool-calling dialect: OpenAI
# itself, Qwen/DashScope, DeepSeek, Zhipu, Moonshot, MiniMax, or a self-hosted
# vLLM/Ollama gateway. This is what makes the agent usable in a China private
# deployment, where the Anthropic API is not reachable.


def _to_openai_tools(tool_schemas: list) -> list:
    return [
        {
            "type": "function",
            "function": {
                "name": t["name"],
                "description": t.get("description", ""),
                "parameters": t.get("input_schema", {"type": "object", "properties": {}}),
            },
        }
        for t in tool_schemas
    ]


def _to_openai_messages(system: str, messages: list) -> list:
    """Translate the loop's Anthropic-shaped history into OpenAI chat messages.

    Keeping the loop's internal format unchanged means AnthropicLLM and the
    existing tests are untouched; only this adapter does the translation.
    """
    out: list[dict[str, Any]] = []
    if system:
        out.append({"role": "system", "content": system})
    for m in messages:
        role = m.get("role")
        content = m.get("content")
        if role == "user" and isinstance(content, str):
            out.append({"role": "user", "content": content})
        elif role == "user" and isinstance(content, list):
            # Anthropic tool_result blocks → OpenAI 'tool' role messages.
            for block in content:
                if block.get("type") == "tool_result":
                    out.append(
                        {
                            "role": "tool",
                            "tool_call_id": block.get("tool_use_id", ""),
                            "content": block.get("content", ""),
                        }
                    )
        elif role == "assistant" and isinstance(content, list):
            text_parts: list[str] = []
            tool_calls: list[dict] = []
            for block in content:
                if block.get("type") == "text":
                    text_parts.append(block.get("text", ""))
                elif block.get("type") == "tool_use":
                    tool_calls.append(
                        {
                            "id": block.get("id", ""),
                            "type": "function",
                            "function": {
                                "name": block.get("name", ""),
                                "arguments": json.dumps(block.get("input", {})),
                            },
                        }
                    )
            msg: dict[str, Any] = {"role": "assistant", "content": "".join(text_parts) or None}
            if tool_calls:
                msg["tool_calls"] = tool_calls
            out.append(msg)
        elif role == "assistant" and isinstance(content, str):
            out.append({"role": "assistant", "content": content})
    return out


class OpenAICompatLLM:
    """Production `LLM` backed by an OpenAI-compatible chat-completions API."""

    def __init__(self, client: Any, model: str, max_tokens: int) -> None:
        self._client = client
        self._model = model
        self._max_tokens = max_tokens

    async def respond(
        self, system: str, messages: list, tool_schemas: list
    ) -> LLMResponse:
        kwargs: dict[str, Any] = {
            "model": self._model,
            "max_tokens": self._max_tokens,
            "messages": _to_openai_messages(system, messages),
        }
        if tool_schemas:
            kwargs["tools"] = _to_openai_tools(tool_schemas)
        # The SDK call is synchronous; run it off the event loop, with retries.
        completion = await _call_with_retries(
            lambda: self._client.chat.completions.create(**kwargs)
        )

        usage = {}
        u = getattr(completion, "usage", None)
        if u is not None:
            usage = {
                "input_tokens": getattr(u, "prompt_tokens", 0) or 0,
                "output_tokens": getattr(u, "completion_tokens", 0) or 0,
            }

        choice = completion.choices[0].message
        text = choice.content or ""
        raw_calls = getattr(choice, "tool_calls", None) or []

        tool_calls: list[ToolCall] = []
        assistant_content: list[dict] = []
        if text:
            assistant_content.append({"type": "text", "text": text})
        for tc in raw_calls:
            args = tc.function.arguments
            try:
                parsed = json.loads(args) if isinstance(args, str) and args else (args or {})
            except json.JSONDecodeError:
                parsed = {}
            tool_calls.append(ToolCall(id=tc.id, name=tc.function.name, input=parsed))
            assistant_content.append(
                {"type": "tool_use", "id": tc.id, "name": tc.function.name, "input": parsed}
            )

        if tool_calls:
            return LLMResponse(
                text=None, tool_calls=tool_calls, assistant_content=assistant_content, usage=usage
            )
        return LLMResponse(
            text=text,
            tool_calls=[],
            assistant_content=assistant_content or [{"type": "text", "text": text}],
            usage=usage,
        )
