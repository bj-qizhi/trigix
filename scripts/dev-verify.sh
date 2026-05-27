#!/usr/bin/env bash
set -euo pipefail

ROOT_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)"
cd "$ROOT_DIR"

POSTGRES_PORT="${POSTGRES_PORT:-35432}"
REDIS_PORT="${REDIS_PORT:-36380}"
MINIO_API_PORT="${MINIO_API_PORT:-39000}"
MINIO_CONSOLE_PORT="${MINIO_CONSOLE_PORT:-39001}"
PLATFORM_PORT="${PLATFORM_PORT:-38080}"
EXECUTOR_PORT="${EXECUTOR_PORT:-38090}"

DATABASE_URL="${DATABASE_URL:-postgres://agentflow:agentflow@localhost:${POSTGRES_PORT}/agentflow}"
PLATFORM_HTTP_ADDR="${PLATFORM_HTTP_ADDR:-127.0.0.1:${PLATFORM_PORT}}"
EXECUTOR_HTTP_ADDR="${EXECUTOR_HTTP_ADDR:-127.0.0.1:${EXECUTOR_PORT}}"

TENANT_ID="00000000-0000-4000-8000-000000000001"
WORKFLOW_ID="00000000-0000-4000-8000-000000000005"
WORKFLOW_VERSION_ID="00000000-0000-4000-8000-000000000006"

PLATFORM_PID=""
EXECUTOR_PID=""

cleanup() {
  if [[ -n "$PLATFORM_PID" ]] && kill -0 "$PLATFORM_PID" 2>/dev/null; then
    kill "$PLATFORM_PID" 2>/dev/null || true
    wait "$PLATFORM_PID" 2>/dev/null || true
  fi
  if [[ -n "$EXECUTOR_PID" ]] && kill -0 "$EXECUTOR_PID" 2>/dev/null; then
    kill "$EXECUTOR_PID" 2>/dev/null || true
    wait "$EXECUTOR_PID" 2>/dev/null || true
  fi
}
trap cleanup EXIT

require_cmd() {
  if ! command -v "$1" >/dev/null 2>&1; then
    echo "missing required command: $1" >&2
    exit 1
  fi
}

require_cmd cargo
require_cmd curl
require_cmd docker
require_cmd python3

require_disk_space() {
  local min_kb="${1:-1048576}"
  local available_kb
  available_kb="$(df -Pk "$ROOT_DIR" | awk 'NR == 2 {print $4}')"

  if [[ -z "$available_kb" ]]; then
    echo "could not determine available disk space" >&2
    exit 1
  fi

  if (( available_kb < min_kb )); then
    echo "not enough disk space for dev verification" >&2
    echo "available: ${available_kb} KB; required: ${min_kb} KB" >&2
    echo "run: docker system df" >&2
    echo "consider freeing space before starting PostgreSQL" >&2
    exit 1
  fi
}

check_port_free() {
  local port="$1"
  if command -v ss >/dev/null 2>&1 && ss -ltn 2>/dev/null | grep -qE ":${port}\\b"; then
    echo "port ${port} is already in use" >&2
    exit 1
  fi
}

wait_http() {
  local url="$1"
  local attempts="${2:-60}"

  for _ in $(seq 1 "$attempts"); do
    if curl -fsS "$url" >/dev/null 2>&1; then
      return 0
    fi
    sleep 1
  done

  echo "timed out waiting for ${url}" >&2
  exit 1
}

wait_postgres() {
  local attempts="${1:-60}"

  for _ in $(seq 1 "$attempts"); do
    if docker compose exec -T postgres pg_isready -U agentflow -d agentflow >/dev/null 2>&1; then
      return 0
    fi
    sleep 1
  done

  echo "timed out waiting for postgres" >&2
  echo "recent postgres logs:" >&2
  docker compose logs postgres --tail=40 >&2 || true
  exit 1
}

echo "checking project ports"
require_disk_space 1048576
check_port_free "$PLATFORM_PORT"
check_port_free "$EXECUTOR_PORT"

echo "starting docker infrastructure"
docker compose up -d postgres redis minio

echo "waiting for postgres"
wait_postgres

