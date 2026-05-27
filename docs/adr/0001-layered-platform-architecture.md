# Layered Platform Architecture

We will design the platform as a layered system with React for the web console, Rust for platform business services and high-performance workflow execution, Python for AI Runtime capabilities, and PostgreSQL, Redis, MinIO, and pgvector for storage. This removes Go from the backend stack, lets Platform and Executor share Rust domain crates, and keeps fast-changing AI ecosystem work in Python.

## Considered Options

- Python-only backend: fastest for MVP, but weaker for long-term platform boundaries and high-performance execution.
- Java platform backend: strong enterprise ecosystem, but heavier than needed for this product direction.
- Go platform backend with Rust executor and Python AI Runtime: good balance, but keeps one more backend language in the system.
- Rust platform backend with Rust executor and Python AI Runtime: stronger type sharing, fewer backend languages, and better long-term reliability.

## Consequences

The architecture introduces Rust development complexity earlier than a Go or Python platform would. The trade-off is clearer ownership with fewer backend languages: Rust manages platform business plus execution reliability and performance, while Python manages Agent and RAG capabilities.
