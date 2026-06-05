#!/usr/bin/env python3
# Copyright © 2026 北京祺智科技有限公司. All rights reserved.
# https://www.qzso.com/ · managecode@gmail.com

"""End-to-end Trigix demo: a "customer feedback triage" pipeline built entirely
from custom nodes (the node SDK) — no API keys, fully offline.

Pipeline:  trigger → HTML→Text → Redact PII → Sentiment

It registers the custom nodes from a running node service, imports + publishes
a workflow, runs it on a raw HTML feedback message containing PII, and prints
each node's output.

Prerequisites (two terminals):
  1. Platform:      cargo run -p trigix-platform           # :38080
  2. Node service:  cd sdk/python && pip install -e . \\
                    && uvicorn examples.useful_nodes:app --port 9000

Then:  python3 scripts/demo_feedback_triage.py
"""

from __future__ import annotations

import argparse
import json
import sys
import time
import urllib.error
import urllib.request

TENANT = "tenant-1"


def call(method: str, url: str, body: dict | None = None) -> dict:
    data = json.dumps(body).encode() if body is not None else None
    req = urllib.request.Request(url, data=data, method=method)
    req.add_header("content-type", "application/json")
    try:
        with urllib.request.urlopen(req, timeout=30) as resp:
            raw = resp.read()
            return json.loads(raw) if raw else {}
    except urllib.error.HTTPError as e:
        sys.exit(f"  ✗ {method} {url} → {e.code}: {e.read().decode()[:200]}")
    except urllib.error.URLError as e:
        sys.exit(f"  ✗ {method} {url} unreachable: {e}. Is the service running?")


def main() -> None:
    ap = argparse.ArgumentParser()
    ap.add_argument("--platform", default="http://localhost:38080")
    ap.add_argument("--node-service", default="http://localhost:9000")
    args = ap.parse_args()
    platform = args.platform.rstrip("/")
    node_svc = args.node_service.rstrip("/")

    print("① Registering custom nodes from the node service manifest…")
    imported = call("POST", f"{platform}/v1/custom-nodes/import", {"base_url": node_svc})
    by_slug = {n["slug"]: n for n in imported}
    for n in imported:
        print(f"   • {n['label']:14s} {n['endpoint']}")
    for need in ("html_to_text", "redact_pii", "sentiment"):
        if need not in by_slug:
            sys.exit(f"   ✗ node service is missing '{need}'. Run examples.useful_nodes.")

    def custom(node_id: str, slug: str, cfg: dict) -> dict:
        return {
            "id": node_id,
            "type": "custom",
            "config": {"custom_node": slug, "endpoint": by_slug[slug]["endpoint"], **cfg},
        }

    print("\n② Building the workflow: trigger → clean → (redact, sentiment)")
    graph = {
        "workflow_version_id": "ignored",
        "nodes": [
            {"id": "trigger", "type": "trigger"},
            custom("clean", "html_to_text", {"field": "feedback_html"}),
            custom("redact", "redact_pii", {"from_node": "clean", "from_field": "text"}),
            custom("score", "sentiment", {"from_node": "clean", "from_field": "text"}),
        ],
        "edges": [
            {"source": "trigger", "target": "clean"},
            {"source": "clean", "target": "redact"},
            {"source": "clean", "target": "score"},
        ],
    }

    wf = call("POST", f"{platform}/v1/workflows/import", {
        "tenant_id": TENANT,
        "workspace_id": "workspace-1",
        "project_id": "project-1",
        "name": "Feedback Triage (demo)",
        "graph": graph,
    })
    version_id = wf["latest_version_id"]
    print(f"   workflow={wf['id']}  version={version_id}")

    print("\n③ Publishing…")
    call("POST", f"{platform}/v1/workflow-versions/{version_id}/publish", {"tenant_id": TENANT})

    feedback = (
        "<p>Honestly your app is <b>fast</b> and super reliable, I love it &amp; "
        "recommend it!</p><p>Reach me at jane@corp.com or card 4111 1111 1111 1111.</p>"
    )
    print("\n④ Running on a raw HTML feedback message with PII…")
    print(f"   input: {feedback[:70]}…")
    ex = call("POST", f"{platform}/v1/workflows/{wf['id']}/executions", {
        "tenant_id": TENANT,
        "input_json": json.dumps({"feedback_html": feedback}),
    })
    exec_id = ex["id"]

    # Poll to completion.
    for _ in range(40):
        ex = call("GET", f"{platform}/v1/executions/{exec_id}?tenant_id={TENANT}")
        if ex.get("status") in ("succeeded", "failed", "cancelled"):
            break
        time.sleep(0.25)

    print(f"\n⑤ Result: execution {exec_id} → {ex.get('status')}\n")
    outputs = {nr["node_id"]: nr for nr in ex.get("node_results", [])}

    def show(node_id: str, title: str) -> dict:
        nr = outputs.get(node_id, {})
        out = {}
        if nr.get("output_json"):
            try:
                out = json.loads(nr["output_json"])
            except (json.JSONDecodeError, TypeError):
                out = {"raw": nr["output_json"]}
        print(f"   ── {title} [{nr.get('status', '?')}]")
        print(f"      {json.dumps(out, ensure_ascii=False)}")
        return out

    show("clean", "HTML → Text")
    redact = show("redact", "Redact PII")
    score = show("score", "Sentiment")

    print("\n✅ Triage:")
    print(f"   • PII masked: {redact.get('total', 0)} item(s) {redact.get('counts', {})}")
    print(f"   • Sentiment:  {score.get('label', '?')} (score {score.get('score', '?')})")
    route = "escalate to a human" if score.get("label") == "negative" else "auto-acknowledge"
    print(f"   • Route:      → {route}")


if __name__ == "__main__":
    main()
