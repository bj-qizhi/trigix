# Copyright © 2026 北京祺智科技有限公司. All rights reserved.
# https://www.qzso.com/ · managecode@gmail.com

"""Practical example nodes built with the Trigix node SDK — standard library
only, no API keys, fully offline. Run with:

    uvicorn examples.useful_nodes:app --port 9000

Then register http://localhost:9000 in Trigix → Custom Nodes → Import All.
"""

from __future__ import annotations

import json
import re
from html.parser import HTMLParser

from trigix_node_sdk import create_app, node


def _read_source(config, input, node_outputs, default_field: str) -> str:
    """Read the node's text input — from an upstream node's output when
    `from_node` (+ optional `from_field`) is set, otherwise from the workflow
    `input[field]`. Lets nodes be chained into a pipeline."""
    from_node = config.get("from_node")
    if from_node:
        raw = node_outputs.get(from_node, "")
        try:
            data = json.loads(raw) if raw else {}
        except (json.JSONDecodeError, TypeError):
            data = {}
        return str(data.get(config.get("from_field", default_field), ""))
    return str(input.get(config.get("field", default_field), ""))

# ── html_to_text ────────────────────────────────────────────────────────────
# Turn raw HTML into clean plain text — the standard preprocessing step before
# feeding scraped pages to an LLM.

_BLOCK_TAGS = {"p", "br", "div", "li", "tr", "h1", "h2", "h3", "h4", "h5", "h6"}


class _TextExtractor(HTMLParser):
    def __init__(self, keep_links: bool) -> None:
        super().__init__(convert_charrefs=True)
        self.parts: list[str] = []
        self._skip = 0
        self._keep_links = keep_links
        self._href: str | None = None

    def handle_starttag(self, tag, attrs):
        if tag in ("script", "style"):
            self._skip += 1
        if tag in _BLOCK_TAGS:
            self.parts.append("\n")
        if tag == "a" and self._keep_links:
            self._href = dict(attrs).get("href")

    def handle_endtag(self, tag):
        if tag in ("script", "style") and self._skip:
            self._skip -= 1
        if tag == "a" and self._keep_links and self._href:
            self.parts.append(f" ({self._href})")
            self._href = None

    def handle_data(self, data):
        if not self._skip:
            self.parts.append(data)


def html_to_text(html: str, keep_links: bool = False) -> str:
    parser = _TextExtractor(keep_links)
    parser.feed(html)
    text = "".join(parser.parts)
    lines = [re.sub(r"[ \t]+", " ", ln).strip() for ln in text.splitlines()]
    return "\n".join(ln for ln in lines if ln)


@node(
    slug="html_to_text",
    label="HTML → Text",
    description="Strip HTML to clean plain text (drops script/style, collapses whitespace).",
    config_schema={
        "type": "object",
        "properties": {
            "field": {"type": "string", "title": "Input field", "default": "html"},
            "keep_links": {"type": "boolean", "title": "Append link URLs"},
        },
    },
)
def html_to_text_node(config, input, node_outputs):
    src = _read_source(config, input, node_outputs, "html")
    text = html_to_text(src, bool(config.get("keep_links", False)))
    return {"text": text, "length": len(text)}


# ── redact_pii ──────────────────────────────────────────────────────────────
# Mask emails / phone numbers / card numbers / IPs before sending text to an
# LLM or writing it to logs — a common compliance requirement.

_PII_PATTERNS = {
    "EMAIL": re.compile(r"[A-Za-z0-9._%+-]+@[A-Za-z0-9.-]+\.[A-Za-z]{2,}"),
    "CREDIT_CARD": re.compile(r"\b(?:\d[ -]?){13,16}\b"),
    "IPV4": re.compile(r"\b(?:\d{1,3}\.){3}\d{1,3}\b"),
    "PHONE": re.compile(r"\+?\d[\d\s().-]{7,}\d"),
}
# Apply in an order that avoids a phone pattern eating a card/IP.
_PII_ORDER = ["EMAIL", "CREDIT_CARD", "IPV4", "PHONE"]


def redact_pii(text: str, categories: list[str] | None = None) -> tuple[str, dict]:
    active = categories or _PII_ORDER
    counts: dict[str, int] = {}
    for cat in _PII_ORDER:
        if cat not in active:
            continue
        pattern = _PII_PATTERNS[cat]
        text, n = pattern.subn(f"[{cat}]", text)
        if n:
            counts[cat] = n
    return text, counts


@node(
    slug="redact_pii",
    label="Redact PII",
    description="Mask emails, phone numbers, card numbers and IPs in text.",
    config_schema={
        "type": "object",
        "properties": {
            "field": {"type": "string", "title": "Input field", "default": "text"},
            "categories": {
                "type": "string",
                "title": "Categories (comma-separated; blank = all)",
            },
        },
    },
)
def redact_pii_node(config, input, node_outputs):
    cats = config.get("categories")
    categories = [c.strip().upper() for c in cats.split(",")] if cats else None
    redacted, counts = redact_pii(_read_source(config, input, node_outputs, "text"), categories)
    return {"redacted": redacted, "counts": counts, "total": sum(counts.values())}


# ── sentiment ───────────────────────────────────────────────────────────────
# Lexicon-based sentiment score for routing reviews / feedback. Deterministic,
# no model required.

_POSITIVE = {
    "good", "great", "excellent", "amazing", "love", "loved", "wonderful",
    "fantastic", "happy", "best", "perfect", "awesome", "helpful", "fast",
    "recommend", "recommended", "satisfied", "nice", "pleasant", "reliable",
}
_NEGATIVE = {
    "bad", "terrible", "awful", "hate", "hated", "horrible", "worst", "slow",
    "broken", "useless", "disappointed", "poor", "buggy", "crash", "crashes",
    "annoying", "frustrating", "unreliable", "expensive", "confusing", "fail",
}


def sentiment(text: str) -> dict:
    tokens = re.findall(r"[a-z']+", text.lower())
    pos = sum(t in _POSITIVE for t in tokens)
    neg = sum(t in _NEGATIVE for t in tokens)
    total = pos + neg
    score = 0.0 if total == 0 else round((pos - neg) / total, 3)
    label = "neutral"
    if score > 0.2:
        label = "positive"
    elif score < -0.2:
        label = "negative"
    return {"label": label, "score": score, "positive": pos, "negative": neg}


@node(
    slug="sentiment",
    label="Sentiment",
    description="Lexicon-based sentiment label + score (positive / neutral / negative).",
    config_schema={
        "type": "object",
        "properties": {"field": {"type": "string", "title": "Input field", "default": "text"}},
    },
)
def sentiment_node(config, input, node_outputs):
    return sentiment(_read_source(config, input, node_outputs, "text"))


app = create_app()
