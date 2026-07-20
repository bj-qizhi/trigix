"""Canonical model-construction boundary for Agent and RAG execution.

Provider selection, credential precedence and SDK retry ownership live here so
AI capabilities do not each grow subtly different provider behaviour.
"""

import os
from typing import Any

import anthropic
from fastapi import HTTPException

from .agent.loop import AnthropicLLM, OpenAICompatLLM

_anthropic_client: anthropic.Anthropic | None = None


def select_provider(config: dict[str, Any]) -> str:
    provider = str(config.get("provider", "")).lower().strip()
    if provider in ("openai", "anthropic"):
        return provider
    if config.get("base_url") or config.get("api_base"):
        return "openai"
    model = str(config.get("model", "")).lower()
    return "anthropic" if model == "" or model.startswith("claude") else "openai"


def _shared_anthropic_client() -> anthropic.Anthropic:
    global _anthropic_client
    if _anthropic_client is None:
        _anthropic_client = anthropic.Anthropic(max_retries=0)
    return _anthropic_client


def build_llm(config: dict[str, Any], model: str, max_tokens: int):
    """Build the configured backend with one retry and credential policy.

    SDK retries are disabled because the Agent loop owns transient retries. A
    per-Node Credential wins over runtime environment configuration.
    """
    if select_provider(config) == "anthropic":
        cfg_key = config.get("api_key")
        if cfg_key:
            client = anthropic.Anthropic(api_key=str(cfg_key), max_retries=0)
        elif os.environ.get("ANTHROPIC_API_KEY"):
            client = _shared_anthropic_client()
        else:
            raise HTTPException(
                status_code=503,
                detail="Anthropic model requires config.api_key or ANTHROPIC_API_KEY",
            )
        return AnthropicLLM(client, model, max_tokens)

    base_url = config.get("base_url") or config.get("api_base") or os.environ.get(
        "OPENAI_BASE_URL"
    )
    api_key = (
        config.get("api_key")
        or os.environ.get("OPENAI_API_KEY")
        or os.environ.get("LLM_API_KEY")
    )
    if not api_key:
        raise HTTPException(
            status_code=503,
            detail="OpenAI-compatible model requires config.api_key, OPENAI_API_KEY, or LLM_API_KEY",
        )
    try:
        from openai import OpenAI
    except ImportError as exc:  # pragma: no cover
        raise HTTPException(
            status_code=503,
            detail="Install the runtime's openai extra for OpenAI-compatible providers",
        ) from exc

    options: dict[str, Any] = {"api_key": api_key, "max_retries": 0}
    if base_url:
        options["base_url"] = base_url
    return OpenAICompatLLM(OpenAI(**options), model, max_tokens)
