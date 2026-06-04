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
