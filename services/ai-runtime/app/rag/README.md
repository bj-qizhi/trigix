# RAG (Retrieval-Augmented Generation)

A real, pgvector-backed knowledge store — ingest documents, embed them, and
retrieve the nearest chunks for a query.

## Pieces

- `chunking.py` — split documents into overlapping, boundary-aware windows.
- `embeddings.py` — OpenAI `text-embedding-3-small` when `OPENAI_API_KEY` is
  set; otherwise a deterministic local hashing embedding so RAG works offline
  and is fully testable. Both produce `EMBED_DIM` (default 1536) vectors.
- `store.py` — owns its pgvector schema (`af_kb_chunks`); ingest replaces a
  document's chunks, query does cosine-distance nearest-neighbour search.
- `router.py` — FastAPI endpoints.

## Endpoints

| Method | Path | Purpose |
|--------|------|---------|
| POST | `/v1/rag/ingest` | Chunk + embed + store a document into a knowledge base |
| POST | `/v1/rag/query` | Retrieve the top-k most relevant chunks for a query |
| DELETE | `/v1/rag/documents/{tenant_id}/{kb}/{doc_id}` | Remove a document |

Requires `DATABASE_URL` pointing at a Postgres with the `vector` extension.

## Tests

```bash
pip install -e ".[test]"
pytest                                            # unit tests (chunking, embeddings)
RAG_TEST_DATABASE_URL=postgresql://... pytest     # + live pgvector integration
```

## Next

Wire a RAG node into the executor so workflows can retrieve context and augment
prompts (`{{rag.results}}`), and an Agent tool-use loop on top.