echo "running tests"
cargo test
python3 -m py_compile services/ai-runtime/app/main.py
docker compose config >/dev/null

echo "starting rust executor on ${EXECUTOR_HTTP_ADDR}"
EXECUTOR_HTTP_ADDR="$EXECUTOR_HTTP_ADDR" \
  cargo run -p agentflow-executor >/tmp/agentflow-executor-dev-verify.log 2>&1 &
EXECUTOR_PID="$!"

wait_http "http://${EXECUTOR_HTTP_ADDR}/healthz"

echo "running executor service smoke test"
EXECUTOR_RESPONSE="$(
  curl -fsS -X POST "http://${EXECUTOR_HTTP_ADDR}/v1/executions:run" \
    -H 'Content-Type: application/json' \
    -d "{
      \"execution_id\": \"dev-verify-execution\",
      \"graph\": {
        \"workflow_version_id\": \"${WORKFLOW_VERSION_ID}\",
        \"nodes\": [
          {\"id\": \"trigger\", \"type\": \"trigger\"},
          {\"id\": \"agent\", \"type\": \"agent\"}
        ],
        \"edges\": [
          {\"source\": \"trigger\", \"target\": \"agent\"}
        ]
      },
      \"input_json\": \"{\\\"lead_id\\\":\\\"lead-dev-verify\\\"}\"
    }"
)"

python3 -c '
import json
import sys

payload = json.load(sys.stdin)

assert payload["execution_id"] == "dev-verify-execution", payload
assert payload["status"] == "succeeded", payload
assert [node["node_id"] for node in payload["node_results"]] == ["trigger", "agent"], payload
' <<<"$EXECUTOR_RESPONSE"

echo "starting rust platform on ${PLATFORM_HTTP_ADDR}"
DATABASE_URL="$DATABASE_URL" \
  PLATFORM_HTTP_ADDR="$PLATFORM_HTTP_ADDR" \
  EXECUTOR_BASE_URL="http://${EXECUTOR_HTTP_ADDR}" \
  cargo run -p agentflow-platform >/tmp/agentflow-platform-dev-verify.log 2>&1 &
PLATFORM_PID="$!"

wait_http "http://${PLATFORM_HTTP_ADDR}/healthz"

echo "loading workflow version ${WORKFLOW_VERSION_ID}"
WORKFLOW_VERSION_RESPONSE="$(
  curl -fsS "http://${PLATFORM_HTTP_ADDR}/v1/workflow-versions/${WORKFLOW_VERSION_ID}?tenant_id=${TENANT_ID}"
)"

python3 -c '
import json
import sys

payload = json.load(sys.stdin)

assert payload["id"] == "00000000-0000-4000-8000-000000000006", payload
assert payload["tenant_id"] == "00000000-0000-4000-8000-000000000001", payload
assert payload["workflow_id"] == "00000000-0000-4000-8000-000000000005", payload
assert payload["status"] == "published", payload
assert [node["id"] for node in payload["graph"]["nodes"]] == ["trigger", "agent"], payload
' <<<"$WORKFLOW_VERSION_RESPONSE"

echo "creating workflow"
CREATE_WORKFLOW_RESPONSE="$(
  curl -fsS -X POST "http://${PLATFORM_HTTP_ADDR}/v1/workflows" \
    -H 'Content-Type: application/json' \
    -d "{
      \"tenant_id\": \"${TENANT_ID}\",
      \"workspace_id\": \"00000000-0000-4000-8000-000000000003\",
      \"project_id\": \"00000000-0000-4000-8000-000000000004\",
      \"name\": \"Dev Verify Workflow\"
    }"
)"

WORKFLOW_ID_TO_RUN="$(
  python3 -c 'import json,sys; print(json.load(sys.stdin)["id"])' <<<"$CREATE_WORKFLOW_RESPONSE"
)"

python3 -c '
import json
import sys

expected_id = sys.argv[1]
payload = json.load(sys.stdin)

