# Copyright © 2026 北京祺智科技有限公司. All rights reserved.
# https://www.qzso.com/ · managecode@gmail.com

"""The agent tool-use loop.

The LLM is behind the `LLM` protocol so the loop can be driven by a real
Anthropic model in production and by a scripted fake model in tests — the loop
logic (tool dispatch, result feedback, iteration, termination) is exercised
without any API key.
"""

from __future__ import annotations

from dataclasses import dataclass, field
from typing import Any, Protocol

from .tools import Tool


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


class LLM(Protocol):
    async def respond(
        self, system: str, messages: list, tool_schemas: list
    ) -> LLMResponse: ...


@dataclass
class AgentResult:
    output: str
    steps: list = field(default_factory=list)


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

    for _ in range(max(1, max_iterations)):
        resp = await llm.respond(system, messages, schemas)
        if not resp.tool_calls:
            return AgentResult(output=resp.text or "", steps=steps)

        messages.append({"role": "assistant", "content": resp.assistant_content})
        results = []
        for call in resp.tool_calls:
            tool = tool_map.get(call.name)
            if tool is None:
                content = f"error: unknown tool '{call.name}'"
            else:
                try:
                    content = await tool.run(call.input)
                except Exception as exc:  # surface tool errors back to the model
                    content = f"error: {exc}"
            steps.append({"tool": call.name, "input": call.input, "output": content})
            results.append(
                {"type": "tool_result", "tool_use_id": call.id, "content": content}
            )
        messages.append({"role": "user", "content": results})

    return AgentResult(
        output="(agent reached the maximum number of steps without a final answer)",
        steps=steps,
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
            "system": system,
            "messages": messages,
        }
        if tool_schemas:
            kwargs["tools"] = tool_schemas
        msg = self._client.messages.create(**kwargs)

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
            return LLMResponse(text=None, tool_calls=tool_calls, assistant_content=assistant_content)
        return LLMResponse(
            text="".join(text_parts), tool_calls=[], assistant_content=assistant_content
        )
