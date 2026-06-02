// Copyright © 2026 北京祺智科技有限公司. All rights reserved.
// Contact: managecode@gmail.com

use serde_json::{json, Value};

/// OpenAPI 3.0.3 specification for the Velara Platform API.
pub fn spec() -> Value {
    json!({
      "openapi": "3.0.3",
      "info": {
        "title": "Velara Platform API",
        "description": "REST API for the Velara low-code AI workflow platform. All endpoints (except /healthz, /metrics, /v1/auth/token, /v1/forms/*) require a Bearer JWT obtained from POST /v1/auth/token.",
        "version": env!("CARGO_PKG_VERSION"),
        "contact": { "name": "Velara", "url": "https://github.com/velara" }
      },
      "servers": [{ "url": "/" }],
      "security": [{ "bearerAuth": [] }],
      "components": {
        "securitySchemes": {
          "bearerAuth": { "type": "http", "scheme": "bearer", "bearerFormat": "JWT" }
        },
        "schemas": {
          "WorkflowRecord": {
            "type": "object",
            "properties": {
              "id": { "type": "string" },
              "tenant_id": { "type": "string" },
              "name": { "type": "string" },
              "status": { "type": "string", "enum": ["draft","published","archived"] },
              "description": { "type": "string", "nullable": true },
              "readme": { "type": "string", "nullable": true },
              "tags": { "type": "array", "items": { "type": "string" } },
              "pinned": { "type": "boolean" },
              "locked": { "type": "boolean" },
              "folder": { "type": "string", "nullable": true },
              "created_at": { "type": "integer" },
              "updated_at": { "type": "integer" }
            }
          },
          "WorkflowVersionRecord": {
            "type": "object",
            "properties": {
              "id": { "type": "string" },
              "workflow_id": { "type": "string" },
              "version": { "type": "integer" },
              "status": { "type": "string", "enum": ["draft","published"] },
              "graph": { "type": "object" },
              "message": { "type": "string", "nullable": true },
              "created_at": { "type": "integer" }
            }
          },
          "ExecutionRecord": {
            "type": "object",
            "properties": {
              "id": { "type": "string" },
              "workflow_id": { "type": "string" },
              "workflow_version_id": { "type": "string" },
              "status": { "type": "string", "enum": ["running","waiting_approval","succeeded","failed","cancelled"] },
              "input_json": { "type": "string" },
              "output_json": { "type": "string", "nullable": true },
              "started_at": { "type": "integer" },
              "finished_at": { "type": "integer", "nullable": true },
              "label": { "type": "string", "nullable": true },
              "trigger_type": { "type": "string", "nullable": true },
              "dry_run": { "type": "boolean" },
              "note": { "type": "string", "nullable": true },
              "node_results": { "type": "array", "items": { "$ref": "#/components/schemas/NodeExecutionRecord" } }
            }
          },
          "ExecutionSummary": {
            "type": "object",
            "properties": {
              "id": { "type": "string" },
              "workflow_id": { "type": "string" },
              "workflow_version_id": { "type": "string" },
              "status": { "type": "string" },
              "started_at": { "type": "integer" },
              "finished_at": { "type": "integer", "nullable": true },
              "label": { "type": "string", "nullable": true },
              "trigger_type": { "type": "string", "nullable": true },
              "dry_run": { "type": "boolean" }
            }
          },
          "NodeExecutionRecord": {
            "type": "object",
            "properties": {
              "node_id": { "type": "string" },
              "status": { "type": "string", "enum": ["succeeded","failed","skipped","running"] },
              "output_json": { "type": "string", "nullable": true },
              "error": { "type": "string", "nullable": true },
              "duration_ms": { "type": "integer" },
              "retry_count": { "type": "integer" }
            }
          },
          "CredentialRecord": {
            "type": "object",
            "properties": {
              "id": { "type": "string" },
              "tenant_id": { "type": "string" },
              "name": { "type": "string" },
              "value": { "type": "string" }
            }
          },
          "WebhookRecord": {
            "type": "object",
            "properties": {
              "token": { "type": "string" },
              "workflow_version_id": { "type": "string" },
              "url": { "type": "string" },
              "secret": { "type": "string", "nullable": true }
            }
          },
          "EventSubscription": {
            "type": "object",
            "properties": {
              "id": { "type": "string" },
              "tenant_id": { "type": "string" },
              "url": { "type": "string" },
              "events": { "type": "array", "items": { "type": "string" } },
              "description": { "type": "string", "nullable": true },
              "created_at": { "type": "integer" }
            }
          },
          "WorkflowComment": {
            "type": "object",
            "properties": {
              "id": { "type": "string" },
              "workflow_id": { "type": "string" },
              "author": { "type": "string" },
              "body": { "type": "string" },
              "created_at": { "type": "integer" },
              "updated_at": { "type": "integer" }
            }
          },
          "ApiError": {
            "type": "object",
            "properties": {
              "error": { "type": "string" },
              "message": { "type": "string" }
            }
          }
        }
      },
      "paths": {
        "/healthz": {
          "get": {
            "tags": ["System"],
            "summary": "Liveness check",
            "security": [],
            "responses": { "200": { "description": "ok" } }
          }
        },
        "/healthz/detail": {
          "get": {
            "tags": ["System"],
            "summary": "Detailed health (database + cache status)",
            "security": [],
            "responses": {
              "200": {
                "description": "ok",
                "content": { "application/json": { "schema": {
                  "type": "object",
                  "properties": {
                    "status": { "type": "string" },
                    "version": { "type": "string" },
                    "database": { "type": "boolean" },
                    "cache": { "type": "boolean" }
                  }
                }}}
              }
            }
          }
        },
        "/metrics": {
          "get": {
            "tags": ["System"],
            "summary": "Prometheus metrics",
            "security": [],
            "responses": { "200": { "description": "Prometheus text format" } }
          }
        },
        "/v1/system/info": {
          "get": {
            "tags": ["System"],
            "summary": "System information (version, node types, auth mode)",
            "responses": { "200": { "description": "System info object" } }
          }
        },
        "/v1/search": {
          "get": {
            "tags": ["System"],
            "summary": "Global search across workflows and executions",
            "parameters": [
              { "name": "q", "in": "query", "required": true, "schema": { "type": "string" } }
            ],
            "responses": { "200": { "description": "Search results" } }
          }
        },
        "/v1/auth/token": {
          "post": {
            "tags": ["Auth"],
            "summary": "Create a JWT token",
            "security": [],
            "requestBody": {
              "required": true,
              "content": { "application/json": { "schema": {
                "type": "object",
                "required": ["api_key"],
                "properties": {
                  "api_key": { "type": "string" },
                  "tenant_id": { "type": "string" },
                  "workspace_id": { "type": "string" },
                  "project_id": { "type": "string" },
                  "role": { "type": "string", "enum": ["viewer","editor","admin"] }
                }
              }}}
            },
            "responses": {
              "200": { "description": "JWT token response" },
              "401": { "description": "Invalid API key" }
            }
          }
        },
        "/v1/workflows": {
          "get": {
            "tags": ["Workflows"],
            "summary": "List workflows",
            "parameters": [
              { "name": "tag", "in": "query", "schema": { "type": "string" } },
              { "name": "folder", "in": "query", "schema": { "type": "string" } },
              { "name": "status", "in": "query", "schema": { "type": "string" } }
            ],
            "responses": { "200": { "description": "List of workflows", "content": { "application/json": { "schema": { "type": "array", "items": { "$ref": "#/components/schemas/WorkflowRecord" } } } } } }
          },
          "post": {
            "tags": ["Workflows"],
            "summary": "Create a workflow",
            "requestBody": {
              "required": true,
              "content": { "application/json": { "schema": {
                "type": "object",
                "required": ["name"],
                "properties": {
                  "name": { "type": "string" },
                  "description": { "type": "string" },
                  "tags": { "type": "array", "items": { "type": "string" } },
                  "folder": { "type": "string" }
                }
              }}}
            },
            "responses": { "200": { "description": "Created workflow" } }
          }
        },
        "/v1/workflows/{workflow_id}": {
          "get": {
            "tags": ["Workflows"],
            "summary": "Get a workflow",
            "parameters": [{ "name": "workflow_id", "in": "path", "required": true, "schema": { "type": "string" } }],
            "responses": { "200": { "description": "Workflow record" }, "404": { "description": "Not found" } }
          },
          "patch": {
            "tags": ["Workflows"],
            "summary": "Update workflow metadata (name, description, tags, readme)",
            "parameters": [{ "name": "workflow_id", "in": "path", "required": true, "schema": { "type": "string" } }],
            "requestBody": { "content": { "application/json": { "schema": { "type": "object" } } } },
            "responses": { "200": { "description": "Updated workflow" } }
          }
        },
        "/v1/workflows/{workflow_id}/versions": {
          "get": {
            "tags": ["Workflows"],
            "summary": "List versions for a workflow",
            "parameters": [{ "name": "workflow_id", "in": "path", "required": true, "schema": { "type": "string" } }],
            "responses": { "200": { "description": "List of versions" } }
          },
          "post": {
            "tags": ["Workflows"],
            "summary": "Save a new draft version",
            "parameters": [{ "name": "workflow_id", "in": "path", "required": true, "schema": { "type": "string" } }],
            "requestBody": { "required": true, "content": { "application/json": { "schema": { "type": "object", "required": ["graph"], "properties": { "graph": { "type": "object" }, "message": { "type": "string" } } } } } },
            "responses": { "200": { "description": "Created version" }, "400": { "description": "Cycle detected or locked workflow" } }
          }
        },
        "/v1/workflow-versions/{version_id}/publish": {
          "post": {
            "tags": ["Workflows"],
            "summary": "Publish a workflow version",
            "parameters": [{ "name": "version_id", "in": "path", "required": true, "schema": { "type": "string" } }],
            "responses": { "200": { "description": "Published version" } }
          }
        },
        "/v1/workflows/{workflow_id}/executions": {
          "post": {
            "tags": ["Executions"],
            "summary": "Start execution from latest published version",
            "parameters": [{ "name": "workflow_id", "in": "path", "required": true, "schema": { "type": "string" } }],
            "requestBody": { "content": { "application/json": { "schema": {
              "type": "object",
              "properties": {
                "input": { "type": "object" },
                "label": { "type": "string" },
                "dry_run": { "type": "boolean" },
                "env_set": { "type": "string" },
                "callback_url": { "type": "string" }
              }
            }}}},
            "responses": { "200": { "description": "Started execution" }, "404": { "description": "No published version" }, "429": { "description": "Concurrency limit reached" } }
          }
        },
        "/v1/workflow-versions/{version_id}/executions": {
          "post": {
            "tags": ["Executions"],
            "summary": "Start execution from a specific version",
            "parameters": [{ "name": "version_id", "in": "path", "required": true, "schema": { "type": "string" } }],
            "requestBody": { "content": { "application/json": { "schema": { "type": "object" } } } },
            "responses": { "200": { "description": "Started execution" }, "400": { "description": "Draft version rejected" } }
          }
        },
        "/v1/executions": {
          "get": {
            "tags": ["Executions"],
            "summary": "List executions",
            "parameters": [
              { "name": "workflow_id", "in": "query", "schema": { "type": "string" } },
              { "name": "status", "in": "query", "schema": { "type": "string" } },
              { "name": "label", "in": "query", "schema": { "type": "string" } },
              { "name": "search", "in": "query", "schema": { "type": "string" } },
              { "name": "limit", "in": "query", "schema": { "type": "integer", "default": 50 } },
              { "name": "offset", "in": "query", "schema": { "type": "integer", "default": 0 } }
            ],
            "responses": { "200": { "description": "List of execution summaries (X-Total-Count header present)" } }
          },
          "post": {
            "tags": ["Executions"],
            "summary": "Start an execution (by workflow_id in body)",
            "requestBody": { "required": true, "content": { "application/json": { "schema": { "type": "object", "required": ["workflow_id"], "properties": { "workflow_id": { "type": "string" } } } } } },
            "responses": { "200": { "description": "Started execution" } }
          }
        },
        "/v1/executions/batch": {
          "post": {
            "tags": ["Executions"],
            "summary": "Start multiple executions (max 20)",
            "requestBody": { "required": true, "content": { "application/json": { "schema": { "type": "object", "properties": { "requests": { "type": "array", "items": { "type": "object" } } } } } } },
            "responses": { "200": { "description": "Array of started executions" } }
          }
        },
        "/v1/executions/stats": {
          "get": {
            "tags": ["Executions"],
            "summary": "Aggregate execution statistics",
            "responses": { "200": { "description": "Stats object with total, running, succeeded, failed etc." } }
          }
        },
        "/v1/executions/stream": {
          "get": {
            "tags": ["Executions"],
            "summary": "SSE stream of execution list (pushes every 2s)",
            "responses": { "200": { "description": "text/event-stream" } }
          }
        },
        "/v1/executions/cancel-running": {
          "post": {
            "tags": ["Executions"],
            "summary": "Cancel all currently running executions for the tenant",
            "responses": { "200": { "description": "Number of cancelled executions" } }
          }
        },
        "/v1/executions/{execution_id}": {
          "get": {
            "tags": ["Executions"],
            "summary": "Get execution detail (includes node_results)",
            "parameters": [{ "name": "execution_id", "in": "path", "required": true, "schema": { "type": "string" } }],
            "responses": { "200": { "description": "Full execution record", "content": { "application/json": { "schema": { "$ref": "#/components/schemas/ExecutionRecord" } } } } }
          },
          "delete": {
            "tags": ["Executions"],
            "summary": "Delete a terminal execution",
            "parameters": [{ "name": "execution_id", "in": "path", "required": true, "schema": { "type": "string" } }],
            "responses": { "204": { "description": "Deleted" }, "400": { "description": "Execution still running" } }
          },
          "patch": {
            "tags": ["Executions"],
            "summary": "Update execution label",
            "parameters": [{ "name": "execution_id", "in": "path", "required": true, "schema": { "type": "string" } }],
            "requestBody": { "content": { "application/json": { "schema": { "type": "object", "properties": { "label": { "type": "string" } } } } } },
            "responses": { "200": { "description": "Updated" } }
          }
        },
        "/v1/executions/{execution_id}/cancel": {
          "post": {
            "tags": ["Executions"],
            "summary": "Cancel a running execution",
            "parameters": [{ "name": "execution_id", "in": "path", "required": true, "schema": { "type": "string" } }],
            "responses": { "200": { "description": "Cancelled" } }
          }
        },
        "/v1/executions/{execution_id}/retry": {
          "post": {
            "tags": ["Executions"],
            "summary": "Retry a failed/cancelled execution with same input",
            "parameters": [{ "name": "execution_id", "in": "path", "required": true, "schema": { "type": "string" } }],
            "responses": { "200": { "description": "New execution record" } }
          }
        },
        "/v1/executions/{execution_id}/approve": {
          "post": {
            "tags": ["Executions"],
            "summary": "Approve a waiting_approval execution",
            "parameters": [{ "name": "execution_id", "in": "path", "required": true, "schema": { "type": "string" } }],
            "requestBody": { "content": { "application/json": { "schema": { "type": "object", "properties": { "comment": { "type": "string" } } } } } },
            "responses": { "200": { "description": "Approved" } }
          }
        },
        "/v1/executions/{execution_id}/reject": {
          "post": {
            "tags": ["Executions"],
            "summary": "Reject a waiting_approval execution",
            "parameters": [{ "name": "execution_id", "in": "path", "required": true, "schema": { "type": "string" } }],
            "requestBody": { "content": { "application/json": { "schema": { "type": "object", "properties": { "comment": { "type": "string" } } } } } },
            "responses": { "200": { "description": "Rejected" } }
          }
        },
        "/v1/executions/{execution_id}/note": {
          "post": {
            "tags": ["Executions"],
            "summary": "Set or clear an annotation note on an execution",
            "parameters": [{ "name": "execution_id", "in": "path", "required": true, "schema": { "type": "string" } }],
            "requestBody": { "content": { "application/json": { "schema": { "type": "object", "properties": { "note": { "type": "string", "nullable": true } } } } } },
            "responses": { "200": { "description": "Updated" } }
          }
        },
        "/v1/executions/{execution_id}/events": {
          "get": {
            "tags": ["Executions"],
            "summary": "SSE stream of live node-execution updates for a single execution",
            "parameters": [{ "name": "execution_id", "in": "path", "required": true, "schema": { "type": "string" } }],
            "responses": { "200": { "description": "text/event-stream" } }
          }
        },
        "/v1/credentials": {
          "get": {
            "tags": ["Credentials"],
            "summary": "List credentials (values masked)",
            "responses": { "200": { "description": "List of credentials" } }
          },
          "post": {
            "tags": ["Credentials"],
            "summary": "Create a credential",
            "requestBody": { "required": true, "content": { "application/json": { "schema": { "type": "object", "required": ["name","value"], "properties": { "name": { "type": "string" }, "value": { "type": "string" } } } } } },
            "responses": { "200": { "description": "Created credential" } }
          }
        },
        "/v1/credentials/{credential_id}": {
          "delete": {
            "tags": ["Credentials"],
            "summary": "Delete a credential",
            "parameters": [{ "name": "credential_id", "in": "path", "required": true, "schema": { "type": "string" } }],
            "responses": { "204": { "description": "Deleted" } }
          }
        },
        "/v1/env-sets": {
          "get": {
            "tags": ["Environment"],
            "summary": "List environment variable set names",
            "responses": { "200": { "description": "List of env set summaries" } }
          },
          "delete": {
            "tags": ["Environment"],
            "summary": "Delete an environment variable set",
            "parameters": [{ "name": "name", "in": "query", "required": true, "schema": { "type": "string" } }],
            "responses": { "204": { "description": "Deleted" } }
          }
        },
        "/v1/env-vars": {
          "get": {
            "tags": ["Environment"],
            "summary": "List environment variables",
            "parameters": [{ "name": "set", "in": "query", "schema": { "type": "string" } }],
            "responses": { "200": { "description": "List of env vars" } }
          }
        },
        "/v1/env-vars/{key}": {
          "put": {
            "tags": ["Environment"],
            "summary": "Create or update an environment variable",
            "parameters": [
              { "name": "key", "in": "path", "required": true, "schema": { "type": "string" } },
              { "name": "set", "in": "query", "schema": { "type": "string" } }
            ],
            "requestBody": { "required": true, "content": { "application/json": { "schema": { "type": "object", "required": ["value"], "properties": { "value": { "type": "string" } } } } } },
            "responses": { "200": { "description": "Updated" } }
          },
          "delete": {
            "tags": ["Environment"],
            "summary": "Delete an environment variable",
            "parameters": [
              { "name": "key", "in": "path", "required": true, "schema": { "type": "string" } },
              { "name": "set", "in": "query", "schema": { "type": "string" } }
            ],
            "responses": { "204": { "description": "Deleted" } }
          }
        },
        "/v1/webhooks": {
          "get": {
            "tags": ["Webhooks"],
            "summary": "List all webhook registrations for the tenant",
            "responses": { "200": { "description": "List of webhook records" } }
          }
        },
        "/v1/webhooks/{token}": {
          "post": {
            "tags": ["Webhooks"],
            "summary": "Trigger a workflow via webhook",
            "security": [],
            "parameters": [{ "name": "token", "in": "path", "required": true, "schema": { "type": "string" } }],
            "responses": { "200": { "description": "Execution started" }, "401": { "description": "Invalid signature" }, "404": { "description": "Unknown token" } }
          },
          "delete": {
            "tags": ["Webhooks"],
            "summary": "Delete a webhook",
            "parameters": [{ "name": "token", "in": "path", "required": true, "schema": { "type": "string" } }],
            "responses": { "204": { "description": "Deleted" } }
          }
        },
        "/v1/event-subscriptions": {
          "get": {
            "tags": ["Event Subscriptions"],
            "summary": "List global event subscriptions (lifecycle webhooks)",
            "responses": { "200": { "description": "List of subscriptions" } }
          },
          "post": {
            "tags": ["Event Subscriptions"],
            "summary": "Create an event subscription",
            "requestBody": { "required": true, "content": { "application/json": { "schema": {
              "type": "object",
              "required": ["url"],
              "properties": {
                "url": { "type": "string" },
                "events": { "type": "array", "items": { "type": "string" }, "description": "Empty = subscribe to all 4 event types" },
                "description": { "type": "string" }
              }
            }}}},
            "responses": { "200": { "description": "Created subscription" } }
          }
        },
        "/v1/event-subscriptions/{sub_id}": {
          "delete": {
            "tags": ["Event Subscriptions"],
            "summary": "Delete an event subscription",
            "parameters": [{ "name": "sub_id", "in": "path", "required": true, "schema": { "type": "string" } }],
            "responses": { "204": { "description": "Deleted" } }
          }
        },
        "/v1/audit-log": {
          "get": {
            "tags": ["Audit"],
            "summary": "List audit log entries",
            "parameters": [
              { "name": "action", "in": "query", "schema": { "type": "string" } },
              { "name": "resource_id", "in": "query", "schema": { "type": "string" } },
              { "name": "limit", "in": "query", "schema": { "type": "integer", "default": 100 } }
            ],
            "responses": { "200": { "description": "List of audit events" } }
          }
        },
        "/v1/token-usage": {
          "get": {
            "tags": ["Analytics"],
            "summary": "AI token usage summary grouped by model",
            "responses": { "200": { "description": "Token usage summary" } }
          }
        },
        "/v1/analytics/node-types": {
          "get": {
            "tags": ["Analytics"],
            "summary": "Execution stats grouped by node type",
            "responses": { "200": { "description": "Per-node-type stats" } }
          }
        },
        "/v1/api-keys": {
          "get": {
            "tags": ["API Keys"],
            "summary": "List API keys (values not shown after creation)",
            "responses": { "200": { "description": "List of API key records" } }
          },
          "post": {
            "tags": ["API Keys"],
            "summary": "Create an API key (value shown once)",
            "requestBody": { "content": { "application/json": { "schema": { "type": "object", "properties": { "name": { "type": "string" } } } } } },
            "responses": { "200": { "description": "API key with plaintext value" } }
          }
        },
        "/v1/api-keys/{key_id}": {
          "delete": {
            "tags": ["API Keys"],
            "summary": "Revoke an API key",
            "parameters": [{ "name": "key_id", "in": "path", "required": true, "schema": { "type": "string" } }],
            "responses": { "204": { "description": "Revoked" } }
          }
        },
        "/v1/workspaces": {
          "get": {
            "tags": ["Workspaces"],
            "summary": "List workspaces",
            "responses": { "200": { "description": "List of workspace records" } }
          },
          "post": {
            "tags": ["Workspaces"],
            "summary": "Create a workspace",
            "requestBody": { "content": { "application/json": { "schema": { "type": "object", "required": ["name"], "properties": { "name": { "type": "string" } } } } } },
            "responses": { "200": { "description": "Created workspace" } }
          }
        },
        "/v1/workspaces/{workspace_id}": {
          "delete": {
            "tags": ["Workspaces"],
            "summary": "Delete a workspace",
            "parameters": [{ "name": "workspace_id", "in": "path", "required": true, "schema": { "type": "string" } }],
            "responses": { "204": { "description": "Deleted" } }
          }
        },
        "/v1/workspaces/{workspace_id}/projects": {
          "get": {
            "tags": ["Workspaces"],
            "summary": "List projects in a workspace",
            "parameters": [{ "name": "workspace_id", "in": "path", "required": true, "schema": { "type": "string" } }],
            "responses": { "200": { "description": "List of project records" } }
          },
          "post": {
            "tags": ["Workspaces"],
            "summary": "Create a project in a workspace",
            "parameters": [{ "name": "workspace_id", "in": "path", "required": true, "schema": { "type": "string" } }],
            "requestBody": { "content": { "application/json": { "schema": { "type": "object", "required": ["name"], "properties": { "name": { "type": "string" } } } } } },
            "responses": { "200": { "description": "Created project" } }
          }
        },
        "/v1/cron/preview": {
          "post": {
            "tags": ["Scheduling"],
            "summary": "Preview next N fire times for a cron expression",
            "requestBody": { "required": true, "content": { "application/json": { "schema": { "type": "object", "required": ["expression"], "properties": { "expression": { "type": "string" }, "count": { "type": "integer", "default": 3 } } } } } },
            "responses": { "200": { "description": "{ next_times: string[], error?: string }" } }
          }
        },
        "/v1/schedules": {
          "get": {
            "tags": ["Scheduling"],
            "summary": "List active schedules for the tenant",
            "responses": { "200": { "description": "List of schedule summaries" } }
          }
        },
        "/v1/schedules/{version_id}/pause": {
          "post": {
            "tags": ["Scheduling"],
            "summary": "Pause a scheduled workflow",
            "parameters": [{ "name": "version_id", "in": "path", "required": true, "schema": { "type": "string" } }],
            "responses": { "200": { "description": "Paused" } }
          }
        },
        "/v1/schedules/{version_id}/resume": {
          "post": {
            "tags": ["Scheduling"],
            "summary": "Resume a paused scheduled workflow",
            "parameters": [{ "name": "version_id", "in": "path", "required": true, "schema": { "type": "string" } }],
            "responses": { "200": { "description": "Resumed" } }
          }
        },
        "/v1/workflows/{workflow_id}/comments": {
          "get": {
            "tags": ["Comments"],
            "summary": "List comments on a workflow",
            "parameters": [{ "name": "workflow_id", "in": "path", "required": true, "schema": { "type": "string" } }],
            "responses": { "200": { "description": "List of comments" } }
          },
          "post": {
            "tags": ["Comments"],
            "summary": "Add a comment to a workflow",
            "parameters": [{ "name": "workflow_id", "in": "path", "required": true, "schema": { "type": "string" } }],
            "requestBody": { "required": true, "content": { "application/json": { "schema": { "type": "object", "required": ["body"], "properties": { "body": { "type": "string" }, "author": { "type": "string" } } } } } },
            "responses": { "200": { "description": "Created comment" } }
          }
        },
        "/v1/comments/{comment_id}": {
          "patch": {
            "tags": ["Comments"],
            "summary": "Edit a comment",
            "parameters": [{ "name": "comment_id", "in": "path", "required": true, "schema": { "type": "string" } }],
            "requestBody": { "content": { "application/json": { "schema": { "type": "object", "properties": { "body": { "type": "string" } } } } } },
            "responses": { "200": { "description": "Updated" } }
          },
          "delete": {
            "tags": ["Comments"],
            "summary": "Delete a comment",
            "parameters": [{ "name": "comment_id", "in": "path", "required": true, "schema": { "type": "string" } }],
            "responses": { "204": { "description": "Deleted" } }
          }
        },
        "/v1/workflows/{workflow_id}/test-cases": {
          "get": {
            "tags": ["Test Cases"],
            "summary": "List test cases for a workflow",
            "parameters": [{ "name": "workflow_id", "in": "path", "required": true, "schema": { "type": "string" } }],
            "responses": { "200": { "description": "List of test cases" } }
          },
          "post": {
            "tags": ["Test Cases"],
            "summary": "Create a test case",
            "parameters": [{ "name": "workflow_id", "in": "path", "required": true, "schema": { "type": "string" } }],
            "requestBody": { "required": true, "content": { "application/json": { "schema": { "type": "object", "required": ["name","input_json"], "properties": { "name": { "type": "string" }, "input_json": { "type": "string" }, "expected_output_json": { "type": "string" } } } } } },
            "responses": { "200": { "description": "Created test case" } }
          }
        },
        "/v1/test-cases/{test_case_id}/run": {
          "post": {
            "tags": ["Test Cases"],
            "summary": "Run a test case against the published workflow version",
            "parameters": [{ "name": "test_case_id", "in": "path", "required": true, "schema": { "type": "string" } }],
            "responses": { "200": { "description": "Test result (passed/failed + actual output)" } }
          }
        },
        "/v1/workflows/{workflow_id}/publish-form": {
          "post": {
            "tags": ["Forms"],
            "summary": "Publish a public submission form for a workflow",
            "parameters": [{ "name": "workflow_id", "in": "path", "required": true, "schema": { "type": "string" } }],
            "requestBody": { "content": { "application/json": { "schema": { "type": "object", "properties": { "title": { "type": "string" }, "description": { "type": "string" } } } } } },
            "responses": { "200": { "description": "Form token and URL" } }
          }
        },
        "/v1/forms/{token}": {
          "get": {
            "tags": ["Forms"],
            "summary": "Get form metadata (public, no auth)",
            "security": [],
            "parameters": [{ "name": "token", "in": "path", "required": true, "schema": { "type": "string" } }],
            "responses": { "200": { "description": "Form metadata" }, "404": { "description": "Not found" } }
          },
          "delete": {
            "tags": ["Forms"],
            "summary": "Delete a form token",
            "parameters": [{ "name": "token", "in": "path", "required": true, "schema": { "type": "string" } }],
            "responses": { "204": { "description": "Deleted" } }
          }
        },
        "/v1/forms/{token}/submit": {
          "post": {
            "tags": ["Forms"],
            "summary": "Submit a form (public, no auth) — starts a workflow execution",
            "security": [],
            "parameters": [{ "name": "token", "in": "path", "required": true, "schema": { "type": "string" } }],
            "requestBody": { "content": { "application/json": { "schema": { "type": "object" } } } },
            "responses": { "200": { "description": "Execution started" } }
          }
        },
        "/v1/workflows/{workflow_id}/export": {
          "get": {
            "tags": ["Workflows"],
            "summary": "Export workflow as JSON (includes metadata + latest version graph)",
            "parameters": [{ "name": "workflow_id", "in": "path", "required": true, "schema": { "type": "string" } }],
            "responses": { "200": { "description": "Workflow export JSON" } }
          }
        },
        "/v1/workflows/import": {
          "post": {
            "tags": ["Workflows"],
            "summary": "Import a workflow from exported JSON",
            "requestBody": { "required": true, "content": { "application/json": { "schema": { "type": "object", "required": ["graph"], "properties": { "name": { "type": "string" }, "graph": { "type": "object" } } } } } },
            "responses": { "200": { "description": "Imported workflow record" } }
          }
        },
        "/v1/workflows/generate": {
          "post": {
            "tags": ["Workflows"],
            "summary": "AI-assisted workflow generation using Claude",
            "requestBody": { "required": true, "content": { "application/json": { "schema": { "type": "object", "required": ["prompt"], "properties": { "prompt": { "type": "string" }, "api_key": { "type": "string" }, "model": { "type": "string" }, "create": { "type": "boolean" } } } } } },
            "responses": { "200": { "description": "Generated workflow graph (optionally created)" } }
          }
        },
        "/v1/workflows/{workflow_id}/stats": {
          "get": {
            "tags": ["Workflows"],
            "summary": "Execution statistics for a single workflow",
            "parameters": [{ "name": "workflow_id", "in": "path", "required": true, "schema": { "type": "string" } }],
            "responses": { "200": { "description": "{ total, succeeded, failed, running, avg_duration_secs }" } }
          }
        },
        "/v1/workflows/{workflow_id}/variables": {
          "get": {
            "tags": ["Variables"],
            "summary": "List persistent variables for a workflow",
            "parameters": [{ "name": "workflow_id", "in": "path", "required": true, "schema": { "type": "string" } }],
            "responses": { "200": { "description": "List of variables" } }
          }
        },
        "/v1/workflows/{workflow_id}/variables/{key}": {
          "get": {
            "tags": ["Variables"],
            "summary": "Get a workflow variable",
            "parameters": [
              { "name": "workflow_id", "in": "path", "required": true, "schema": { "type": "string" } },
              { "name": "key", "in": "path", "required": true, "schema": { "type": "string" } }
            ],
            "responses": { "200": { "description": "Variable value" } }
          },
          "put": {
            "tags": ["Variables"],
            "summary": "Set a workflow variable",
            "parameters": [
              { "name": "workflow_id", "in": "path", "required": true, "schema": { "type": "string" } },
              { "name": "key", "in": "path", "required": true, "schema": { "type": "string" } }
            ],
            "requestBody": { "required": true, "content": { "application/json": { "schema": { "type": "object", "properties": { "value": {} } } } } },
            "responses": { "200": { "description": "Set" } }
          },
          "delete": {
            "tags": ["Variables"],
            "summary": "Delete a workflow variable",
            "parameters": [
              { "name": "workflow_id", "in": "path", "required": true, "schema": { "type": "string" } },
              { "name": "key", "in": "path", "required": true, "schema": { "type": "string" } }
            ],
            "responses": { "204": { "description": "Deleted" } }
          }
        },
        "/v1/workflows/{workflow_id}/variables/{key}/increment": {
          "post": {
            "tags": ["Variables"],
            "summary": "Atomically increment a numeric workflow variable",
            "parameters": [
              { "name": "workflow_id", "in": "path", "required": true, "schema": { "type": "string" } },
              { "name": "key", "in": "path", "required": true, "schema": { "type": "string" } }
            ],
            "requestBody": { "content": { "application/json": { "schema": { "type": "object", "properties": { "delta": { "type": "number", "default": 1 } } } } } },
            "responses": { "200": { "description": "New value" } }
          }
        }
      }
    })
}

