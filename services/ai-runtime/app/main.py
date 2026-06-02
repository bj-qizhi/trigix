# Copyright © 2026 北京祺智科技有限公司. All rights reserved.
# https://www.qzso.com/ · managecode@gmail.com

import json
import os
import re
from typing import Any

import anthropic
from fastapi import FastAPI, HTTPException
from pydantic import BaseModel

app = FastAPI(title="AgentFlow AI Runtime")

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


@app.post("/v1/nodes/agent", response_model=AgentNodeResponse)
async def run_agent_node(request: AgentNodeRequest) -> AgentNodeResponse:
    config = request.node_config
    model = config.get("model", "claude-sonnet-4-6")
    system_prompt = config.get("system_prompt", "You are a helpful AI assistant.")
    max_tokens = int(config.get("max_tokens", 1024))

    user_message = _build_user_message(config, request.input_json, request.node_outputs)

    api_key = os.environ.get("ANTHROPIC_API_KEY")
    if not api_key:
        raise HTTPException(
            status_code=503,
            detail="ANTHROPIC_API_KEY is not configured",
        )

    try:
        client = get_anthropic_client()
        message = client.messages.create(
            model=model,
            max_tokens=max_tokens,
            system=system_prompt,
            messages=[{"role": "user", "content": user_message}],
        )
    except anthropic.APIError as exc:
        raise HTTPException(status_code=502, detail=f"Anthropic API error: {exc}") from exc

    text = message.content[0].text if message.content else ""

    import json

    try:
        parsed = json.loads(text)
        output_json = json.dumps(parsed)
    except (json.JSONDecodeError, ValueError):
        output_json = json.dumps({"text": text})

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
