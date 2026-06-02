# AI Agent Workflow Low-Code Platform Design

## Product Positioning

The product is an enterprise AI Agent workflow low-code platform.

It lets teams visually compose AI Agents, workflow automation, connectors, knowledge bases, human approval, audit trails, and reusable capability packages.

Core principle:

```text
Deterministic workflow + bounded Agent decisions + human approval + audit replay
```

The platform should not start as a fully autonomous universal Agent. It should start as a controlled enterprise workflow system where Agent nodes provide local intelligence.

## Architecture Overview

Recommended layered architecture:

```text
Frontend
  React + TypeScript + React Flow + Monaco Editor

Platform Backend
  Rust

Execution Engine
  Rust

AI Runtime
  Python

Storage
  PostgreSQL + Redis + MinIO + pgvector / Qdrant

Protocols
  REST + gRPC + Redis Streams / NATS
```

High-level flow:

```text
[Web Console]
  Workflow canvas
  Agent builder
  Capability library
  Knowledge base
  Execution logs
  Access control
        |
        v
[Rust Platform Backend]
  Users, tenants, projects, workflow metadata, credentials, audit, billing
        |
        v
[Rust Execution Engine]
  DAG execution, scheduling, retries, timeout, cancellation, state machine
        |
        v
[Python AI Runtime]
  Agent, RAG, LangGraph, LLM calls, embedding, document parsing, eval
        |
        v
[Storage / Infra]
  PostgreSQL, Redis, MinIO, pgvector / Qdrant
```

## Layer Responsibilities

### Frontend

Use React, TypeScript, React Flow, and Monaco Editor.

Responsibilities:

- Workflow canvas
- Node configuration panels
- Agent configuration
- Prompt editing
- JSON Schema editing
- Knowledge base management
- Execution log viewer
- Approval UI
- Capability library UI
- Tenant and user administration

### Rust Platform Backend

Rust owns business platform concerns:

- Authentication and authorization
- Tenants and organizations
- Workspaces and projects
- Workflow definitions and versions
- Agent metadata
- Capability library metadata
- Connector metadata
- Credential metadata
- Audit logs
- Billing and usage
- REST API for frontend
- gRPC calls to Rust Executor and Python AI Runtime

Recommended Rust stack:

```text
HTTP: Axum
Async runtime: Tokio
RPC: tonic + protobuf
DB: PostgreSQL + sqlx
Auth: OIDC / OAuth2 / JWT
Permissions: casbin-rs
Config: config or figment
Logging: tracing
OpenAPI: utoipa
Observability: OpenTelemetry
```

### Rust Execution Engine

Rust owns high-performance execution and runtime reliability:

- DAG parsing
- DAG validation
- Node scheduling
- Concurrent execution
- Retry policy
- Timeout control
- Cancellation
- Webhook runtime
- Worker pool
- Event streaming
- Execution checkpointing
- Resource limits

Recommended Rust stack:

```text
Async runtime: Tokio
HTTP: Axum
gRPC: tonic
Serialization: serde + prost
Database: sqlx
Tracing: tracing + OpenTelemetry
Queue/Event: redis-rs, async-nats later
Sandbox: wasmtime / containerd / firecracker later
```

Rust should execute deterministic workflow nodes directly and call Python for AI nodes.

### Python AI Runtime

Python owns fast-changing AI capabilities:

- Agent execution
- LangGraph orchestration
- RAG
- LlamaIndex integration
- LLM provider SDKs
- Embedding
- Document parsing
- Prompt evaluation
- Tool calling
- Structured output
- Guardrails

Recommended Python stack:

```text
API: FastAPI
Agent: LangGraph
RAG: LlamaIndex or native implementation
Tasks: Celery or async workers for MVP
Models: LiteLLM or internal model gateway
Validation: Pydantic
```

## Core Modules

```text
auth
tenant
workspace
project
workflow
execution
agent
knowledge
capability
connector
credential
audit
billing
model-gateway
```

## Workflow Model

Core entities:

```text
Workflow
  -> WorkflowVersion
  -> Trigger
  -> Node
  -> Edge
  -> Execution
  -> NodeExecution
  -> Artifact
  -> AuditLog
```

Workflow lifecycle:

```text
Draft -> Validated -> Published -> Running -> Succeeded / Failed / WaitingApproval / Cancelled
```

