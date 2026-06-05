# Demo: customer feedback triage

A recorded run of an end-to-end workflow built from custom nodes (the
[node SDK](../sdk)). It runs offline with no API keys, and is deterministic, so
the output below is reproducible.

The workflow takes a raw HTML feedback message, converts it to text, masks any
PII, scores sentiment, and decides how to route it:

```
trigger -> HTML to Text -> Redact PII
                       \-> Sentiment -> route
```

Redact PII and Sentiment both read the cleaned text from HTML to Text, so they
run in parallel after it.

To reproduce it, see [scripts/README.md](../scripts/README.md): start the
platform, start the example node service (`uvicorn examples.useful_nodes:app
--port 9000`), then run `python3 scripts/demo_feedback_triage.py`.

## The graph

The three custom nodes are wired with `from_node` so the downstream nodes
consume an upstream node's output rather than the workflow input.

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

## Run output

```text
[1/5] Registering custom nodes from the node service manifest
      HTML → Text    http://localhost:9000/nodes/html_to_text
      Redact PII     http://localhost:9000/nodes/redact_pii
      Sentiment      http://localhost:9000/nodes/sentiment

[2/5] Building the workflow: trigger -> clean -> (redact, sentiment)
   workflow=3a84d238-…  version=a5fcb3ab-…

[3/5] Publishing the workflow version

[4/5] Running on a raw HTML feedback message with PII
      input: <p>Honestly your app is <b>fast</b> and super reliable, I love it &amp...

[5/5] Execution ca31bbbe-…: succeeded

      HTML → Text [succeeded]
        {"text": "Honestly your app is fast and super reliable, I love it & recommend it!\nReach me at jane@corp.com or card 4111 1111 1111 1111.", "length": 126}
      Redact PII [succeeded]
        {"redacted": "Honestly your app is fast and super reliable, I love it & recommend it!\nReach me at [EMAIL] or card [CREDIT_CARD].", "counts": {"EMAIL": 1, "CREDIT_CARD": 1}, "total": 2}
      Sentiment [succeeded]
        {"label": "positive", "score": 1.0, "positive": 4, "negative": 0}

Triage decision:
      PII masked:  2 item(s) {'EMAIL': 1, 'CREDIT_CARD': 1}
      sentiment:   positive (score 1.0)
      route:       auto-acknowledge
```

## What each node did

| Node | Result |
|------|--------|
| HTML to Text | Removed tags, decoded `&amp;` to `&`, collapsed whitespace; produced 126 characters of text. |
| Redact PII | Replaced the email and the card number with `[EMAIL]` and `[CREDIT_CARD]` (2 matches). |
| Sentiment | Scored the cleaned text `positive` (1.0) on a word list. |

The final step routes on the sentiment label: a negative result would escalate
to a human; this one is auto-acknowledged.

## Notes

- The three nodes are about 150 lines of standard-library Python in
  `sdk/python/examples/useful_nodes.py`, served over HTTP by the SDK. The
  executor calls them like any built-in node; no changes to the Rust core are
  involved.
- `redact` and `score` execute in parallel because both depend only on `clean`;
  the executor schedules nodes by DAG level.
- For the AI nodes (RAG retrieval over pgvector and the agent tool-use loop),
  see `services/ai-runtime` and the `rag` / `agent` node types.