assert payload["id"] == expected_id, payload
assert payload["tenant_id"] == "00000000-0000-4000-8000-000000000001", payload
assert payload["workspace_id"] == "00000000-0000-4000-8000-000000000003", payload
assert payload["project_id"] == "00000000-0000-4000-8000-000000000004", payload
assert payload["name"] == "Dev Verify Workflow", payload
assert payload["status"] == "draft", payload
assert payload["latest_version_id"] is None, payload
' "$WORKFLOW_ID_TO_RUN" <<<"$CREATE_WORKFLOW_RESPONSE"

echo "loading workflow ${WORKFLOW_ID_TO_RUN}"
GET_WORKFLOW_RESPONSE="$(
  curl -fsS "http://${PLATFORM_HTTP_ADDR}/v1/workflows/${WORKFLOW_ID_TO_RUN}?tenant_id=${TENANT_ID}"
)"

python3 -c '
import json
import sys

expected_id = sys.argv[1]
payload = json.load(sys.stdin)

assert payload["id"] == expected_id, payload
assert payload["tenant_id"] == "00000000-0000-4000-8000-000000000001", payload
assert payload["name"] == "Dev Verify Workflow", payload
assert payload["status"] == "draft", payload
assert payload["latest_version_id"] is None, payload
' "$WORKFLOW_ID_TO_RUN" <<<"$GET_WORKFLOW_RESPONSE"

echo "updating workflow ${WORKFLOW_ID_TO_RUN}"
UPDATE_WORKFLOW_RESPONSE="$(
  curl -fsS -X PATCH "http://${PLATFORM_HTTP_ADDR}/v1/workflows/${WORKFLOW_ID_TO_RUN}" \
    -H 'Content-Type: application/json' \
    -d "{
      \"tenant_id\": \"${TENANT_ID}\",
      \"name\": \"Dev Verify Workflow Renamed\"
    }"
)"

python3 -c '
import json
import sys

expected_id = sys.argv[1]
payload = json.load(sys.stdin)

assert payload["id"] == expected_id, payload
assert payload["name"] == "Dev Verify Workflow Renamed", payload
assert payload["status"] == "draft", payload
assert payload["latest_version_id"] is None, payload
' "$WORKFLOW_ID_TO_RUN" <<<"$UPDATE_WORKFLOW_RESPONSE"

echo "listing workflows"
WORKFLOW_LIST_RESPONSE="$(
  curl -fsS "http://${PLATFORM_HTTP_ADDR}/v1/workflows?tenant_id=${TENANT_ID}&project_id=00000000-0000-4000-8000-000000000004"
)"

python3 -c '
import json
import sys

expected_id = sys.argv[1]
payload = json.load(sys.stdin)

assert any(workflow["id"] == expected_id for workflow in payload), payload
workflow = next(workflow for workflow in payload if workflow["id"] == expected_id)
assert workflow["name"] == "Dev Verify Workflow Renamed", workflow
' "$WORKFLOW_ID_TO_RUN" <<<"$WORKFLOW_LIST_RESPONSE"

echo "listing draft workflows"
DRAFT_WORKFLOW_LIST_RESPONSE="$(
  curl -fsS "http://${PLATFORM_HTTP_ADDR}/v1/workflows?tenant_id=${TENANT_ID}&project_id=00000000-0000-4000-8000-000000000004&status=draft"
)"

python3 -c '
import json
import sys

expected_id = sys.argv[1]
payload = json.load(sys.stdin)

assert len(payload) >= 1, payload
assert all(workflow["status"] == "draft" for workflow in payload), payload
assert any(workflow["id"] == expected_id for workflow in payload), payload
' "$WORKFLOW_ID_TO_RUN" <<<"$DRAFT_WORKFLOW_LIST_RESPONSE"

echo "creating workflow version"
CREATE_WORKFLOW_VERSION_RESPONSE="$(
  curl -fsS -X POST "http://${PLATFORM_HTTP_ADDR}/v1/workflows/${WORKFLOW_ID_TO_RUN}/versions" \
    -H 'Content-Type: application/json' \
    -d "{
      \"tenant_id\": \"${TENANT_ID}\",
      \"graph\": {
        \"workflow_version_id\": \"client-supplied-id\",
        \"nodes\": [
          {\"id\": \"trigger\", \"type\": \"trigger\"},
          {\"id\": \"agent\", \"type\": \"agent\"}
        ],
        \"edges\": [
          {\"source\": \"trigger\", \"target\": \"agent\"}
        ]
      }
    }"
)"

