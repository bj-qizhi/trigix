# Copyright © 2026 北京祺智科技有限公司. All rights reserved.
# https://www.qzso.com/ · managecode@gmail.com

"""Rerankers: reorder retrieved chunks by query-document relevance.

Two backends, mirroring the embedding design:

- **HttpReranker** — a real neural cross-encoder served over HTTP in the
  Cohere/Jina/BGE-compatible dialect (e.g. a self-hosted BAAI/bge-reranker via
  Xinference / vLLM / TEI / SiliconFlow, or DashScope). This is the production
  path and works in a China private deployment. Enabled by ``RERANK_BASE_URL``.
- **LocalReranker** — a deterministic, dependency-free lexical reranker used
  offline and as the fallback, so reranking works and is testable without a
  model server.
"""

from __future__ import annotations

import os
import re
from typing import Protocol

import httpx

_TOKEN_RE = re.compile(r"[a-z0-9]+")


class Reranker(Protocol):
    async def rerank(self, query: str, docs: list[str]) -> list[float]:
        """Return one relevance score per document, aligned to `docs`."""
        ...


class LocalReranker:
    """Score each document by how fully and densely it covers the query terms.

    Not a neural cross-encoder — a deterministic stand-in so the rerank path
    runs without a model server. Scores are in [0, 1].
    """

    async def rerank(self, query: str, docs: list[str]) -> list[float]:
        q = set(_TOKEN_RE.findall(query.lower()))
        if not q:
            return [0.0] * len(docs)
        scores: list[float] = []
        for doc in docs:
            toks = _TOKEN_RE.findall(doc.lower())
            if not toks:
                scores.append(0.0)
                continue
            coverage = len(q & set(toks)) / len(q)
            density = sum(1 for t in toks if t in q) / len(toks)
            scores.append(round(0.85 * coverage + 0.15 * min(density * 5.0, 1.0), 6))
        return scores


class HttpReranker:
    """A neural cross-encoder reranker behind a Cohere/Jina/BGE-style endpoint."""

    def __init__(self, base_url: str, model: str = "", api_key: str | None = None) -> None:
        url = base_url.rstrip("/")
        self._url = url if url.endswith("/rerank") else url + "/rerank"
        self._model = model
        self._api_key = api_key

    async def rerank(self, query: str, docs: list[str]) -> list[float]:
        if not docs:
            return []
        headers = {"Authorization": f"Bearer {self._api_key}"} if self._api_key else None
        body: dict = {"query": query, "documents": docs, "top_n": len(docs)}
        if self._model:
            body["model"] = self._model
        async with httpx.AsyncClient(timeout=30.0) as client:
            resp = await client.post(self._url, json=body, headers=headers)
            resp.raise_for_status()
            data = resp.json()
        results = data.get("results") if isinstance(data, dict) else data
        scores = [0.0] * len(docs)
        for item in results or []:
            idx = item.get("index")
            if idx is None or not (0 <= idx < len(docs)):
                continue
            scores[idx] = float(item.get("relevance_score", item.get("score", 0.0)) or 0.0)
        return scores


def get_reranker() -> Reranker:
    """Resolve the active reranker: a real cross-encoder when RERANK_BASE_URL is
    set, otherwise the local lexical fallback."""
    base = os.environ.get("RERANK_BASE_URL")
    if base:
        return HttpReranker(
            base,
            os.environ.get("RERANK_MODEL", "bge-reranker-v2-m3"),
            os.environ.get("RERANK_API_KEY"),
        )
    return LocalReranker()
