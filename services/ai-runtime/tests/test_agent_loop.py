# Copyright © 2026 北京祺智科技有限公司. All rights reserved.
# https://www.qzso.com/ · managecode@gmail.com

"""Drive the agent loop with a scripted fake LLM — no Anthropic key required."""

import asyncio
import time

import pytest

from app.agent.loop import (
    AnthropicLLM,
    LLMResponse,
    ToolCall,
    _call_with_retries,
    _is_transient,
    run_agent_loop,
)
from app.agent.tools import Tool, build_tools


class FakeLLM:
    def __init__(self, scripted: list[LLMResponse]) -> None:
        self._scripted = scripted
        self.calls: list[list] = []

    async def respond(self, system, messages, tool_schemas):
        self.calls.append(list(messages))
        return self._scripted.pop(0)


def _tool_use(call_id, name, inp):
    return LLMResponse(
        text=None,
        tool_calls=[ToolCall(id=call_id, name=name, input=inp)],
        assistant_content=[{"type": "tool_use", "id": call_id, "name": name, "input": inp}],
    )


def _final(text):
    return LLMResponse(text=text, tool_calls=[], assistant_content=[{"type": "text", "text": text}])


async def test_loop_executes_tool_then_finishes():
    tools = build_tools(["calculator"])
    llm = FakeLLM([_tool_use("t1", "calculator", {"expression": "6 * 7"}), _final("The answer is 42")])

    result = await run_agent_loop(llm, "sys", "what is 6*7?", tools, max_iterations=5)

    assert result.output == "The answer is 42"
    assert len(result.steps) == 1
    assert result.steps[0]["tool"] == "calculator"
    assert result.steps[0]["output"] == "42"
    # The model's 2nd turn must have been fed the tool_result.
    last_msg = llm.calls[1][-1]
    assert last_msg["role"] == "user"
    assert last_msg["content"][0]["type"] == "tool_result"
    assert last_msg["content"][0]["content"] == "42"


async def test_loop_with_no_tools_is_single_shot():
    llm = FakeLLM([_final("hello")])
    result = await run_agent_loop(llm, "sys", "hi", tools=[], max_iterations=5)
    assert result.output == "hello"
    assert result.steps == []
    assert len(llm.calls) == 1


async def test_unknown_tool_is_reported_back():
    llm = FakeLLM([_tool_use("t1", "ghost", {}), _final("done")])
    result = await run_agent_loop(llm, "sys", "x", tools=build_tools(["calculator"]), max_iterations=5)
    assert result.output == "done"
    assert "unknown tool" in result.steps[0]["output"]


async def test_tool_error_is_fed_back_not_raised():
    llm = FakeLLM([_tool_use("t1", "calculator", {"expression": "1 + "}), _final("recovered")])
    result = await run_agent_loop(llm, "sys", "x", tools=build_tools(["calculator"]), max_iterations=5)
    assert result.output == "recovered"
    assert result.steps[0]["output"].startswith("error:")


async def test_loop_stops_at_max_iterations():
    # Always asks for a tool → never finishes.
    scripted = [_tool_use(f"t{i}", "calculator", {"expression": "1+1"}) for i in range(10)]
    llm = FakeLLM(scripted)
    result = await run_agent_loop(llm, "sys", "x", tools=build_tools(["calculator"]), max_iterations=3)
    assert "maximum number of steps" in result.output
    assert len(result.steps) == 3


def _slow_tool(name: str, delay: float) -> Tool:
    async def run(args: dict) -> str:
        await asyncio.sleep(delay)
        return f"{name}-done"

    return Tool(name=name, description="", input_schema={"type": "object"}, run=run)


