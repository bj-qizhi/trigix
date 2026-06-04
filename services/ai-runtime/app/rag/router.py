# Copyright © 2026 北京祺智科技有限公司. All rights reserved.
# https://www.qzso.com/ · managecode@gmail.com

"""FastAPI endpoints for the RAG knowledge store."""

from __future__ import annotations

import os

from fastapi import APIRouter, HTTPException
from pydantic import BaseModel

from .chunking import chunk_text
from .embeddings import EMBED_DIM, using_openai
from .store import RagStore

router = APIRouter(prefix="/v1/rag", tags=["rag"])

_store: RagStore | None = None


async def get_store() -> RagStore:
    global _store
    if _store is None:
        dsn = os.environ.get("DATABASE_URL")
        if not dsn:
            raise HTTPException(
                status_code=503,
                detail="DATABASE_URL is not configured; RAG requires a pgvector database",
            )
        # asyncpg wants the scheme without the +driver suffix.
        dsn = dsn.replace("postgres+asyncpg://", "postgresql://").replace(
            "postgresql+asyncpg://", "postgresql://"
        )
        _store = await RagStore.connect(dsn)
    return _store


class IngestRequest(BaseModel):
    tenant_id: str
    kb: str
    doc_id: str
    text: str
    chunk_size: int = 1000
    overlap: int = 150


class IngestResponse(BaseModel):
    doc_id: str
    chunks: int
    backend: str
    dim: int


class QueryRequest(BaseModel):
    tenant_id: str
    kb: str
    query: str
    top_k: int = 4


class QueryResult(BaseModel):
    doc_id: str
    chunk_index: int
    content: str
    score: float


class QueryResponse(BaseModel):
    results: list[QueryResult]


@router.post("/ingest", response_model=IngestResponse)
async def ingest(req: IngestRequest) -> IngestResponse:
    store = await get_store()
    chunks = chunk_text(req.text, req.chunk_size, req.overlap)
    n = await store.ingest(req.tenant_id, req.kb, req.doc_id, chunks)
    return IngestResponse(
        doc_id=req.doc_id,
        chunks=n,
        backend="openai" if using_openai() else "local",
        dim=EMBED_DIM,
    )


@router.post("/query", response_model=QueryResponse)
async def query(req: QueryRequest) -> QueryResponse:
    store = await get_store()
    top_k = max(1, min(req.top_k, 50))
    hits = await store.query(req.tenant_id, req.kb, req.query, top_k)
    return QueryResponse(
        results=[
            QueryResult(
                doc_id=h.doc_id,
                chunk_index=h.chunk_index,
                content=h.content,
                score=h.score,
            )
            for h in hits
        ]
    )


@router.get("/kbs")
async def list_kbs(tenant_id: str) -> dict[str, list[dict]]:
    store = await get_store()
    return {"knowledge_bases": await store.list_kbs(tenant_id)}


@router.get("/documents")
async def list_documents(tenant_id: str, kb: str) -> dict[str, list[dict]]:
    store = await get_store()
    return {"documents": await store.list_documents(tenant_id, kb)}


@router.delete("/documents/{tenant_id}/{kb}/{doc_id}")
async def delete_document(tenant_id: str, kb: str, doc_id: str) -> dict[str, int]:
    store = await get_store()
    deleted = await store.delete_document(tenant_id, kb, doc_id)
    return {"deleted": deleted}
