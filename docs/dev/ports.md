# Development Ports

This project avoids common host ports because the development host may already run PostgreSQL, Redis, MinIO, Vite, and other application services.

## Current Defaults

| Service | Container Port | Host Port |
|---|---:|---:|
| PostgreSQL | `5432` | `35432` |
| Redis | `6379` | `36380` |
| MinIO API | `9000` | `39000` |
| MinIO Console | `9001` | `39001` |
| Rust Platform HTTP | n/a | `38080` |
| Rust Executor HTTP | n/a | `38090` |
| Python AI Runtime HTTP | n/a | `38070` |
| React Web Console | n/a | `3100` |

## Useful URLs

```text
PostgreSQL:     postgres://velara:velara@localhost:35432/velara
Redis:          localhost:36380
MinIO API:      http://localhost:39000
MinIO Console:  http://localhost:39001
Rust Platform:  http://127.0.0.1:38080
Rust Executor:  http://127.0.0.1:38090
Python Runtime: http://127.0.0.1:38070
Web Console:    http://localhost:3100
```

If any of these conflict on your machine, override the host port in `docker-compose.yml` and the matching value in `.env`.

The `make dev-verify` target checks the Platform and Executor API ports before starting temporary processes. Docker will also fail fast if an infrastructure port is already occupied.

It also checks for at least 1GB of free disk before starting PostgreSQL.
