# Copyright © 2026 北京祺智科技有限公司. All rights reserved.
# https://www.qzso.com/ · managecode@gmail.com

"""Drive the agent loop through the OpenAI-compatible backend with a fake
chat-completions client — no `openai` package or API key required."""

import json

from app.agent.loop import (
    OpenAICompatLLM,
    _to_openai_messages,
    _to_openai_tools,
    run_agent_loop,
)
from app.agent.tools import build_tools


# ── A minimal stand-in for the OpenAI SDK client ────────────────────────────


class _Function:
    def __init__(self, name, arguments):
        self.name = name
        self.arguments = arguments


class _ToolCall:
    def __init__(self, call_id, name, arguments):
        self.id = call_id
        self.function = _Function(name, arguments)


class _Message:
    def __init__(self, content=None, tool_calls=None):
        self.content = content
        self.tool_calls = tool_calls


class _Choice:
    def __init__(self, message):
        self.message = message


class _Completion:
    def __init__(self, message):
        self.choices = [_Choice(message)]


class _Completions:
    def __init__(self, outer):
        self._outer = outer

    def create(self, **kwargs):
        self._outer.captured.append(kwargs)
        return _Completion(self._outer._scripted.pop(0))


class _Chat:
    def __init__(self, outer):
        self.completions = _Completions(outer)


class FakeOpenAIClient:
    def __init__(self, scripted):
        self._scripted = scripted
        self.captured = []
        self.chat = _Chat(self)


async def test_openai_backend_runs_tool_then_finishes():
    # 1st turn: model asks for the calculator. 2nd turn: final text answer.
    scripted = [
        _Message(content=None, tool_calls=[_ToolCall("call_1", "calculator", '{"expression": "6 * 7"}')]),
        _Message(content="The answer is 42"),
    ]
    client = FakeOpenAIClient(scripted)
    llm = OpenAICompatLLM(client, "qwen-plus", 256)

    result = await run_agent_loop(llm, "sys", "what is 6*7?", build_tools(["calculator"]), max_iterations=5)

    assert result.output == "The answer is 42"
    assert len(result.steps) == 1
    assert result.steps[0]["tool"] == "calculator"
    assert result.steps[0]["output"] == "42"

    # Tools were advertised in OpenAI's function-calling shape.
    assert client.captured[0]["tools"][0]["type"] == "function"
    assert client.captured[0]["tools"][0]["function"]["name"] == "calculator"

    # The 2nd request must carry the tool result back as a 'tool' role message.
    second_request_msgs = client.captured[1]["messages"]
    tool_msgs = [m for m in second_request_msgs if m["role"] == "tool"]
    assert tool_msgs and tool_msgs[0]["tool_call_id"] == "call_1"
    assert tool_msgs[0]["content"] == "42"
    # ...and the assistant turn that requested the tool, in OpenAI shape.
    assistant_msgs = [m for m in second_request_msgs if m["role"] == "assistant"]
    assert assistant_msgs[0]["tool_calls"][0]["function"]["name"] == "calculator"


async def test_malformed_tool_arguments_do_not_crash():
    scripted = [
        _Message(content=None, tool_calls=[_ToolCall("c1", "calculator", "{not json")]),
        _Message(content="recovered"),
    ]
    llm = OpenAICompatLLM(FakeOpenAIClient(scripted), "deepseek-chat", 128)
    result = await run_agent_loop(llm, "sys", "x", build_tools(["calculator"]), max_iterations=3)
    assert result.output == "recovered"
    # Empty args → calculator gets "" → surfaced as an error, fed back, not raised.
    assert result.steps[0]["output"].startswith("error:")


def test_message_translation_round_trips_anthropic_history():
    history = [
        {"role": "user", "content": "hi"},
        {
            "role": "assistant",
            "content": [
                {"type": "text", "text": "let me compute"},
                {"type": "tool_use", "id": "c1", "name": "calculator", "input": {"expression": "1+1"}},
            ],
        },
        {"role": "user", "content": [{"type": "tool_result", "tool_use_id": "c1", "content": "2"}]},
    ]
    out = _to_openai_messages("be helpful", history)
    assert out[0] == {"role": "system", "content": "be helpful"}
    assert out[1] == {"role": "user", "content": "hi"}
    assistant = out[2]
    assert assistant["role"] == "assistant"
    assert assistant["content"] == "let me compute"
    assert json.loads(assistant["tool_calls"][0]["function"]["arguments"]) == {"expression": "1+1"}
    assert out[3] == {"role": "tool", "tool_call_id": "c1", "content": "2"}


def test_tool_schema_translation():
    schemas = [{"name": "calculator", "description": "math", "input_schema": {"type": "object"}}]
    out = _to_openai_tools(schemas)
    assert out[0]["type"] == "function"
    assert out[0]["function"]["name"] == "calculator"
    assert out[0]["function"]["parameters"] == {"type": "object"}