async def test_multiple_tools_in_a_turn_run_concurrently():
    # One turn requesting two slow tools; gather should overlap them, so wall
    # time is ~max(delays), not the sum, and results stay aligned in order.
    delay = 0.15
    two_calls = LLMResponse(
        text=None,
        tool_calls=[
            ToolCall(id="a", name="slow_a", input={}),
            ToolCall(id="b", name="slow_b", input={}),
        ],
        assistant_content=[
            {"type": "tool_use", "id": "a", "name": "slow_a", "input": {}},
            {"type": "tool_use", "id": "b", "name": "slow_b", "input": {}},
        ],
    )
    llm = FakeLLM([two_calls, _final("ok")])
    tools = [_slow_tool("slow_a", delay), _slow_tool("slow_b", delay)]

    start = time.perf_counter()
    result = await run_agent_loop(llm, "sys", "x", tools, max_iterations=5)
    elapsed = time.perf_counter() - start

    assert elapsed < delay * 1.8  # concurrent, not 2*delay
    assert [s["tool"] for s in result.steps] == ["slow_a", "slow_b"]
    assert [s["output"] for s in result.steps] == ["slow_a-done", "slow_b-done"]


# ── Retry / backoff ─────────────────────────────────────────────────────────


class _StatusError(Exception):
    def __init__(self, status_code: int) -> None:
        super().__init__(f"http {status_code}")
        self.status_code = status_code


def test_is_transient_classifies_by_status_and_name():
    assert _is_transient(_StatusError(429)) is True
    assert _is_transient(_StatusError(503)) is True
    assert _is_transient(_StatusError(400)) is False
    assert _is_transient(_StatusError(401)) is False
    assert _is_transient(type("APITimeoutError", (Exception,), {})()) is True
    assert _is_transient(type("APIConnectionError", (Exception,), {})()) is True
    assert _is_transient(ValueError("bad input")) is False


async def test_retry_succeeds_after_transient_errors():
    calls = {"n": 0}

    def fn():
        calls["n"] += 1
        if calls["n"] < 3:
            raise _StatusError(429)
        return "ok"

    out = await _call_with_retries(fn, max_attempts=3, base_delay=0.0)
    assert out == "ok"
    assert calls["n"] == 3


async def test_retry_gives_up_after_max_attempts():
    calls = {"n": 0}

    def fn():
        calls["n"] += 1
        raise _StatusError(503)

    with pytest.raises(_StatusError):
        await _call_with_retries(fn, max_attempts=3, base_delay=0.0)
    assert calls["n"] == 3


async def test_retry_does_not_retry_non_transient():
    calls = {"n": 0}

    def fn():
        calls["n"] += 1
        raise _StatusError(400)

    with pytest.raises(_StatusError):
        await _call_with_retries(fn, max_attempts=3, base_delay=0.0)
    assert calls["n"] == 1


# ── Prompt caching (Anthropic) ──────────────────────────────────────────────


class _FakeMsg:
    def __init__(self):
        self.content = [type("B", (), {"type": "text", "text": "hi"})()]
        self.stop_reason = "end_turn"
        self.usage = type("U", (), {"input_tokens": 1, "output_tokens": 1})()


class _FakeAnthropic:
    def __init__(self):
        self.captured = {}

        class _Messages:
            def create(_self, **kwargs):
                self.captured = kwargs
                return _FakeMsg()

        self.messages = _Messages()


async def test_anthropic_marks_static_prefix_for_caching():
    client = _FakeAnthropic()
    llm = AnthropicLLM(client, "claude-sonnet-4-6", 256)
    tools = [{"name": "calculator", "description": "", "input_schema": {}}]

    await llm.respond("you are helpful", [{"role": "user", "content": "hi"}], tools)

    system = client.captured["system"]
    assert system[0]["cache_control"] == {"type": "ephemeral"}
    assert system[0]["text"] == "you are helpful"
    # Tools precede system, so the system breakpoint already caches them; the
    # tool list itself is passed through unmarked.
    assert "cache_control" not in client.captured["tools"][0]


async def test_anthropic_caches_tools_when_no_system():
    client = _FakeAnthropic()
    llm = AnthropicLLM(client, "claude-sonnet-4-6", 256)
    tools = [{"name": "calculator", "description": "", "input_schema": {}}]

    await llm.respond("", [{"role": "user", "content": "hi"}], tools)

    assert "system" not in client.captured
    assert client.captured["tools"][-1]["cache_control"] == {"type": "ephemeral"}
