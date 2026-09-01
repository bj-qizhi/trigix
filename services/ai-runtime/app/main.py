# Copyright © 2026 北京祺智科技有限公司. All rights reserved.
# https://www.qzso.com/ · managecode@gmail.com

import asyncio
import json
import os
import re
from typing import Any

import anthropic
from fastapi import FastAPI, HTTPException
from fastapi.responses import StreamingResponse
from pydantic import BaseModel

from .agent.loop import run_agent_loop
from .agent.tools import build_tools
from .model_gateway import build_llm
from .rag.router import router as rag_router

app = FastAPI(title="Trigix AI Runtime")
app.include_router(rag_router)

class AgentNodeRequest(BaseModel):
    tenant_id: str = ""
    execution_id: str = ""
    node_id: str
    node_config: dict[str, Any]
    input_json: str
    node_outputs: dict[str, str] = {}


class AgentNodeResponse(BaseModel):
    output_json: str


@app.get("/healthz")
def healthz() -> dict[str, str]:
    return {"status": "ok"}


def _build_llm(config: dict[str, Any], model: str, max_tokens: int):
    # Compatibility seam for tests and callers that patch the old local helper.
    return build_llm(config, model, max_tokens)


async def _prepare_agent(request: AgentNodeRequest):
    """Resolve everything a run needs (llm, prompts, tools) from the request.
    Shared by the buffered and streaming agent endpoints."""
    config = request.node_config
    model = config.get("model", "claude-sonnet-4-6")
    system_prompt = config.get("system_prompt", "You are a helpful AI assistant.")
    # Resolve {{input.…}} / {{node_id.…}} in the system prompt too (the user
    # message is already templated in _build_user_message).
    system_prompt = _resolve_template(
        system_prompt, request.input_json, request.node_outputs
    )
    max_tokens = int(config.get("max_tokens", 1024))

    user_message = _build_user_message(config, request.input_json, request.node_outputs)

    # Resolve the agent's tool set. `calculator` is always available; `rag_search`
    # is added when configured and a knowledge-base store is reachable.
    tool_names = config.get("tools") or []
    if "browser" in tool_names:
        system_prompt += (
            "\n\nBrowser safety policy: operate only on the configured allowed hosts and actions. "
            "Never attempt CAPTCHA solving, anti-bot evasion, fingerprint spoofing, or access-control bypass. "
            "Close the Browser Session when the task is complete."
        )
    store = None
    if "rag_search" in tool_names:
        try:
            from .rag.router import get_store

            store = await get_store()
        except Exception:
            store = None  # no DB → rag_search silently unavailable
    node_tools = config.get("node_tools") if isinstance(config.get("node_tools"), list) else []
    http_allow_hosts = config.get("http_allow_hosts")
    if not isinstance(http_allow_hosts, list):
        env_allow = os.environ.get("AGENT_HTTP_ALLOW_HOSTS", "").strip()
        http_allow_hosts = [h.strip() for h in env_allow.split(",") if h.strip()] or None
    http_allow_public = bool(config.get("http_allow_public")) or os.environ.get(
        "AGENT_HTTP_ALLOW_PUBLIC", ""
    ).strip().lower() in ("1", "true", "yes")
    tools = build_tools(
        tool_names,
        store=store,
        tenant_id=str(config.get("tenant_id", "tenant-1")),
        default_kb=str(config.get("kb", "")),
        node_tools=node_tools,
        http_allow_hosts=http_allow_hosts,
        http_allow_public=http_allow_public,
        browser_runtime_base_url=os.environ.get("BROWSER_RUNTIME_BASE_URL", "").strip(),
        browser_runtime_auth_token=os.environ.get("BROWSER_RUNTIME_AUTH_TOKEN", "").strip(),
        browser_tenant_id=request.tenant_id,
        browser_execution_id=request.execution_id,
        browser_allowed_hosts=config.get("browser_allowed_hosts"),
        browser_allowed_actions=config.get("browser_allowed_actions"),
        browser_max_steps=int(config.get("browser_max_steps", 12)),
        browser_max_duration_seconds=int(config.get("browser_max_duration_seconds", 120)),
    )
    max_iterations = int(config.get("max_iterations", 6))
    llm = _build_llm(config, model, max_tokens)
    return llm, system_prompt, user_message, tools, max_iterations


