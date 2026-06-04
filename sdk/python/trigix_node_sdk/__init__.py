# Copyright © 2026 北京祺智科技有限公司. All rights reserved.
# https://www.qzso.com/ · managecode@gmail.com

"""Trigix custom node SDK.

Write a workflow node as a plain Python function and serve it over HTTP — no
changes to the Trigix executor required. Register the node's endpoint in Trigix
(Custom Nodes settings) and it appears in the workflow editor.

    from trigix_node_sdk import node, create_app

    @node(slug="greet", label="Greeter",
          config_schema={"type": "object",
                         "properties": {"name": {"type": "string"}}})
    def greet(config, input, node_outputs):
        name = config.get("name") or input.get("name", "world")
        return {"greeting": f"Hello, {name}!"}

    app = create_app()        # uvicorn module:app --port 9000

The node contract (matches the executor): the runtime POSTs
``{node_id, config, input_json, node_outputs}`` to ``/nodes/<slug>`` and expects
``{output_json}``. Your handler receives the parsed ``input`` dict and returns a
JSON-serializable dict.
"""

from __future__ import annotations

import json
from dataclasses import dataclass
from typing import Any, Callable

from pydantic import BaseModel

Handler = Callable[[dict, dict, dict], Any]


class NodeRequest(BaseModel):
    node_id: str = ""
    config: dict = {}
    input_json: str = "{}"
    node_outputs: dict = {}


@dataclass
class NodeDef:
    slug: str
    label: str
    description: str
    config_schema: dict
    handler: Handler


_REGISTRY: dict[str, NodeDef] = {}


def node(
    slug: str,
    label: str | None = None,
    description: str = "",
    config_schema: dict | None = None,
) -> Callable[[Handler], Handler]:
    """Register a function as a Trigix custom node."""

    def decorator(fn: Handler) -> Handler:
        _REGISTRY[slug] = NodeDef(
            slug=slug,
            label=label or slug,
            description=description,
            config_schema=config_schema or {"type": "object"},
            handler=fn,
        )
        return fn

    return decorator


def registry() -> dict[str, NodeDef]:
    return dict(_REGISTRY)


def run_node(slug: str, config: dict, input_json: str, node_outputs: dict) -> str:
    """Execute a registered node and return its ``output_json``. Raises
    KeyError for an unknown slug."""
    nd = _REGISTRY[slug]
    try:
        input_data = json.loads(input_json or "{}")
    except (json.JSONDecodeError, TypeError):
        input_data = {}
    result = nd.handler(config or {}, input_data, node_outputs or {})
    return json.dumps(result)


def create_app(base_url: str = ""):
    """Build a FastAPI app serving every registered node plus a discovery
    manifest. `base_url` is prefixed to the endpoint URLs in the manifest."""
    from fastapi import FastAPI, HTTPException

    app = FastAPI(title="Trigix Custom Nodes")

    @app.get("/healthz")
    def healthz() -> dict[str, str]:
        return {"status": "ok"}

    @app.get("/manifest")
    def manifest() -> dict[str, list[dict]]:
        return {
            "nodes": [
                {
                    "slug": d.slug,
                    "label": d.label,
                    "description": d.description,
                    "config_schema": d.config_schema,
                    "endpoint": f"{base_url}/nodes/{d.slug}",
                }
                for d in _REGISTRY.values()
            ]
        }

    @app.post("/nodes/{slug}")
    def run(slug: str, req: NodeRequest) -> dict[str, str]:
        if slug not in _REGISTRY:
            raise HTTPException(status_code=404, detail=f"unknown node '{slug}'")
        try:
            return {"output_json": run_node(slug, req.config, req.input_json, req.node_outputs)}
        except Exception as exc:  # surface handler errors as 500
            raise HTTPException(status_code=500, detail=f"node '{slug}' failed: {exc}") from exc

    return app
