# Demo scripts

## `demo_feedback_triage.py` — end-to-end custom-node pipeline

A "customer feedback triage" workflow built entirely from [custom nodes](../sdk)
(the node SDK) — **no API keys, fully offline**:

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
① Registering custom nodes …  HTML → Text / Redact PII / Sentiment
② Building the workflow: trigger → clean → (redact, sentiment)
③ Publishing…
④ Running on a raw HTML feedback message with PII…
⑤ Result: execution … → succeeded
   ── HTML → Text   {"text": "Honestly your app is fast and super reliable…"}
   ── Redact PII    {"redacted": "… [EMAIL] or card [CREDIT_CARD].", "total": 2}
   ── Sentiment     {"label": "positive", "score": 1.0}
✅ Triage:  PII masked: 2 · Sentiment: positive · Route: → auto-acknowledge
```

This is a self-contained way to show the platform + the node SDK working
together end to end — clone, run three commands, see a real pipeline execute.
