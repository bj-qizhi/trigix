# ── Stage 1: build Rust backend ──────────────────────────────────────────────
FROM rust:1.82-slim AS rust-builder

RUN apt-get update && apt-get install -y pkg-config libssl-dev && rm -rf /var/lib/apt/lists/*

WORKDIR /build
COPY Cargo.toml Cargo.lock ./
COPY crates/ crates/
COPY services/ services/
COPY packages/ packages/

# Build release binary
RUN cargo build --release -p trigix-platform

# ── Stage 2: build React frontend ────────────────────────────────────────────
FROM node:20-alpine AS web-builder

WORKDIR /build
COPY apps/web/package.json apps/web/package-lock.json* ./
RUN npm ci --ignore-scripts

COPY apps/web/ ./
RUN npm run build

# ── Stage 3: runtime image ───────────────────────────────────────────────────
FROM debian:bookworm-slim AS runtime

RUN apt-get update && apt-get install -y ca-certificates nginx && rm -rf /var/lib/apt/lists/*

# Copy Rust binary
COPY --from=rust-builder /build/target/release/trigix-platform /usr/local/bin/trigix-platform

# Copy frontend dist into nginx web root
COPY --from=web-builder /build/dist /var/www/html

# Nginx config: serve frontend on 80, proxy /v1 /metrics /healthz to backend on 38080
COPY infra/nginx/default.conf /etc/nginx/sites-available/default

# Database migrations
COPY infra/postgres/migrations /app/migrations

WORKDIR /app

EXPOSE 80 38080

COPY infra/docker/entrypoint.sh /entrypoint.sh
RUN chmod +x /entrypoint.sh

ENTRYPOINT ["/entrypoint.sh"]
