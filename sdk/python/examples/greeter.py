# Copyright © 2026 北京祺智科技有限公司. All rights reserved.
# https://www.qzso.com/ · managecode@gmail.com

"""Example custom nodes. Run with:

    pip install -e .
    uvicorn examples.greeter:app --port 9000

Then register at Trigix (Custom Nodes settings), e.g. slug `greet`,
endpoint `http://localhost:9000/nodes/greet`.
"""

from trigix_node_sdk import create_app, node


@node(
    slug="greet",
    label="Greeter",
    description="Greets a name from config or input.",
    config_schema={
        "type": "object",
        "properties": {"name": {"type": "string", "title": "Name"}},
    },
)
def greet(config, input, node_outputs):
    name = config.get("name") or input.get("name", "world")
    return {"greeting": f"Hello, {name}!"}


@node(
    slug="word_count",
    label="Word Count",
    description="Counts words in the input text.",
    config_schema={
        "type": "object",
        "properties": {"field": {"type": "string", "title": "Input field", "default": "text"}},
    },
)
def word_count(config, input, node_outputs):
    field = config.get("field", "text")
    text = str(input.get(field, ""))
    return {"words": len(text.split()), "chars": len(text)}


app = create_app()