Node types:

| Node | Purpose |
|---|---|
| Trigger | Manual, webhook, scheduled, message event |
| LLM Node | Single model call |
| Agent Node | Tool-using bounded Agent |
| RAG Node | Knowledge retrieval |
| HTTP Node | External API call |
| Code Node | Sandboxed JS/Python execution |
| Condition Node | Branching |
| Transform Node | JSON field mapping |
| Human Approval | Manual review |
| Subflow Node | Invoke another workflow |
| Notification | Email, Slack, Feishu, WeCom, DingTalk |
| Database Node | Database read/write |
| MCP Tool Node | Invoke MCP tools |

## Multi-Tenant Model

MVP storage model:

```text
Single database + shared tables + tenant_id isolation
```

Hierarchy:

```text
Tenant / Organization
  -> Workspace
  -> Project
  -> Workflow
  -> Agent
  -> Knowledge Base
  -> Credential
  -> Execution
```

Roles:

| Role | Permissions |
|---|---|
| Owner | Organization, billing, all permissions |
| Admin | Users, permissions, credentials, publishing |
| Developer | Agents, connectors, capability packages |
| Builder | Build and test workflows |
| Operator | Run, retry, approve |
| Viewer | View results and logs |
| Auditor | Audit and cost views |

Every core table should include:

```text
tenant_id
workspace_id
created_by
created_at
updated_at
```

Enterprise upgrade path:

```text
Dedicated database
Dedicated schema
Dedicated object bucket
Dedicated vector index
Dedicated execution queue
```

## Agent Model

Agents are bounded workflow nodes, not ungoverned autonomous processes.

Agent configuration:

```text
Agent
  - name
  - role
  - system_prompt
  - model
  - tools
  - knowledge_bases
  - memory_policy
  - max_steps
  - timeout
  - cost_limit
  - output_schema
  - guardrails
  - approval_policy
```

MVP Agent capability:

```text
Prompt + Tools + RAG + Structured Output + Human Approval
```

Later capability levels:

- Basic Agent: prompt, model, structured output
- Tool Agent: HTTP, database, MCP, subflow tools
- RAG Agent: knowledge retrieval with citations
- Memory Agent: short-term, task, and long-term memory
- Planning Agent: task decomposition and multi-step execution
- Collaborative Agent: supervisor, reviewer, worker patterns

## Agent Safety

Required guardrails:

- Tool allowlist
- Input and output schema validation
- Max execution steps
- Token and cost limits
- Timeout control
- Dry run
- Prompt injection detection
- Sensitive data redaction
- External URL restrictions
- File access sandbox
- Execution replay

High-risk actions require human approval by default:

- Sending email
- Writing production databases
- Updating CRM or ERP
- Deleting data
- Sending files externally
- Payment or ordering operations
- Accessing production credentials

## Storage Architecture

Primary storage:

```text
PostgreSQL: business state
Redis: cache, queue, lock, temporary execution state
MinIO: files, documents, artifacts
pgvector: MVP vector search
Qdrant/Milvus: later large-scale vector search
ClickHouse: later event and cost analytics
```

Key PostgreSQL tables:

```text
users
tenants
workspaces
projects
members
roles
permissions

workflows
workflow_versions
workflow_triggers
workflow_executions
node_executions

agents
agent_versions
agent_tools
tools
connectors

knowledge_bases
knowledge_documents
knowledge_chunks

credentials
audit_logs
billing_usage
capabilities
```

Workflow version record:

```text
workflow_versions
  id
  workflow_id
  version
  graph_json
  status
  created_by
  created_at
  published_at
```

Execution record:

```text
workflow_executions
  id
  tenant_id
  workflow_id
  version_id
  status
  input_json
  output_json
  started_at
  finished_at
  duration_ms
  cost

node_executions
  id
  execution_id
  node_id
  node_type
  status
  input_json
  output_json
  error_json
  token_usage
  started_at
  finished_at
```

## Credential Management

Credentials must never be exposed in plaintext to the frontend.

MVP:

```text
PostgreSQL stores metadata
AES-GCM stores encrypted secret payloads
CREDENTIAL_MASTER_KEY comes from environment
```

Enterprise:

```text
HashiCorp Vault
AWS KMS
Azure Key Vault
Aliyun KMS
Tencent Cloud KMS
```

