# Copyright © 2026 北京祺智科技有限公司. All rights reserved.
# https://www.qzso.com/ · managecode@gmail.com

"""Built-in tools the agent can call.

Each tool exposes an Anthropic-compatible `input_schema` and an async `run`.
The read-only tools (a sandboxed calculator, knowledge-base search) keep the
loop testable offline; the acting tools (http_request, custom nodes) reach the
network, so http_request runs under a locked-down egress policy (default-deny,
SSRF validation, DNS-rebinding-safe IP pinning, response size cap).
"""

from __future__ import annotations

import ast
import ipaddress
import json
import operator
import socket
import asyncio
import time
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
    cleanup: Callable[[], Awaitable[None]] | None = None


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
        mode = args.get("mode") if args.get("mode") in ("vector", "hybrid") else "vector"
        rerank = bool(args.get("rerank", False))
        if not kb:
            return "error: no knowledge base specified"
        hits = await store.query(tenant_id, kb, query, top_k, mode=mode, rerank=rerank)
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
                "mode": {"type": "string", "enum": ["vector", "hybrid"]},
                "rerank": {"type": "boolean"},
            },
            "required": ["query"],
        },
        run=run,
    )


# ── HTTP request (sandboxed outbound egress) ────────────────────────────────
#
# The agent's egress is locked down rather than merely SSRF-checked:
#   * default-deny — refused unless the host is allowlisted or open public
#     egress is explicitly enabled;
#   * the validated IP is pinned at connect time, so a hostname that passes the
#     check cannot be re-resolved to an internal address (DNS rebinding);
#   * responses are size-capped and redirects are not followed.

_BLOCKED_HOST_LITERALS = {"localhost", "metadata.google.internal"}
_MAX_RESPONSE_BYTES = 2 * 1024 * 1024


def _egress_target(
    url: str, allow_hosts: list[str] | None, allow_public: bool
) -> tuple[bool, str | None, str]:
    """Authorise an outbound request.

    Returns ``(allowed, pinned_ip, reason)``. ``pinned_ip`` is the validated
    address the request must connect to (open-egress hosts); it is ``None`` for
    allowlisted hosts, which the operator already trusts and which are reached
    by normal resolution.
    """
    parsed = urlparse(url)
    if parsed.scheme not in ("http", "https"):
        return False, None, "only http/https URLs are allowed"
    host = parsed.hostname
    if not host:
        return False, None, "URL has no host"
    if allow_hosts is not None:
        if host in allow_hosts:
            return True, None, ""
        return False, None, f"host '{host}' is not in the allowlist"
    if not allow_public:
        return False, None, (
            "outbound HTTP is disabled; set an allowlist or AGENT_HTTP_ALLOW_PUBLIC"
        )
    if host.lower() in _BLOCKED_HOST_LITERALS:
        return False, None, f"host '{host}' is blocked"
    port = parsed.port or (443 if parsed.scheme == "https" else 80)
    try:
        infos = socket.getaddrinfo(host, port, proto=socket.IPPROTO_TCP)
    except socket.gaierror as exc:
        return False, None, f"DNS resolution failed: {exc}"
    pinned: str | None = None
    for info in infos:
        ip = info[4][0].split("%")[0]
        try:
            addr = ipaddress.ip_address(ip)
        except ValueError:
            return False, None, f"unparseable address {ip}"
        if (
            addr.is_private
            or addr.is_loopback
            or addr.is_link_local
            or addr.is_reserved
            or addr.is_multicast
            or addr.is_unspecified
        ):
            return False, None, f"host resolves to a non-public address ({ip})"
        if pinned is None:
            pinned = ip
    if pinned is None:
        return False, None, "no address resolved"
    return True, pinned, ""


def is_safe_url(
    url: str, allow_hosts: list[str] | None, allow_public: bool = False
) -> tuple[bool, str]:
    """Whether the agent may call `url` (validation only; see _egress_target)."""
    allowed, _ip, reason = _egress_target(url, allow_hosts, allow_public)
    return allowed, reason


