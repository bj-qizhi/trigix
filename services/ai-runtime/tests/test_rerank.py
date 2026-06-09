# Copyright © 2026 北京祺智科技有限公司. All rights reserved.
# https://www.qzso.com/ · managecode@gmail.com

"""Unit tests for the rerankers — no model server or network required."""

import os

from app.rag import rerank as rr
from app.rag.rerank import HttpReranker, LocalReranker, get_reranker


async def test_local_reranker_orders_by_query_coverage():
    docs = [
        "the cat sat on the mat",
        "invoices and billing are handled by the finance team",
        "a short unrelated note",
    ]
    scores = await LocalReranker().rerank("how is billing handled for invoices", docs)
    assert len(scores) == 3
    # The billing/invoices doc must score highest.
    assert scores[1] == max(scores)
    assert scores[1] > scores[0]


async def test_local_reranker_empty_query_is_neutral():
    assert await LocalReranker().rerank("", ["anything", "else"]) == [0.0, 0.0]


# ── HttpReranker against a fake Cohere/Jina-style endpoint ──────────────────


class _Resp:
    def __init__(self, payload):
        self._payload = payload
        self.status_code = 200

    def raise_for_status(self):
        pass

    def json(self):
        return self._payload


class _Client:
    def __init__(self, payload, capture):
        self._payload = payload
        self._capture = capture

    async def __aenter__(self):
        return self

    async def __aexit__(self, *exc):
        return False

    async def post(self, url, **kw):
        self._capture.update(url=url, **kw)
        return _Resp(self._payload)


async def test_http_reranker_parses_results_by_index(monkeypatch):
    capture: dict = {}
    payload = {"results": [
        {"index": 2, "relevance_score": 0.9},
        {"index": 0, "relevance_score": 0.1},
        {"index": 1, "relevance_score": 0.5},
    ]}
    monkeypatch.setattr(rr.httpx, "AsyncClient", lambda **kw: _Client(payload, capture))
    reranker = HttpReranker("http://rerank:9997/v1", model="bge-reranker-v2-m3", api_key="k")
    scores = await reranker.rerank("q", ["a", "b", "c"])
    assert scores == [0.1, 0.5, 0.9]  # realigned to input order
    assert capture["url"] == "http://rerank:9997/v1/rerank"
    assert capture["json"]["documents"] == ["a", "b", "c"]
    assert capture["headers"]["Authorization"] == "Bearer k"


def test_get_reranker_selects_backend(monkeypatch):
    monkeypatch.delenv("RERANK_BASE_URL", raising=False)
    assert isinstance(get_reranker(), LocalReranker)
    monkeypatch.setenv("RERANK_BASE_URL", "http://rerank:9997/v1")
    assert isinstance(get_reranker(), HttpReranker)
    os.environ.pop("RERANK_BASE_URL", None)
