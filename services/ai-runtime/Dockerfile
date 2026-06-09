# Trigix AI Runtime — RAG retrieval + agent tool-use loop (FastAPI).
FROM python:3.12-slim

WORKDIR /app

# Install dependencies first so the layer caches across app-code changes.
COPY pyproject.toml ./
COPY app/ ./app/
RUN pip install --no-cache-dir ".[openai]"

EXPOSE 8000

CMD ["uvicorn", "app.main:app", "--host", "0.0.0.0", "--port", "8000"]
