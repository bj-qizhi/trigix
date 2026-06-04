# Copyright © 2026 北京祺智科技有限公司. All rights reserved.
# https://www.qzso.com/ · managecode@gmail.com

import json

from fastapi.testclient import TestClient

from trigix_node_sdk import create_app, node, run_node


@node(slug="upper", label="Uppercase", config_schema={"type": "object"})
def upper(config, input, node_outputs):
    return {"out": str(input.get("text", "")).upper()}


def test_run_node_parses_input_and_returns_output_json():
    out = run_node("upper", {}, json.dumps({"text": "hi"}), {})
    assert json.loads(out) == {"out": "HI"}


def test_run_node_tolerates_bad_input_json():
    out = run_node("upper", {}, "not json", {})
    assert json.loads(out) == {"out": ""}


def test_http_contract_matches_executor():
    client = TestClient(create_app("http://node:9000"))

    # manifest lists the node with its endpoint
    manifest = client.get("/manifest").json()
    slugs = {n["slug"]: n for n in manifest["nodes"]}
    assert "upper" in slugs
    assert slugs["upper"]["endpoint"] == "http://node:9000/nodes/upper"

    # the executor contract: {node_id, config, input_json, node_outputs} -> {output_json}
    resp = client.post(
        "/nodes/upper",
        json={"node_id": "n1", "config": {}, "input_json": json.dumps({"text": "abc"}), "node_outputs": {}},
    )
    assert resp.status_code == 200
    assert json.loads(resp.json()["output_json"]) == {"out": "ABC"}


def test_unknown_node_returns_404():
    client = TestClient(create_app())
    resp = client.post("/nodes/ghost", json={"input_json": "{}"})
    assert resp.status_code == 404


def test_handler_error_returns_500():
    @node(slug="boom")
    def boom(config, input, node_outputs):
        raise ValueError("kaboom")

    client = TestClient(create_app())
    resp = client.post("/nodes/boom", json={"input_json": "{}"})
    assert resp.status_code == 500
    assert "kaboom" in resp.json()["detail"]