## Capability Library

Capability types:

| Type | Examples |
|---|---|
| Connector | Feishu, WeCom, DingTalk, Slack, Salesforce |
| Tool | HTTP, SQL, OCR, PDF parsing, web scraping |
| Agent Template | Sales assistant, support assistant, contract reviewer |
| Workflow Template | Invoice processing, ticket routing, daily report |
| Prompt Template | Summary, classification, extraction, translation |
| Knowledge Pack | Industry pack, policy pack, product manual pack |
| MCP Server | External tool protocol service |

Capability manifest:

```yaml
name: sales-lead-enrichment
type: workflow-template
version: 1.0.0
author: internal
permissions:
  - http.request
  - crm.read
  - crm.write
inputs:
  lead_id:
    type: string
outputs:
  summary:
    type: string
  score:
    type: number
```

Required library features:

- Versioning
- Dependency declaration
- Permission declaration
- Security review
- Test cases
- Example inputs and outputs
- Private capability registry
- Public marketplace later

## CLI Scaffold

The platform should include a developer CLI:

```bash
velara init app
velara create node
velara create connector
velara create agent
velara create tool
velara create workflow
velara create mcp-server
velara dev
velara test
velara publish
```

Generated structure:

```text
my-velara-app/
  velara.yaml
  nodes/
  agents/
  workflows/
  connectors/
  tools/
  knowledge/
  tests/
  package.json
```

## Open Source Product References

| Product | Strength | Weakness | Lesson |
|---|---|---|---|
| Dify | LLM app, RAG, workflow | Limited complex enterprise process depth | Learn app builder and knowledge UX |
| n8n | Rich connectors, mature automation | AI Agent is not native core | Learn connector and workflow ecosystem |
| Flowise | Visual AI composition | Enterprise governance is weaker | Learn AI node canvas |
| Langflow | Python AI app builder | More developer tool than workflow platform | Learn component debugging |
| LangGraph | Strong Agent graph orchestration | Developer-first, not low-code | Use inside AI Runtime |
| CrewAI | Clear role/task/crew model | Governance still needs platform layer | Learn multi-Agent modeling |
| AutoGen | Early multi-Agent influence | Not recommended as new core dependency | Reference patterns only |
| Temporal | Reliable durable execution | Non-AI-native and complex | Consider later for reliability layer |

Differentiation:

```text
Dify: LLM app platform
n8n: automation platform
Flowise: AI visual builder
LangGraph: developer framework

This platform: enterprise Agent workflow operating system
```

## MVP Scope

Build first:

- Multi-tenant account system
- Workflow canvas
- Manual, webhook, and scheduled triggers
- LLM node
- Agent node
- HTTP node
- Condition node
- RAG knowledge base
- Human approval node
- Execution logs
- Credential management
- Basic capability library
- Basic CLI scaffold

Do not build first:

- Complex BPMN
- Full marketplace
- Fully autonomous multi-Agent systems
- Desktop RPA
- Model training platform
- Complex multi-region deployment

## Roadmap

### Phase 1: MVP

Goal: build, run, and debug AI workflows.

Target duration: 8-12 weeks.

### Phase 2: Enterprise Usability

- Fine-grained permissions
- Audit trails
- Version rollback
- Cost analytics
- Private deployment
- Model gateway
- Private capability library

### Phase 3: Platformization

- Plugin SDK
- MCP ecosystem
- Agent collaboration
- Template marketplace
- Eval system
- Large-scale execution

### Phase 4: Commercialization

- SaaS edition
- Private deployment edition
- Industry solution packages
- Capability marketplace
- Enterprise AI governance

## Final Recommendation

Use this target architecture:

```text
Frontend: React + TypeScript + React Flow + Monaco
Platform: Rust
Executor: Rust + Tokio + Axum + tonic
AI Runtime: Python + FastAPI + LangGraph + LlamaIndex
Storage: PostgreSQL + Redis + MinIO + pgvector
Future: Qdrant / Milvus + NATS / Kafka + ClickHouse + Temporal + Kubernetes
```

Operating principle:

```text
Rust manages platform business.
Rust manages execution performance.
Python manages AI capability.
PostgreSQL manages durable state.
Redis manages temporary state and queueing.
MinIO manages files and artifacts.
Capability library creates long-term platform leverage.
```