WORKFLOW_VERSION_ID_TO_RUN="$(
  python3 -c 'import json,sys; print(json.load(sys.stdin)["id"])' <<<"$CREATE_WORKFLOW_VERSION_RESPONSE"
)"

python3 -c '
import json
import sys

expected_workflow_id = sys.argv[1]
expected_workflow_version_id = sys.argv[2]
payload = json.load(sys.stdin)

assert payload["id"] == expected_workflow_version_id, payload
assert payload["tenant_id"] == "00000000-0000-4000-8000-000000000001", payload
assert payload["workflow_id"] == expected_workflow_id, payload
assert payload["version"] == 1, payload
assert payload["status"] == "draft", payload
assert payload["graph"]["workflow_version_id"] == expected_workflow_version_id, payload
' "$WORKFLOW_ID_TO_RUN" "$WORKFLOW_VERSION_ID_TO_RUN" <<<"$CREATE_WORKFLOW_VERSION_RESPONSE"

echo "listing workflow versions"
WORKFLOW_VERSION_LIST_RESPONSE="$(
  curl -fsS "http://${PLATFORM_HTTP_ADDR}/v1/workflows/${WORKFLOW_ID_TO_RUN}/versions?tenant_id=${TENANT_ID}"
)"

python3 -c '
import json
import sys

expected_id = sys.argv[1]
payload = json.load(sys.stdin)

assert len(payload) == 1, payload
assert payload[0]["id"] == expected_id, payload
assert payload[0]["status"] == "draft", payload
assert payload[0]["version"] == 1, payload
assert payload[0]["graph"]["workflow_version_id"] == expected_id, payload
' "$WORKFLOW_VERSION_ID_TO_RUN" <<<"$WORKFLOW_VERSION_LIST_RESPONSE"

echo "listing draft workflow versions"
DRAFT_WORKFLOW_VERSION_LIST_RESPONSE="$(
  curl -fsS "http://${PLATFORM_HTTP_ADDR}/v1/workflows/${WORKFLOW_ID_TO_RUN}/versions?tenant_id=${TENANT_ID}&status=draft"
)"

python3 -c '
import json
import sys

expected_id = sys.argv[1]
payload = json.load(sys.stdin)

assert len(payload) == 1, payload
assert payload[0]["id"] == expected_id, payload
assert payload[0]["status"] == "draft", payload
' "$WORKFLOW_VERSION_ID_TO_RUN" <<<"$DRAFT_WORKFLOW_VERSION_LIST_RESPONSE"

echo "publishing workflow version ${WORKFLOW_VERSION_ID_TO_RUN}"
PUBLISH_WORKFLOW_VERSION_RESPONSE="$(
  curl -fsS -X POST "http://${PLATFORM_HTTP_ADDR}/v1/workflow-versions/${WORKFLOW_VERSION_ID_TO_RUN}/publish" \
    -H 'Content-Type: application/json' \
    -d "{
      \"tenant_id\": \"${TENANT_ID}\"
    }"
)"

python3 -c '
import json
import sys

expected_id = sys.argv[1]
payload = json.load(sys.stdin)

assert payload["id"] == expected_id, payload
assert payload["status"] == "published", payload
' "$WORKFLOW_VERSION_ID_TO_RUN" <<<"$PUBLISH_WORKFLOW_VERSION_RESPONSE"

echo "checking published workflow"
PUBLISHED_WORKFLOW_LIST_RESPONSE="$(
  curl -fsS "http://${PLATFORM_HTTP_ADDR}/v1/workflows?tenant_id=${TENANT_ID}&project_id=00000000-0000-4000-8000-000000000004"
)"

python3 -c '
import json
import sys

expected_workflow_id = sys.argv[1]
expected_workflow_version_id = sys.argv[2]
payload = json.load(sys.stdin)