def _assemble_output(result) -> str:
    """Fold an AgentResult into the node's output_json — the model's own fields
    plus the usage and tool-call trace, without clobbering either."""
    try:
        parsed = json.loads(result.output)
    except (json.JSONDecodeError, ValueError):
        parsed = {"text": result.output}
    # `_agent_steps` is [{tool, input, output}] per step so the run is
    # observable/debuggable downstream instead of being discarded.
    if isinstance(parsed, dict):
        parsed.setdefault("_agent_usage", result.usage)
        parsed.setdefault("_agent_steps", result.steps)
    return json.dumps(parsed)


@app.post("/v1/nodes/agent", response_model=AgentNodeResponse)
async def run_agent_node(request: AgentNodeRequest) -> AgentNodeResponse:
    llm, system_prompt, user_message, tools, max_iterations = await _prepare_agent(request)
    try:
        result = await run_agent_loop(
            llm, system_prompt, user_message, tools, max_iterations
        )
    except anthropic.APIError as exc:
        raise HTTPException(status_code=502, detail=f"Anthropic API error: {exc}") from exc

    return AgentNodeResponse(output_json=_assemble_output(result))


@app.post("/v1/nodes/agent/stream")
async def run_agent_node_stream(request: AgentNodeRequest) -> StreamingResponse:
    """Same agent run, streamed: emits `data: {"delta": "..."}` SSE frames as the
    model generates text, then a final `data: {"done": true, "output_json": ...}`
    (or `{"error": ...}`). The buffered endpoint above is unchanged; callers that
    don't want live tokens keep using it."""
    llm, system_prompt, user_message, tools, max_iterations = await _prepare_agent(request)
    loop = asyncio.get_running_loop()
    queue: asyncio.Queue = asyncio.Queue()

    def on_delta(text: str) -> None:
        # Called from the SDK worker thread → hop back onto the event loop.
        loop.call_soon_threadsafe(queue.put_nowait, {"delta": text})

    async def drive() -> None:
        try:
            result = await run_agent_loop(
                llm, system_prompt, user_message, tools, max_iterations,
                on_text_delta=on_delta,
            )
            payload = {"done": True, "output_json": _assemble_output(result)}
        except Exception as exc:  # surface the failure to the client, then stop
            payload = {"error": str(exc)}
        loop.call_soon_threadsafe(queue.put_nowait, payload)

    task = asyncio.create_task(drive())

    async def gen():
        try:
            while True:
                item = await queue.get()
                yield f"data: {json.dumps(item)}\n\n"
                if "done" in item or "error" in item:
                    break
        finally:
            task.cancel()

    return StreamingResponse(gen(), media_type="text/event-stream")


# ── RAG end-to-end generation ────────────────────────────────────────────────
# Retrieve from a knowledge base, stuff the chunks into a grounded prompt, and
# have an LLM answer — so a `rag` node in "generate" mode returns a finished
# answer (+ its sources) instead of only raw chunks.


class RagGenerateRequest(BaseModel):
    tenant_id: str
    kb: str
    query: str
    top_k: int = 4
    mode: str = "vector"
    min_score: float | None = None
    rerank: bool = False
    model: str = "claude-sonnet-4-6"
    system_prompt: str = (
        "You are a helpful assistant. Answer the question using ONLY the provided "
        "context. If the context does not contain the answer, say you don't know "
        "rather than guessing."
    )
    max_tokens: int = 1024
    api_key: str | None = None
    base_url: str | None = None
    provider: str | None = None


class RagSource(BaseModel):
    doc_id: str
    chunk_index: int
    content: str
    score: float


class RagGenerateResponse(BaseModel):
    answer: str
    sources: list[RagSource]
    usage: dict


async def _rag_retrieve(req: RagGenerateRequest):
    from .rag.router import get_store

    store = await get_store()
    top_k = max(1, min(req.top_k, 50))
    mode = req.mode if req.mode in ("vector", "hybrid") else "vector"
    return await store.query(
        req.tenant_id, req.kb, req.query, top_k,
        mode=mode, min_score=req.min_score, rerank=req.rerank,
    )


def _rag_llm(req: RagGenerateRequest):
    config = {
        "model": req.model,
        "api_key": req.api_key,
        "base_url": req.base_url,
        "provider": req.provider,
    }
    return _build_llm(config, req.model, req.max_tokens)


def _rag_prompt(query: str, hits) -> str:
    context = (
        "\n\n".join(f"[{i + 1}] {h.content}" for i, h in enumerate(hits))
        if hits
        else "(no relevant context found)"
    )
    return f"Context:\n{context}\n\nQuestion: {query}"


def _rag_sources(hits) -> list[RagSource]:
    return [
        RagSource(doc_id=h.doc_id, chunk_index=h.chunk_index, content=h.content, score=h.score)
        for h in hits
    ]