def http_request_tool(
    allow_hosts: list[str] | None = None, allow_public: bool = False
) -> Tool:
    async def run(args: dict) -> str:
        url = str(args.get("url", ""))
        method = str(args.get("method", "GET")).upper()
        allowed, pinned_ip, reason = _egress_target(url, allow_hosts, allow_public)
        if not allowed:
            return f"error: {reason}"

        headers = dict(args["headers"]) if isinstance(args.get("headers"), dict) else {}
        body = args.get("body")
        req_kwargs: dict[str, Any] = {}
        if isinstance(body, (dict, list)):
            req_kwargs["json"] = body
        elif body is not None:
            req_kwargs["content"] = str(body)

        target = httpx.URL(url)
        if pinned_ip is not None:
            # Connect to the exact validated IP, keep the real Host, and verify
            # TLS against the hostname — closes the DNS-rebinding window.
            headers["Host"] = target.host
            req_kwargs["extensions"] = {"sni_hostname": target.host}
            target = target.copy_with(host=pinned_ip)
        if headers:
            req_kwargs["headers"] = headers

        try:
            async with httpx.AsyncClient(timeout=15.0, follow_redirects=False) as client:
                request = client.build_request(method, target, **req_kwargs)
                resp = await client.send(request, stream=True)
                buf = bytearray()
                async for chunk in resp.aiter_bytes():
                    buf.extend(chunk)
                    if len(buf) >= _MAX_RESPONSE_BYTES:
                        break
                await resp.aclose()
            text = bytes(buf).decode("utf-8", errors="replace")
            return json.dumps({"status": resp.status_code, "body": text[:8000]})
        except httpx.HTTPError as exc:
            return f"error: request failed: {exc}"

    return Tool(
        name="http_request",
        description="Make an HTTP request to an allowed public URL and return its status and body.",
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


# ── Browser Runtime tools ───────────────────────────────────────────────────

_BROWSER_ACTIONS = ("navigate", "click", "input", "wait", "extract", "screenshot")


def browser_runtime_tools(
    base_url: str,
    auth_token: str,
    tenant_id: str,
    execution_id: str,
    allowed_hosts: list[str],
    allowed_actions: list[str],
    max_steps: int,
    max_duration_seconds: int,
) -> list[Tool]:
    """Build a bounded, tenant-bound Browser Agent tool set.

    The Browser Runtime remains the network-policy authority. The Agent adds a
    narrower per-run host/action/step/duration policy so a model cannot expand
    the operator-approved scope during tool use.
    """
    if not base_url or len(auth_token) < 32 or not tenant_id:
        return []
    hosts = {str(host).strip().lower() for host in allowed_hosts if str(host).strip()}
    actions = {action for action in allowed_actions if action in _BROWSER_ACTIONS}
    step_limit = min(max(1, max_steps), 100)
    duration_limit = min(max(1, max_duration_seconds), 3600)
    deadline = time.monotonic() + duration_limit
    state: dict[str, Any] = {"session_id": None, "steps": 0}
    lock = asyncio.Lock()
    headers = {
        "authorization": f"Bearer {auth_token}",
        "x-trigix-tenant-id": tenant_id,
    }

    async def runtime_request(method: str, path: str, payload: dict | None = None) -> dict:
        remaining = deadline - time.monotonic()
        if remaining <= 0:
            raise RuntimeError("Browser Agent duration limit reached")
        timeout = min(15.0, max(0.25, remaining))
        async with httpx.AsyncClient(timeout=timeout, follow_redirects=False) as client:
            response = await client.request(
                method,
                f"{base_url.rstrip('/')}{path}",
                headers=headers,
                json=payload,
            )
        data = response.json()
        if response.status_code >= 400:
            error = data.get("error", {}) if isinstance(data, dict) else {}
            raise RuntimeError(
                f"{error.get('code', 'BROWSER_RUNTIME_ERROR')}: "
                f"{error.get('message', response.text[:500])}"
            )
        return data

    async def start(_args: dict) -> str:
        async with lock:
            if state["session_id"]:
                return json.dumps({"session_id": state["session_id"], "reused": True})
            session = await runtime_request(
                "POST",
                "/v1/sessions",
                {"tenant_id": tenant_id, "execution_id": execution_id},
            )
            state["session_id"] = session["id"]
            return json.dumps({"session_id": session["id"]})

    async def close(_args: dict) -> str:
        async with lock:
            session_id = state["session_id"]
            if not session_id:
                return json.dumps({"closed": True, "session_id": None})
            await runtime_request("DELETE", f"/v1/sessions/{session_id}")
            state["session_id"] = None
            return json.dumps({"closed": True, "session_id": session_id})

    async def action_run(action: str, args: dict) -> str:
        async with lock:
            if action not in actions:
                return f"error: browser action '{action}' is not allowed"
            if state["steps"] >= step_limit:
                return "error: Browser Agent step limit reached"
            if time.monotonic() >= deadline:
                return "error: Browser Agent duration limit reached"
            session_id = state["session_id"]
            if not session_id:
                return "error: call browser_start before browser actions"
            if action == "navigate":
                host = (urlparse(str(args.get("url", ""))).hostname or "").lower()
                if not _host_is_allowed(host, hosts):
                    return f"error: browser host '{host}' is not allowed"
            state["steps"] += 1
            created = await runtime_request(
                "POST",
                "/v1/tasks",
                {
                    "tenant_id": tenant_id,
                    "execution_id": execution_id,
                    "session_id": session_id,
                    "timeout_ms": min(60_000, max(1_000, int((deadline - time.monotonic()) * 1000))),
                    "actions": [{"type": action, "params": args}],
                },
            )
            task_id = created["task_id"]
            while True:
                task = await runtime_request("GET", f"/v1/tasks/{task_id}")
                if task.get("status") in ("completed", "failed", "timeout", "cancelled"):
                    break
                await asyncio.sleep(0.1)
            if task.get("status") != "completed":
                error = task.get("error") or {}
                return f"error: {error.get('code', 'BROWSER_ACTION_FAILED')}: {error.get('message', task.get('status'))}"
            result = task.get("result") or {}
            action_results = result.get("actions") or []
            output = action_results[-1].get("data") if action_results else None
            artifact_id = output.get("id") if isinstance(output, dict) else None
            return json.dumps(
                {
                    "task_id": task_id,
                    "session_id": session_id,
                    "result": output,
                    "artifact_url": f"/v1/browser/artifacts/{artifact_id}" if artifact_id else None,
                    "url": result.get("final_url"),
                    "title": result.get("title"),
                    "steps_used": state["steps"],
                    "steps_remaining": step_limit - state["steps"],
                }
            )[:8000]

    tools = [
        Tool(
            name="browser_start",
            description="Start an isolated Browser Session before using browser action tools.",
            input_schema={"type": "object", "properties": {}},
            run=start,
            cleanup=lambda: _cleanup_browser(close),
        )
    ]
    schemas = {
        "navigate": ({"url": {"type": "string"}, "wait_until": {"type": "string", "enum": ["load", "domcontentloaded", "networkidle", "commit"]}}, ["url"]),
        "click": ({"selector": {"type": "string"}}, ["selector"]),
        "input": ({"selector": {"type": "string"}, "value": {"type": "string"}, "clear_first": {"type": "boolean"}}, ["selector", "value"]),
        "wait": ({"selector": {"type": "string"}, "milliseconds": {"type": "number"}, "url": {"type": "string"}, "load_state": {"type": "string"}}, []),
        "extract": ({"selector": {"type": "string"}, "mode": {"type": "string", "enum": ["text", "html", "attribute", "json", "list", "table"]}, "attribute": {"type": "string"}}, ["selector"]),
        "screenshot": ({"full_page": {"type": "boolean"}}, []),
    }
    for action in _BROWSER_ACTIONS:
        if action not in actions:
            continue
        properties, required = schemas[action]

        async def run(args: dict, selected: str = action) -> str:
            return await action_run(selected, args)

        tools.append(
            Tool(
                name=f"browser_{action}",
                description=f"Run the bounded Browser {action} action in the active session.",
                input_schema={"type": "object", "properties": properties, "required": required},
                run=run,
            )
        )
    tools.append(
        Tool(
            name="browser_close",
            description="Close the active Browser Session and release its resources.",
            input_schema={"type": "object", "properties": {}},
            run=close,
        )
    )
    return tools


def _host_is_allowed(host: str, allowed_hosts: set[str]) -> bool:
    if not host:
        return False
    return host in allowed_hosts or any(
        pattern.startswith("*.") and host.endswith(pattern[1:]) and host != pattern[2:]
        for pattern in allowed_hosts
    )


async def _cleanup_browser(close: ToolRun) -> None:
    try:
        await close({})
    except Exception:
        pass


def build_tools(
    names: list[str],
    store: Any = None,
    tenant_id: str = "tenant-1",
    default_kb: str = "",
    node_tools: list[dict] | None = None,
    http_allow_hosts: list[str] | None = None,
    http_allow_public: bool = False,
    browser_runtime_base_url: str = "",
    browser_runtime_auth_token: str = "",
    browser_tenant_id: str = "",
    browser_execution_id: str = "",
    browser_allowed_hosts: list[str] | None = None,
    browser_allowed_actions: list[str] | None = None,
    browser_max_steps: int = 12,
    browser_max_duration_seconds: int = 120,
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
            tools.append(http_request_tool(http_allow_hosts, http_allow_public))
        elif name == "browser":
            tools.extend(
                browser_runtime_tools(
                    browser_runtime_base_url,
                    browser_runtime_auth_token,
                    browser_tenant_id,
                    browser_execution_id,
                    browser_allowed_hosts if isinstance(browser_allowed_hosts, list) else [],
                    browser_allowed_actions if isinstance(browser_allowed_actions, list) else list(_BROWSER_ACTIONS),
                    browser_max_steps,
                    browser_max_duration_seconds,
                )
            )
    for spec in node_tools or []:
        if spec.get("name") and spec.get("url"):
            tools.append(custom_node_tool(spec))
    return tools
