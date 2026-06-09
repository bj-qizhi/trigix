# Copyright © 2026 北京祺智科技有限公司. All rights reserved.
# https://www.qzso.com/ · managecode@gmail.com

"""Live RAG integration test against a real pgvector database.

Skipped unless ``RAG_TEST_DATABASE_URL`` is set, e.g.:
    RAG_TEST_DATABASE_URL=postgresql://t:t@localhost:35495/t \
        pytest tests/test_rag_integration.py
"""

import os
import uuid

import pytest

from app.rag.chunking import chunk_text
from app.rag.store import RagStore

DSN = os.environ.get("RAG_TEST_DATABASE_URL")
pytestmark = pytest.mark.skipif(not DSN, reason="RAG_TEST_DATABASE_URL not set")


async def test_ingest_and_retrieve_most_relevant():
    store = await RagStore.connect(DSN)
    tenant, kb = "t1", f"kb-{uuid.uuid4().hex[:8]}"
    try:
        await store.ingest(tenant, kb, "doc-finance",
                           chunk_text("The finance team processes invoices and monthly billing.", 1000, 100))
        await store.ingest(tenant, kb, "doc-pets",
                           chunk_text("Our office has a garden where cats and dogs play together.", 1000, 100))
        await store.ingest(tenant, kb, "doc-eng",
                           chunk_text("Engineers deploy the Rust execution engine to Kubernetes.", 1000, 100))

        hits = await store.query(tenant, kb, "how are billing invoices handled?", top_k=3)
        assert hits, "expected at least one hit"
        # The finance doc must rank first for a billing query.
        assert hits[0].doc_id == "doc-finance"
        assert hits[0].score > hits[-1].score

        # Re-ingesting the same doc_id replaces, not duplicates.
        n = await store.ingest(tenant, kb, "doc-finance",
                              chunk_text("Updated finance note about invoices.", 1000, 100))
        assert n == 1
        finance_hits = [h for h in await store.query(tenant, kb, "invoices", top_k=10)
                        if h.doc_id == "doc-finance"]
        assert len(finance_hits) == 1

        # Deletion removes the doc.
        deleted = await store.delete_document(tenant, kb, "doc-finance")
        assert deleted == 1
    finally:
        # Clean up everything for this kb.
        await store.delete_document(tenant, kb, "doc-pets")
        await store.delete_document(tenant, kb, "doc-eng")
        await store.close()


async def test_hybrid_retrieval_finds_exact_token():
    store = await RagStore.connect(DSN)
    tenant, kb = "t-hyb", f"kb-{uuid.uuid4().hex[:8]}"
    try:
        # The exact identifier SKU-99421 lives in one doc; the others are noise.
        await store.ingest(tenant, kb, "doc-order",
                           chunk_text("订单 SKU-99421 已发货，预计三天送达。", 1000, 100))
        await store.ingest(tenant, kb, "doc-other",
                           chunk_text("库存盘点流程与仓库管理规范说明。", 1000, 100))

        hits = await store.query(tenant, kb, "SKU-99421", top_k=2, mode="hybrid")
        assert hits, "hybrid search returned nothing"
        assert hits[0].doc_id == "doc-order"

        # min_score gates weak vector hits.
        gated = await store.query(tenant, kb, "totally unrelated quantum topic",
                                  top_k=5, mode="vector", min_score=0.99)
        assert all(h.score >= 0.99 for h in gated)
    finally:
        await store.delete_document(tenant, kb, "doc-order")
        await store.delete_document(tenant, kb, "doc-other")
        await store.close()


async def test_rerank_reorders_candidates():
    store = await RagStore.connect(DSN)
    tenant, kb = "t-rr", f"kb-{uuid.uuid4().hex[:8]}"
    try:
        await store.ingest(tenant, kb, "doc-vacation",
                           chunk_text("Employees request annual leave through the HR portal.", 1000, 100))
        await store.ingest(tenant, kb, "doc-expense",
                           chunk_text("Submit travel expense reports within thirty days of the trip.", 1000, 100))
        await store.ingest(tenant, kb, "doc-onboard",
                           chunk_text("New hire onboarding covers accounts, equipment, and training.", 1000, 100))

        # With reranking, the expense-report doc should top a reimbursement query.
        hits = await store.query(
            tenant, kb, "how do I get reimbursed for travel expenses?",
            top_k=2, rerank=True,
        )
        assert hits, "rerank returned nothing"
        assert hits[0].doc_id == "doc-expense"
        # Scores are the reranker's relevance, sorted descending.
        assert hits[0].score >= hits[-1].score
    finally:
        for d in ("doc-vacation", "doc-expense", "doc-onboard"):
            await store.delete_document(tenant, kb, d)
        await store.close()


async def test_list_kbs_and_documents():
    store = await RagStore.connect(DSN)
    tenant, kb = "t-list", f"kb-{uuid.uuid4().hex[:8]}"
    try:
        await store.ingest(tenant, kb, "d1", chunk_text("alpha beta gamma " * 50, 100, 20))
        await store.ingest(tenant, kb, "d2", chunk_text("delta epsilon", 1000, 100))

        kbs = await store.list_kbs(tenant)
        mine = [k for k in kbs if k["kb"] == kb]
        assert len(mine) == 1
        assert mine[0]["docs"] == 2
        assert mine[0]["chunks"] >= 2

        docs = await store.list_documents(tenant, kb)
        ids = {d["doc_id"] for d in docs}
        assert ids == {"d1", "d2"}
        assert all(d["chunks"] >= 1 for d in docs)
    finally:
        await store.delete_document(tenant, kb, "d1")
        await store.delete_document(tenant, kb, "d2")
        await store.close()