@app.post("/v1/rag/generate", response_model=RagGenerateResponse)
async def rag_generate(req: RagGenerateRequest) -> RagGenerateResponse:
    hits = await _rag_retrieve(req)
    llm = _rag_llm(req)
    try:
        resp = await llm.respond(
            req.system_prompt,
            [{"role": "user", "content": _rag_prompt(req.query, hits)}],
            [],
        )
    except anthropic.APIError as exc:
        raise HTTPException(status_code=502, detail=f"LLM error: {exc}") from exc
    return RagGenerateResponse(answer=resp.text or "", sources=_rag_sources(hits), usage=resp.usage)


@app.post("/v1/rag/generate/stream")
async def rag_generate_stream(req: RagGenerateRequest) -> StreamingResponse:
    """Streaming RAG generate: `data:{"delta":…}` frames, then a final
    `{"done": true, "output_json": …}` / `{"error": …}` — the same frame shape as
    the agent stream, so the executor consumes both identically."""
    hits = await _rag_retrieve(req)
    llm = _rag_llm(req)
    loop = asyncio.get_running_loop()
    queue: asyncio.Queue = asyncio.Queue()

    def on_delta(text: str) -> None:
        loop.call_soon_threadsafe(queue.put_nowait, {"delta": text})

    async def drive() -> None:
        try:
            resp = await llm.respond(
                req.system_prompt,
                [{"role": "user", "content": _rag_prompt(req.query, hits)}],
                [],
                on_text_delta=on_delta,
            )
            output = json.dumps({
                "answer": resp.text or "",
                "sources": [s.model_dump() for s in _rag_sources(hits)],
                "usage": resp.usage,
            })
            payload = {"done": True, "output_json": output}
        except Exception as exc:
            payload = {"error": str(exc)}
        loop.call_soon_threadsafe(queue.put_nowait, payload)

    task = asyncio.create_task(drive())

    async def gen():
        try:
            while True:
                item = await queue.get()
                yield f"data: {json.dumps(item)}\n\n"
                if "done" in item or "error" in item:
                    break
        finally:
            task.cancel()

    return StreamingResponse(gen(), media_type="text/event-stream")


def _resolve_template(template: str, input_json: str, node_outputs: dict[str, str]) -> str:
    """Replace {{expr}} patterns. expr = 'input', 'input.a.b', 'node_id', 'node_id.a.b'."""

    def resolve_expr(raw_expr: str) -> str:
        expr = raw_expr.strip()
        parts = expr.split(".", 1)
        root, path = parts[0], parts[1] if len(parts) > 1 else None
        raw = input_json if root == "input" else node_outputs.get(root, "")
        if not raw:
            return ""
        if path is None:
            return raw
        try:
            data: Any = json.loads(raw)
            for key in path.split("."):
                if isinstance(data, dict):
                    data = data.get(key, "")
                elif isinstance(data, list) and key.isdigit():
                    data = data[int(key)]
                else:
                    return ""
            return "" if data is None else str(data)
        except (json.JSONDecodeError, IndexError):
            return ""

    # Parse placeholders in one forward pass. A user-controlled template can
    # contain arbitrarily many opening delimiters, so a backtracking regular
    # expression is inappropriate here. Malformed/empty placeholders remain
    # literal and scanning always resumes after the consumed delimiter.
    rendered: list[str] = []
    cursor = 0
    while cursor < len(template):
        start = template.find("{{", cursor)
        if start < 0:
            rendered.append(template[cursor:])
            break
        rendered.append(template[cursor:start])
        first_close = template.find("}", start + 2)
        if first_close < 0:
            rendered.append(template[start:])
            break
        has_pair = first_close + 1 < len(template) and template[first_close + 1] == "}"
        if has_pair and first_close > start + 2:
            rendered.append(resolve_expr(template[start + 2:first_close]))
            cursor = first_close + 2
        else:
            rendered.append(template[start:first_close + 1])
            cursor = first_close + 1
    return "".join(rendered)


def _build_user_message(
    config: dict[str, Any],
    input_json: str,
    node_outputs: dict[str, str],
) -> str:
    template = config.get("prompt_template")
    if template:
        return _resolve_template(template, input_json, node_outputs)

    parts = [f"Input: {input_json}"]
    if node_outputs:
        parts.append("Prior node outputs:")
        for node_id, output in node_outputs.items():
            parts.append(f"  {node_id}: {output}")
    return "\n".join(parts)
