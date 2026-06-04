# Copyright © 2026 北京祺智科技有限公司. All rights reserved.
# https://www.qzso.com/ · managecode@gmail.com

import pytest

from app.agent.tools import build_tools, calculator_tool, safe_calc


def test_safe_calc_arithmetic():
    assert safe_calc("2 + 3 * 4") == 14
    assert safe_calc("(1 + 2) ** 3") == 27
    assert safe_calc("-10 // 3") == -4


def test_safe_calc_rejects_non_arithmetic():
    with pytest.raises(ValueError):
        safe_calc("__import__('os').system('echo hi')")
    with pytest.raises(ValueError):
        safe_calc("a + 1")  # names not allowed


async def test_calculator_tool_run():
    tool = calculator_tool()
    assert await tool.run({"expression": "10 / 4"}) == "2.5"


def test_build_tools_skips_unknown_and_rag_without_store():
    tools = build_tools(["calculator", "rag_search", "bogus"])
    assert [t.name for t in tools] == ["calculator"]  # rag_search needs a store


def test_build_tools_includes_rag_with_store():
    tools = build_tools(["rag_search"], store=object(), tenant_id="t", default_kb="kb")
    assert [t.name for t in tools] == ["rag_search"]
