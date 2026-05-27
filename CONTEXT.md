# AI Agent Workflow Platform

This context defines the core domain language for the AI Agent workflow low-code platform.

## Language

**Workflow**:
A versioned automation definition made of triggers, nodes, and edges. A Workflow describes what should happen, but does not itself represent a running instance.
_Avoid_: Flow, pipeline, automation script

**Workflow Version**:
An immutable snapshot of a Workflow graph that can be published, executed, rolled back, and audited.
_Avoid_: Draft copy, saved graph

**Execution**:
A single run of a Workflow Version with concrete input, state, logs, outputs, costs, and errors.
_Avoid_: Job, run, task

**Node**:
A step inside a Workflow. A Node has a type, configuration, input, output, and execution status.
_Avoid_: Block, component, step

**Edge**:
A directed connection between Nodes that defines control flow and data flow.
_Avoid_: Link, line, connection

**Trigger**:
The event source that starts an Execution, such as manual start, webhook, schedule, or message event.
_Avoid_: Starter, event hook

**Agent**:
A bounded AI decision unit used inside a Workflow. An Agent can use approved tools and knowledge, but must obey configured limits, schemas, and approval policy.
_Avoid_: Bot, assistant, autonomous worker

**Tool**:
A callable capability exposed to an Agent or Node, such as HTTP request, SQL query, OCR, or MCP call.
_Avoid_: Function, plugin

**Connector**:
An integration with an external system, such as Feishu, WeCom, DingTalk, Slack, Salesforce, or a database.
_Avoid_: Integration plugin

**Capability**:
A reusable package in the Capability Library, such as a Tool, Connector, Agent Template, Workflow Template, Prompt Template, Knowledge Pack, or MCP Server.
_Avoid_: Asset, extension

**Capability Library**:
The registry where reusable Capabilities are versioned, reviewed, published, and reused.
_Avoid_: Marketplace, plugin store

**Knowledge Base**:
A tenant-scoped collection of documents and chunks used for retrieval-augmented generation.
_Avoid_: Document store, vector store

**Credential**:
A protected secret or authorization record used by a Connector, Tool, Agent, or Workflow. Credential plaintext is never exposed to the frontend.
_Avoid_: API key, token, secret

**Tenant**:
An organization-level isolation unit for users, Workflows, Agents, Knowledge Bases, Credentials, Executions, and audit records.
_Avoid_: Account, customer

**Workspace**:
A collaboration space inside a Tenant that groups Projects and access control.
_Avoid_: Team space

**Project**:
A scoped collection of Workflows, Agents, Capabilities, Knowledge Bases, and Executions.
_Avoid_: App, folder

**Approval**:
A human decision point that gates a high-risk action or uncertain Agent output before the Workflow continues.
_Avoid_: Review, manual check

**Audit Log**:
An immutable record of user actions, Workflow changes, Agent decisions, tool calls, Credential use, and Execution events.
_Avoid_: Activity feed, debug log

**Model Gateway**:
The platform layer that routes model calls, enforces limits, records usage, and handles provider fallback.
_Avoid_: LLM wrapper

## Example Dialogue

Developer: "Should this sales automation be a Workflow or an Agent?"

Domain expert: "It should be a Workflow. The Agent is only one Node that researches the lead and returns structured output."

Developer: "Where do we store the CRM API key?"

Domain expert: "As a Credential scoped to the Tenant or Project. The Workflow can reference it, but the frontend never sees plaintext."

Developer: "Can the Agent update Salesforce directly?"

Domain expert: "Only through an approved Tool and usually behind an Approval when it writes production data."
