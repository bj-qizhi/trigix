# Copyright © 2026 北京祺智科技有限公司. All rights reserved.
# https://www.qzso.com/ · managecode@gmail.com

"""Built-in tools the agent can call.

Each tool exposes an Anthropic-compatible `input_schema` and an async `run`.
Tools are deliberately safe and side-effect-free (a sandboxed calculator and
knowledge-base search) so the agent loop is fully testable offline.
"""

from __future__ import annotations

import ast
import ipaddress
import json
import operator
import socket
from dataclasses import dataclass
from typing import Any, Awaitable, Callable
from urllib.parse import urlparse

import httpx

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


# ── HTTP request (SSRF-guarded outbound calls) ──────────────────────────────

_BLOCKED_HOST_LITERALS = {"localhost", "metadata.google.internal"}


def is_safe_url(url: str, allow_hosts: list[str] | None) -> tuple[bool, str]:
    """Decide whether the agent may call `url`.

    With an allowlist, only those exact hosts are reachable (the operator
    explicitly trusts them, e.g. an internal tool API). Without one, the URL
    must be http/https and must not resolve to a private, loopback, link-local,
    reserved, or multicast address — the usual SSRF targets (cloud metadata,
    localhost, RFC-1918).
    """
    parsed = urlparse(url)
    if parsed.scheme not in ("http", "https"):
        return False, "only http/https URLs are allowed"
    host = parsed.hostname
    if not host:
        return False, "URL has no host"
    if allow_hosts is not None:
        return (host in allow_hosts), (
            "" if host in allow_hosts else f"host '{host}' is not in the allowlist"
        )
    if host.lower() in _BLOCKED_HOST_LITERALS:
        return False, f"host '{host}' is blocked"
    port = parsed.port or (443 if parsed.scheme == "https" else 80)
    try:
        infos = socket.getaddrinfo(host, port, proto=socket.IPPROTO_TCP)
    except socket.gaierror as exc:
        return False, f"DNS resolution failed: {exc}"
    for info in infos:
        ip = info[4][0]
        try:
            addr = ipaddress.ip_address(ip.split("%")[0])
        except ValueError:
            return False, f"unparseable address {ip}"
        if (
            addr.is_private
            or addr.is_loopback
            or addr.is_link_local
            or addr.is_reserved
            or addr.is_multicast
            or addr.is_unspecified
        ):
            return False, f"host resolves to a non-public address ({ip})"
    return True, ""


def http_request_tool(allow_hosts: list[str] | None = None) -> Tool:
    async def run(args: dict) -> str:
        url = str(args.get("url", ""))
        method = str(args.get("method", "GET")).upper()
        ok, reason = is_safe_url(url, allow_hosts)
        if not ok:
            return f"error: {reason}"
        headers = args.get("headers") if isinstance(args.get("headers"), dict) else None
        body = args.get("body")
        kwargs: dict[str, Any] = {}
        if isinstance(body, (dict, list)):
            kwargs["json"] = body
        elif body is not None:
            kwargs["content"] = str(body)
        try:
            async with httpx.AsyncClient(timeout=15.0, follow_redirects=False) as client:
                resp = await client.request(method, url, headers=headers, **kwargs)
            return json.dumps({"status": resp.status_code, "body": resp.text[:8000]})
        except httpx.HTTPError as exc:
            return f"error: request failed: {exc}"

    return Tool(
        name="http_request",
        description="Make an HTTP request to a public URL and return its status and body.",
        input_schema={
            "type": "object",
            "properties": {
                "url": {"type": "string"},
                "method": {"type": "string", "enum": ["GET", "POST", "PUT", "PATCH", "DELETE"]},
                "headers": {"type": "object"},
                "body": {"description": "JSON object/array, or a raw string"},
            },
            "required": ["url"],
        },
        run=run,
    )


# ── Custom workflow node exposed as a tool ──────────────────────────────────


def custom_node_tool(spec: dict) -> Tool:
    """Wrap a registered custom node (served over the executor's HTTP contract)
    as an agent tool, so the agent can reuse the tenant's own nodes."""
    name = str(spec["name"])
    url = str(spec["url"])
    node_config = spec.get("config") if isinstance(spec.get("config"), dict) else {}

    async def run(args: dict) -> str:
        payload = {
            "node_id": name,
            "config": node_config,
            "input_json": json.dumps(args),
            "node_outputs": {},
        }
        try:
            async with httpx.AsyncClient(timeout=30.0, follow_redirects=False) as client:
                resp = await client.post(url, json=payload)
            resp.raise_for_status()
            return resp.json().get("output_json", resp.text)[:8000]
        except httpx.HTTPError as exc:
            return f"error: custom node call failed: {exc}"

    return Tool(
        name=name,
        description=str(spec.get("description", f"Call the '{name}' custom node.")),
        input_schema=spec.get("input_schema") or {"type": "object", "properties": {}},
        run=run,
    )


def build_tools(
    names: list[str],
    store: Any = None,
    tenant_id: str = "tenant-1",
    default_kb: str = "",
    node_tools: list[dict] | None = None,
    http_allow_hosts: list[str] | None = None,
) -> list[Tool]:
    """Resolve enabled tool names into Tool instances. Unknown names and
    rag_search without a store are skipped. `node_tools` are explicit custom
    node specs ({name, url, description?, input_schema?})."""
    tools: list[Tool] = []
    for name in names:
        if name == "calculator":
            tools.append(calculator_tool())
        elif name == "rag_search" and store is not None:
            tools.append(rag_search_tool(store, tenant_id, default_kb))
        elif name == "http_request":
            tools.append(http_request_tool(http_allow_hosts))
    for spec in node_tools or []:
        if spec.get("name") and spec.get("url"):
            tools.append(custom_node_tool(spec))
    return tools
