# RAG (Retrieval-Augmented Generation)

A real, pgvector-backed knowledge store — ingest documents, embed them, and
retrieve the nearest chunks for a query.

## Pieces

- `chunking.py` — split documents into overlapping, boundary-aware windows.
- `embeddings.py` — OpenAI `text-embedding-3-small` when `OPENAI_API_KEY` is
  set; otherwise a deterministic local hashing embedding so RAG works offline
  and is fully testable. Both produce `EMBED_DIM` (default 1536) vectors.
- `store.py` — owns its pgvector schema (`af_kb_chunks`) with an HNSW vector
  index and a full-text index; ingest replaces a document's chunks, query
  retrieves the most relevant ones.
- `rerank.py` — optional cross-encoder reranking: a Cohere/Jina/BGE-compatible
  HTTP reranker (`RERANK_BASE_URL`) or a local lexical fallback.
- `router.py` — FastAPI endpoints.

## Endpoints

| Method | Path | Purpose |
|--------|------|---------|
| POST | `/v1/rag/ingest` | Chunk + embed + store a document into a knowledge base |
| POST | `/v1/rag/query` | Retrieve the top-k most relevant chunks for a query |
| DELETE | `/v1/rag/documents/{tenant_id}/{kb}/{doc_id}` | Remove a document |

Requires `DATABASE_URL` pointing at a Postgres with the `vector` extension.

## Retrieval

`POST /v1/rag/query` accepts:

- `mode`: `vector` (cosine, default) or `hybrid` (Reciprocal Rank Fusion of the
  vector ranking and a full-text ranking — good for exact codes/identifiers).
- `min_score`: drop weakly-related chunks (vector mode).
- `rerank`: pull a larger candidate pool and reorder it with a cross-encoder.

These compose: hybrid → rerank → score floor.

### Configuration

| Env | Effect |
|-----|--------|
| `OPENAI_API_KEY` | Use OpenAI embeddings instead of the local fallback |
| `RERANK_BASE_URL` / `RERANK_MODEL` / `RERANK_API_KEY` | Enable a real cross-encoder reranker |
| `RAG_FTS_CONFIG` | Text-search config for hybrid's lexical side |

Hybrid's keyword side uses a Postgres text-search config. `simple` does not
segment Chinese, so the store auto-selects a CJK config (`jiebacfg`,
`zhparsercfg`, …) when one is installed — install **pg_jieba** or **zhparser**
for good Chinese keyword matching, or point `RAG_FTS_CONFIG` at your own.

## Tests

```bash
pip install -e ".[test]"
pytest                                            # unit tests (chunking, embeddings)
RAG_TEST_DATABASE_URL=postgresql://... pytest     # + live pgvector integration
```

The RAG node (`{{rag.results}}`) and the agent's `rag_search` tool both go
through this store.
