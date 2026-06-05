# Demo scripts

## `demo_feedback_triage.py`: end-to-end custom-node pipeline

A "customer feedback triage" workflow built entirely from [custom nodes](../sdk)
(the node SDK). No API keys, fully offline:

```
trigger → HTML→Text → Redact PII → Sentiment → route
```

The script registers the custom nodes from a running node service, imports +
publishes a workflow, runs it on a raw HTML feedback message containing PII, and
prints each node's output.

### Run it

Three terminals:

```bash
# 1. Platform (in-memory mode is fine)
PLATFORM_HTTP_ADDR=127.0.0.1:38080 cargo run -p trigix-platform

# 2. The example node service (the SDK's useful nodes)
cd sdk/python && pip install -e . && uvicorn examples.useful_nodes:app --port 9000

# 3. The demo
python3 scripts/demo_feedback_triage.py
# options: --platform http://localhost:38080  --node-service http://localhost:9000
```

### Expected output (abridged)

```
[1/5] Registering custom nodes from the node service manifest
[2/5] Building the workflow: trigger -> clean -> (redact, sentiment)
[3/5] Publishing the workflow version
[4/5] Running on a raw HTML feedback message with PII
[5/5] Execution ...: succeeded
      Redact PII   {"redacted": "... [EMAIL] or card [CREDIT_CARD].", "total": 2}
      Sentiment    {"label": "positive", "score": 1.0}
Triage decision:  PII masked 2, sentiment positive, route auto-acknowledge
```

A full annotated run is in [docs/demo-feedback-triage.md](../docs/demo-feedback-triage.md).