workflow = next(item for item in payload if item["id"] == expected_workflow_id)
assert workflow["status"] == "published", workflow
assert workflow["latest_version_id"] == expected_workflow_version_id, workflow
' "$WORKFLOW_ID_TO_RUN" "$WORKFLOW_VERSION_ID_TO_RUN" <<<"$PUBLISHED_WORKFLOW_LIST_RESPONSE"

echo "listing published workflow versions"
PUBLISHED_WORKFLOW_VERSION_LIST_RESPONSE="$(
  curl -fsS "http://${PLATFORM_HTTP_ADDR}/v1/workflows/${WORKFLOW_ID_TO_RUN}/versions?tenant_id=${TENANT_ID}&status=published"
)"

python3 -c '
import json
import sys

expected_id = sys.argv[1]
payload = json.load(sys.stdin)

assert len(payload) == 1, payload
assert payload[0]["id"] == expected_id, payload
assert payload[0]["status"] == "published", payload
' "$WORKFLOW_VERSION_ID_TO_RUN" <<<"$PUBLISHED_WORKFLOW_VERSION_LIST_RESPONSE"

echo "creating execution from latest published workflow ${WORKFLOW_ID_TO_RUN}"
CREATE_RESPONSE="$(
  curl -fsS -X POST "http://${PLATFORM_HTTP_ADDR}/v1/workflows/${WORKFLOW_ID_TO_RUN}/executions" \
    -H 'Content-Type: application/json' \
    -d "{
      \"tenant_id\": \"${TENANT_ID}\",
      \"input_json\": \"{\\\"lead_id\\\":\\\"lead-dev-verify\\\"}\"
    }"
)"

EXECUTION_ID="$(python3 -c 'import json,sys; print(json.load(sys.stdin)["id"])' <<<"$CREATE_RESPONSE")"

if [[ -z "$EXECUTION_ID" ]]; then
  echo "execution id missing from response: ${CREATE_RESPONSE}" >&2
  exit 1
fi

echo "querying execution ${EXECUTION_ID}"
GET_RESPONSE="$(
  curl -fsS "http://${PLATFORM_HTTP_ADDR}/v1/executions/${EXECUTION_ID}?tenant_id=${TENANT_ID}"
)"

python3 -c '
import json
import sys

expected_id = sys.argv[1]
payload = json.load(sys.stdin)

assert payload["id"] == expected_id, payload
assert payload["status"] == "succeeded", payload
assert payload["tenant_id"] == "00000000-0000-4000-8000-000000000001", payload
assert payload["workflow_id"] == sys.argv[3], payload
assert payload["workflow_version_id"] == sys.argv[2], payload
assert [node["node_id"] for node in payload["node_results"]] == ["trigger", "agent"], payload
assert [node["status"] for node in payload["node_results"]] == ["succeeded", "succeeded"], payload
' "$EXECUTION_ID" "$WORKFLOW_VERSION_ID_TO_RUN" "$WORKFLOW_ID_TO_RUN" <<<"$GET_RESPONSE"

echo "listing executions"
LIST_RESPONSE="$(
  curl -fsS "http://${PLATFORM_HTTP_ADDR}/v1/executions?tenant_id=${TENANT_ID}"
)"

python3 -c '
import json
import sys

expected_id = sys.argv[1]
payload = json.load(sys.stdin)

assert len(payload) >= 1, payload
assert payload[0]["id"] == expected_id, payload
assert payload[0]["tenant_id"] == "00000000-0000-4000-8000-000000000001", payload
assert payload[0]["status"] == "succeeded", payload
' "$EXECUTION_ID" <<<"$LIST_RESPONSE"

echo "archiving workflow ${WORKFLOW_ID_TO_RUN}"
ARCHIVE_WORKFLOW_RESPONSE="$(
  curl -fsS -X POST "http://${PLATFORM_HTTP_ADDR}/v1/workflows/${WORKFLOW_ID_TO_RUN}/archive" \
    -H 'Content-Type: application/json' \
    -d "{
      \"tenant_id\": \"${TENANT_ID}\"
    }"
)"

python3 -c '
import json
import sys

expected_id = sys.argv[1]
payload = json.load(sys.stdin)

