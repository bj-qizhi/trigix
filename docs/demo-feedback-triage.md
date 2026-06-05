# Demo: customer feedback triage (annotated walkthrough)

This is a recorded, annotated run of an end-to-end Trigix workflow built
**entirely from custom nodes** written with the [node SDK](../sdk) — **no API
keys, fully offline, deterministic.** It shows the platform and the SDK working
together: write nodes → register them → wire a workflow → run it.

The pipeline takes a raw HTML feedback message, cleans it, masks any PII, scores
sentiment, and decides how to route it:

```
trigger ─→ HTML→Text ─→ Redact PII
                     └─→ Sentiment ─→ route
```

> Reproduce it yourself: see [scripts/README.md](../scripts/README.md). Start the
> platform, start the example node service (`uvicorn examples.useful_nodes:app
> --port 9000`), then run `python3 scripts/demo_feedback_triage.py`.

---

## ① Register the custom nodes

The script points the platform at a running node service and imports its
manifest — every node the service advertises is registered in one call.

```text
① Registering custom nodes from the node service manifest…
   • HTML → Text    http://localhost:9000/nodes/html_to_text
   • Redact PII     http://localhost:9000/nodes/redact_pii
   • Sentiment      http://localhost:9000/nodes/sentiment
```

> **What this proves:** the three nodes are plain Python functions
> (`sdk/python/examples/useful_nodes.py`) served over HTTP by the SDK. The
> executor never had to change to support them.

## ② Build & ③ publish the workflow

The workflow is a small graph. The three custom nodes are wired so that
**Redact PII** and **Sentiment** both consume the cleaned text produced by
**HTML→Text** (chaining via `from_node`):

```json
{
  "nodes": [
    { "id": "trigger", "type": "trigger" },
    { "id": "clean",  "type": "custom",
      "config": { "custom_node": "html_to_text", "field": "feedback_html" } },
    { "id": "redact", "type": "custom",
      "config": { "custom_node": "redact_pii", "from_node": "clean", "from_field": "text" } },
    { "id": "score",  "type": "custom",
      "config": { "custom_node": "sentiment", "from_node": "clean", "from_field": "text" } }
  ],
  "edges": [
    { "source": "trigger", "target": "clean" },
    { "source": "clean", "target": "redact" },
    { "source": "clean", "target": "score" }
  ]
}
```

```text
② Building the workflow: trigger → clean → (redact, sentiment)
   workflow=3110562a-…  version=0480750c-…
③ Publishing…
```

> **What this proves:** custom nodes are first-class graph citizens — they fan
> out and chain like any built-in node. `redact` and `score` run in parallel off
> `clean` (the executor schedules them by DAG level).

## ④ Run it on a realistic input

The input is a raw HTML feedback message that contains PII (an email and a card
number) — exactly the kind of text you must **not** forward to an LLM or a log
verbatim:

```text
④ Running on a raw HTML feedback message with PII…
   input: <p>Honestly your app is <b>fast</b> and super reliable, I love it &amp…
```

## ⑤ Result

```text
⑤ Result: execution de6563c7-… → succeeded

   ── HTML → Text [succeeded]
      {"text": "Honestly your app is fast and super reliable, I love it & recommend it!\nReach me at jane@corp.com or card 4111 1111 1111 1111.", "length": 126}
   ── Redact PII [succeeded]
      {"redacted": "Honestly your app is fast and super reliable, I love it & recommend it!\nReach me at [EMAIL] or card [CREDIT_CARD].", "counts": {"EMAIL": 1, "CREDIT_CARD": 1}, "total": 2}
   ── Sentiment [succeeded]
      {"label": "positive", "score": 1.0, "positive": 4, "negative": 0}

✅ Triage:
   • PII masked: 2 item(s) {'EMAIL': 1, 'CREDIT_CARD': 1}
   • Sentiment:  positive (score 1.0)
   • Route:      → auto-acknowledge
```

Stage by stage:

| Node | Did what | Output |
|------|----------|--------|
| **HTML→Text** | Stripped tags, decoded `&amp;` → `&`, collapsed whitespace | clean 126-char text |
| **Redact PII** | Masked the email and the card number before anything downstream sees them | `[EMAIL]`, `[CREDIT_CARD]`, `total: 2` |
| **Sentiment** | Scored the cleaned text on a lexicon | `positive`, `1.0` |

The final step routes on sentiment: a negative score would **escalate to a
human**; this positive one is **auto-acknowledged**.

---

## Why this matters

- **The node SDK is real.** Three useful nodes — HTML cleanup, PII redaction,
  sentiment — are ~150 lines of dependency-free Python, served over HTTP, and
  usable in a workflow without touching the Rust core.
- **Composability.** Custom nodes chain and fan out like built-ins; the executor
  runs the DAG (here `redact` and `score` execute in parallel).
- **Verifiable.** Everything here is offline and deterministic — no API keys, no
  flakiness. The same nodes ship with unit tests, and this exact run is
  reproducible from a clean clone.

For the AI-native side (RAG retrieval and an agent tool-use loop), see
`services/ai-runtime` and the `rag` / `agent` nodes.