/// Swagger UI HTML page pointing at /openapi.json.
pub fn swagger_ui_html() -> String {
    r##"<!DOCTYPE html>
<html lang="en">
<head>
  <meta charset="UTF-8" />
  <meta name="viewport" content="width=device-width, initial-scale=1" />
  <title>Velara API Docs</title>
  <link rel="stylesheet" href="https://unpkg.com/swagger-ui-dist@5/swagger-ui.css" />
  <style>body{margin:0}.swagger-ui .topbar{background:#1a1a2e}.swagger-ui .topbar a{visibility:hidden}.swagger-ui .topbar::before{content:"Velara Platform API";color:#fff;font-size:18px;font-weight:600;padding:0 20px;line-height:50px}</style>
</head>
<body>
  <div id="swagger-ui"></div>
  <script src="https://unpkg.com/swagger-ui-dist@5/swagger-ui-bundle.js"></script>
  <script>
    SwaggerUIBundle({
      url: "/openapi.json",
      dom_id: "#swagger-ui",
      presets: [SwaggerUIBundle.presets.apis, SwaggerUIBundle.SwaggerUIStandalonePreset],
      layout: "BaseLayout",
      deepLinking: true,
      persistAuthorization: true,
    });
  </script>
</body>
</html>"##.to_string()
}
