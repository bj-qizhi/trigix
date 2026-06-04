# Copyright © 2026 北京祺智科技有限公司. All rights reserved.
# https://www.qzso.com/ · managecode@gmail.com

"""Built-in tools the agent can call.

Each tool exposes an Anthropic-compatible `input_schema` and an async `run`.
Tools are deliberately safe and side-effect-free (a sandboxed calculator and
knowledge-base search) so the agent loop is fully testable offline.
"""

from __future__ import annotations

import ast
import json
import operator
from dataclasses import dataclass
from typing import Any, Awaitable, Callable

ToolRun = Callable[[dict], Awaitable[str]]


@dataclass
class Tool:
    name: str
    description: str
    input_schema: dict
    run: ToolRun


# ── Calculator (sandboxed arithmetic, no eval) ──────────────────────────────

_BIN_OPS = {
    ast.Add: operator.add,
    ast.Sub: operator.sub,
    ast.Mult: operator.mul,
    ast.Div: operator.truediv,
    ast.FloorDiv: operator.floordiv,
    ast.Mod: operator.mod,
    ast.Pow: operator.pow,
}
_UNARY_OPS = {ast.UAdd: operator.pos, ast.USub: operator.neg}


def _eval(node: ast.AST) -> float:
    if isinstance(node, ast.Expression):
        return _eval(node.body)
    if isinstance(node, ast.Constant) and isinstance(node.value, (int, float)):
        return node.value
    if isinstance(node, ast.BinOp) and type(node.op) in _BIN_OPS:
        return _BIN_OPS[type(node.op)](_eval(node.left), _eval(node.right))
    if isinstance(node, ast.UnaryOp) and type(node.op) in _UNARY_OPS:
        return _UNARY_OPS[type(node.op)](_eval(node.operand))
    raise ValueError("only numeric arithmetic (+, -, *, /, //, %, **) is allowed")


def safe_calc(expression: str) -> float:
    return _eval(ast.parse(expression, mode="eval"))


async def _calculator_run(args: dict) -> str:
    return str(safe_calc(str(args.get("expression", ""))))


def calculator_tool() -> Tool:
    return Tool(
        name="calculator",
        description="Evaluate a basic arithmetic expression (+, -, *, /, //, %, **).",
        input_schema={
            "type": "object",
            "properties": {"expression": {"type": "string", "description": "e.g. (12 + 5) * 3"}},
            "required": ["expression"],
        },
        run=_calculator_run,
    )


# ── Knowledge-base search (RAG retrieval as a tool) ─────────────────────────


def rag_search_tool(store: Any, tenant_id: str, default_kb: str) -> Tool:
    async def run(args: dict) -> str:
        kb = str(args.get("kb") or default_kb)
        query = str(args.get("query", ""))
        top_k = int(args.get("top_k", 4))
        if not kb:
            return "error: no knowledge base specified"
        hits = await store.query(tenant_id, kb, query, top_k)
        return json.dumps(
            [{"content": h.content, "score": round(h.score, 4), "doc_id": h.doc_id} for h in hits]
        )

    return Tool(
        name="rag_search",
        description="Search a knowledge base for relevant document chunks to ground the answer.",
        input_schema={
            "type": "object",
            "properties": {
                "query": {"type": "string"},
                "kb": {"type": "string", "description": "knowledge base name (optional)"},
                "top_k": {"type": "integer"},
            },
            "required": ["query"],
        },
        run=run,
    )


def build_tools(
    names: list[str], store: Any = None, tenant_id: str = "tenant-1", default_kb: str = ""
) -> list[Tool]:
    """Resolve enabled tool names into Tool instances. Unknown names and
    rag_search without a store are skipped."""
    tools: list[Tool] = []
    for name in names:
        if name == "calculator":
            tools.append(calculator_tool())
        elif name == "rag_search" and store is not None:
            tools.append(rag_search_tool(store, tenant_id, default_kb))
    return tools
