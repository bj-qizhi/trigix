# Copyright © 2026 北京祺智科技有限公司. All rights reserved.
# https://www.qzso.com/ · managecode@gmail.com

"""Drive the agent loop with a scripted fake LLM — no Anthropic key required."""

from app.agent.loop import LLMResponse, ToolCall, run_agent_loop
from app.agent.tools import build_tools


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