assert payload["id"] == expected_id, payload
assert payload["status"] == "archived", payload
assert payload["latest_version_id"] == sys.argv[2], payload
' "$WORKFLOW_ID_TO_RUN" "$WORKFLOW_VERSION_ID_TO_RUN" <<<"$ARCHIVE_WORKFLOW_RESPONSE"

ARCHIVED_RUN_STATUS="$(
  curl -sS -o /dev/null -w "%{http_code}" -X POST "http://${PLATFORM_HTTP_ADDR}/v1/workflows/${WORKFLOW_ID_TO_RUN}/executions" \
    -H 'Content-Type: application/json' \
    -d "{
      \"tenant_id\": \"${TENANT_ID}\",
      \"input_json\": \"{\\\"lead_id\\\":\\\"lead-dev-verify-archived\\\"}\"
    }"
)"

if [[ "$ARCHIVED_RUN_STATUS" != "400" ]]; then
  echo "expected archived workflow run to return 400, got ${ARCHIVED_RUN_STATUS}" >&2
  exit 1
fi

ARCHIVED_VERSION_RUN_STATUS="$(
  curl -sS -o /dev/null -w "%{http_code}" -X POST "http://${PLATFORM_HTTP_ADDR}/v1/workflow-versions/${WORKFLOW_VERSION_ID_TO_RUN}/executions" \
    -H 'Content-Type: application/json' \
    -d "{
      \"tenant_id\": \"${TENANT_ID}\",
      \"input_json\": \"{\\\"lead_id\\\":\\\"lead-dev-verify-archived-version\\\"}\"
    }"
)"

if [[ "$ARCHIVED_VERSION_RUN_STATUS" != "400" ]]; then
  echo "expected archived workflow version run to return 400, got ${ARCHIVED_VERSION_RUN_STATUS}" >&2
  exit 1
fi

echo "restoring workflow ${WORKFLOW_ID_TO_RUN}"
RESTORE_WORKFLOW_RESPONSE="$(
  curl -fsS -X POST "http://${PLATFORM_HTTP_ADDR}/v1/workflows/${WORKFLOW_ID_TO_RUN}/restore" \
    -H 'Content-Type: application/json' \
    -d "{
      \"tenant_id\": \"${TENANT_ID}\"
    }"
)"

python3 -c '
import json
import sys

expected_id = sys.argv[1]
payload = json.load(sys.stdin)

assert payload["id"] == expected_id, payload
assert payload["status"] == "published", payload
assert payload["latest_version_id"] == sys.argv[2], payload
' "$WORKFLOW_ID_TO_RUN" "$WORKFLOW_VERSION_ID_TO_RUN" <<<"$RESTORE_WORKFLOW_RESPONSE"

echo "listing published workflows"
PUBLISHED_FILTER_RESPONSE="$(
  curl -fsS "http://${PLATFORM_HTTP_ADDR}/v1/workflows?tenant_id=${TENANT_ID}&project_id=00000000-0000-4000-8000-000000000004&status=published"
)"

python3 -c '
import json
import sys

expected_id = sys.argv[1]
payload = json.load(sys.stdin)

assert any(workflow["id"] == expected_id for workflow in payload), payload
workflow = next(workflow for workflow in payload if workflow["id"] == expected_id)
assert workflow["status"] == "published", workflow
' "$WORKFLOW_ID_TO_RUN" <<<"$PUBLISHED_FILTER_RESPONSE"

RESTORED_RUN_STATUS="$(
  curl -sS -o /dev/null -w "%{http_code}" -X POST "http://${PLATFORM_HTTP_ADDR}/v1/workflows/${WORKFLOW_ID_TO_RUN}/executions" \
    -H 'Content-Type: application/json' \
    -d "{
      \"tenant_id\": \"${TENANT_ID}\",
      \"input_json\": \"{\\\"lead_id\\\":\\\"lead-dev-verify-restored\\\"}\"
    }"
)"

if [[ "$RESTORED_RUN_STATUS" != "202" ]]; then
  echo "expected restored workflow run to return 202, got ${RESTORED_RUN_STATUS}" >&2
  exit 1
fi

echo "dev verification passed"
