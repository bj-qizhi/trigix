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

from .agent.loop import AnthropicLLM, OpenAICompatLLM, run_agent_loop
from .agent.tools import build_tools
from .rag.router import router as rag_router

app = FastAPI(title="Trigix AI Runtime")
app.include_router(rag_router)

_anthropic_client: anthropic.Anthropic | None = None


def get_anthropic_client() -> anthropic.Anthropic:
    global _anthropic_client
    if _anthropic_client is None:
        # Retries are handled in the agent loop (_call_with_retries); disable the
        # SDK's own so the two don't compound.
        _anthropic_client = anthropic.Anthropic(max_retries=0)
    return _anthropic_client


class AgentNodeRequest(BaseModel):
    node_id: str
    node_config: dict[str, Any]
    input_json: str
    node_outputs: dict[str, str] = {}


class AgentNodeResponse(BaseModel):
    output_json: str


@app.get("/healthz")
def healthz() -> dict[str, str]:
    return {"status": "ok"}


def _select_provider(config: dict[str, Any]) -> str:
    """Pick the LLM backend. An explicit `provider` wins; otherwise a configured
    `base_url` or a non-Claude model implies the OpenAI-compatible path, so the
    agent runs on Qwen/DeepSeek/Zhipu/Moonshot/self-hosted vLLM in deployments
    where Anthropic is unreachable."""
    provider = str(config.get("provider", "")).lower().strip()
    if provider in ("openai", "anthropic"):
        return provider
    if config.get("base_url") or config.get("api_base"):
        return "openai"
    model = str(config.get("model", "")).lower()
    if model == "" or model.startswith("claude"):
        return "anthropic"
    return "openai"


def _build_llm(config: dict[str, Any], model: str, max_tokens: int):
    if _select_provider(config) == "anthropic":
        # A per-node api_key wins (resolved from {{credential.…}} by the platform
        # before dispatch); otherwise fall back to the runtime's env var. This
        # mirrors the OpenAI-compatible path and the other LLM nodes.
        cfg_key = config.get("api_key")
        if cfg_key:
            client = anthropic.Anthropic(api_key=str(cfg_key), max_retries=0)
        elif os.environ.get("ANTHROPIC_API_KEY"):
            client = get_anthropic_client()
        else:
            raise HTTPException(
                status_code=503,
                detail="Anthropic agent requires an API key "
                "(config.api_key or the ANTHROPIC_API_KEY env var)",
            )
        return AnthropicLLM(client, model, max_tokens)

    # OpenAI-compatible provider.
    base_url = (
        config.get("base_url")
        or config.get("api_base")
        or os.environ.get("OPENAI_BASE_URL")
    )
    api_key = (
        config.get("api_key")
        or os.environ.get("OPENAI_API_KEY")
        or os.environ.get("LLM_API_KEY")
    )
    if not api_key:
        raise HTTPException(
            status_code=503,
            detail="OpenAI-compatible agent requires an API key "
            "(config.api_key or the OPENAI_API_KEY / LLM_API_KEY env var)",
        )
    try:
        from openai import OpenAI
    except ImportError as exc:  # pragma: no cover - import guard
        raise HTTPException(
            status_code=503,
            detail="The 'openai' package is not installed; install the runtime "
            "with the [openai] extra to use an OpenAI-compatible provider",
        ) from exc
    # Retries are handled in the agent loop; disable the SDK's own to avoid
    # compounding (see get_anthropic_client).
    opts: dict[str, Any] = {"api_key": api_key, "max_retries": 0}
    if base_url:
        opts["base_url"] = base_url
    client = OpenAI(**opts)
    return OpenAICompatLLM(client, model, max_tokens)


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


def _resolve_template(template: str, input_json: str, node_outputs: dict[str, str]) -> str:
    """Replace {{expr}} patterns. expr = 'input', 'input.a.b', 'node_id', 'node_id.a.b'."""
    def resolve_expr(m: re.Match) -> str:
        expr = m.group(1).strip()
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

    return re.sub(r"\{\{([^}]+)\}\}", resolve_expr, template)


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
