# Copyright © 2026 北京祺智科技有限公司. All rights reserved.
# https://www.qzso.com/ · managecode@gmail.com

import json
import os
import re
from typing import Any

import anthropic
from fastapi import FastAPI, HTTPException
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
        _anthropic_client = anthropic.Anthropic()
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
        if not os.environ.get("ANTHROPIC_API_KEY"):
            raise HTTPException(status_code=503, detail="ANTHROPIC_API_KEY is not configured")
        return AnthropicLLM(get_anthropic_client(), model, max_tokens)

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
    client = OpenAI(api_key=api_key, base_url=base_url) if base_url else OpenAI(api_key=api_key)
    return OpenAICompatLLM(client, model, max_tokens)


@app.post("/v1/nodes/agent", response_model=AgentNodeResponse)
async def run_agent_node(request: AgentNodeRequest) -> AgentNodeResponse:
    config = request.node_config
    model = config.get("model", "claude-sonnet-4-6")
    system_prompt = config.get("system_prompt", "You are a helpful AI assistant.")
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
    tools = build_tools(
        tool_names,
        store=store,
        tenant_id=str(config.get("tenant_id", "tenant-1")),
        default_kb=str(config.get("kb", "")),
    )
    max_iterations = int(config.get("max_iterations", 6))

    llm = _build_llm(config, model, max_tokens)
    try:
        result = await run_agent_loop(
            llm, system_prompt, user_message, tools, max_iterations
        )
    except anthropic.APIError as exc:
        raise HTTPException(status_code=502, detail=f"Anthropic API error: {exc}") from exc

    try:
        output_json = json.dumps(json.loads(result.output))
    except (json.JSONDecodeError, ValueError):
        output_json = json.dumps({"text": result.output})

    return AgentNodeResponse(output_json=output_json)


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
