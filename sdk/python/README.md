# Trigix Node SDK (Python)

Write a custom Trigix workflow node as a Python function and serve it over HTTP.
No changes to the Trigix executor are required — the executor calls your node
like any other.

## Quick start

```bash
pip install trigix-node-sdk
# or from this repo: pip install -e sdk/python
uvicorn examples.greeter:app --port 9000
```

```python
from trigix_node_sdk import node, create_app

@node(slug="greet", label="Greeter",
      config_schema={"type": "object",
                     "properties": {"name": {"type": "string"}}})
def greet(config, input, node_outputs):
    name = config.get("name") or input.get("name", "world")
    return {"greeting": f"Hello, {name}!"}

app = create_app()
```

Then in Trigix → **Custom Nodes**, register:
- **slug**: `greet`
- **endpoint**: `http://your-host:9000/nodes/greet`
- **config schema**: copy from `GET /manifest`

Add a **Custom** node to a workflow, pick `greet`, and it runs your code.

## Example nodes

`examples/useful_nodes.py` ships three practical, dependency-free nodes:

- **HTML → Text** — strip HTML to clean text (web scraping → LLM prep)
- **Redact PII** — mask emails / phone numbers / card numbers / IPs (compliance)
- **Sentiment** — lexicon-based sentiment label + score (route reviews/feedback)

```bash
uvicorn examples.useful_nodes:app --port 9000
```

## The contract

The executor POSTs to `/nodes/<slug>`:

```json
{ "node_id": "n1", "config": { "name": "Ada" },
  "input_json": "{\"...\":\"...\"}", "node_outputs": { "prev": "{...}" } }
```

Your handler `def handler(config, input, node_outputs) -> dict` receives the
parsed `input` and returns a JSON-serializable dict. The server wraps it as:

```json
{ "output_json": "{\"greeting\":\"Hello, Ada!\"}" }
```

Downstream nodes reference your output via `{{node_id.field}}`.

## Discovery

`GET /manifest` lists every registered node (slug, label, description,
config schema, endpoint) so it can be registered in Trigix in one step.

## Security

Custom nodes run in your own process, isolated from the Trigix core. Run the
node service on a trusted network; the executor reaches it by URL. (WASM-based
in-process isolation for untrusted nodes is a planned future option.)
