# Trigix

**AI 原生工作流自动化平台**

[![License: MIT](https://img.shields.io/badge/License-MIT-blue.svg)](LICENSE)
[![Rust](https://img.shields.io/badge/Rust-2021-orange.svg)](https://www.rust-lang.org/)
[![React](https://img.shields.io/badge/React-18-61dafb.svg)](https://react.dev/)
[![GitHub Stars](https://img.shields.io/github/stars/bj-qizhi/trigix?style=social)](https://github.com/bj-qizhi/trigix)
[![GitHub Issues](https://img.shields.io/github/issues/bj-qizhi/trigix)](https://github.com/bj-qizhi/trigix/issues)
[![GitHub Forks](https://img.shields.io/github/forks/bj-qizhi/trigix?style=social)](https://github.com/bj-qizhi/trigix/fork)

[English](README.md) · [中文]

> © 2026 [北京祺智科技有限公司](https://www.qzso.com/) · managecode@gmail.com

---

## 项目简介

Trigix 是一个企业级 AI 原生工作流自动化平台。
通过可视化拖拽画布，连接 AI 大模型、API、数据库和 SaaS 工具，构建、运行和监控复杂的业务工作流。

**核心优势：**
- **136 种节点类型** — AI 大模型（Claude、GPT-4、Gemini、Groq、Mistral、国内大模型…）、SaaS 集成（Slack、Jira、Notion、Salesforce…）、数据处理、流程控制
- **Rust 执行引擎** — DAG 调度、并行分支、重试、超时、取消，吞吐量 5-10x Node.js
- **AI 原生** — 8 个内置大模型节点，支持 pgvector，MCP 协议原生集成
- **企业级** — JWT + RBAC、多租户、审计日志、Webhook 签名验证、Kubernetes Helm Chart

---

## 功能特性

| 类别 | 功能亮点 |
|------|---------|
| **画布编辑器** | 拖拽式、React Flow、小地图、网格吸附、撤销/重做、键盘快捷键 |
| **执行引擎** | 异步 DAG、并行分支（Fan-out/Fan-in）、子工作流、ForEach、循环节点 |
| **AI 节点** | Claude、OpenAI、Gemini、Groq、Mistral、Cohere、Replicate、Perplexity + Deepseek、通义千问、智谱、月之暗面等 7 个国内模型 |
| **集成节点** | 100+ 节点：GitHub、Jira、Notion、Slack、Stripe、Salesforce、Airtable、Linear… |
| **数据处理** | Filter、Map、Aggregate、Sort、Merge、Extract、Dedupe、Regex、CSV、XML、YAML… |
| **触发器** | Webhook（HMAC-SHA256 签名）、Cron 表达式、间隔触发、手动触发、表单提交 |
| **认证与安全** | JWT、RBAC（Viewer/Editor/Admin）、API Key 管理、bcrypt 密码、邮箱验证 |
| **可观测性** | 审计日志、执行时间线、Prometheus 指标、OpenTelemetry 链路追踪 |
| **基础设施** | PostgreSQL、Redis Streams、Docker、Kubernetes Helm Chart |

---

## 快速开始

```bash
# 1. 启动本地基础设施
docker compose up -d

# 2. 启动平台后端
DATABASE_URL=postgres://trigix:trigix@localhost:35432/trigix \
PLATFORM_HTTP_ADDR=127.0.0.1:38080 \
cargo run -p trigix-platform

# 3. 启动执行引擎
EXECUTOR_HTTP_ADDR=127.0.0.1:38090 \
cargo run -p trigix-executor

# 4. 启动 Web 控制台
cd apps/web && npm install && npm run dev
# 浏览器打开 http://localhost:3100
```

开发环境默认 API Key：`dev`

---

## 部署

**Kubernetes（Helm chart，已发布到 GHCR）：**

```bash
helm install trigix oci://ghcr.io/bj-qizhi/charts/trigix --version 0.3.1 \
  --namespace trigix --create-namespace
```

部署平台、AI Runtime、PostgreSQL/pgvector 和 Redis。配置见 `charts/trigix/values.yaml`。
每个 [`chart-v*` release](https://github.com/bj-qizhi/trigix/releases) 也附带 chart `.tgz`。

**Docker Compose（单机）：**

```bash
docker compose -f docker-compose.prod.yml up -d --build
```

---

## 目录结构

```text
apps/web                 React Web 控制台（Vite + React Flow）
services/platform-rs     Rust 平台 API（Axum、JWT、多租户）
services/executor        Rust 执行引擎（DAG、并行、重试）
services/ai-runtime      Python AI 运行时（FastAPI、LangChain）
crates/workflow-core     工作流图模型 + DAG 校验（共享 crate）
crates/execution-core    执行状态类型（共享 crate）
infra/postgres           54 个数据库迁移文件
charts/trigix        Kubernetes Helm Chart
docs/                    架构文档、ADR、开发指南
```

---

## 文档导航

| 文档 | 说明 |
|------|------|
| [系统架构设计](docs/architecture/ai-agent-workflow-platform-design.md) | 整体架构、服务边界、领域模型 |
| [工作流图 JSON 规范](docs/architecture/workflow-graph-json.md) | 节点/边的 Schema 参考 |
| [本地开发启动指南](docs/dev/bootstrap.md) | PostgreSQL 模式的本地环境搭建 |
| [端口说明](docs/dev/ports.md) | 所有本地服务端口 |
| [ADR-0001：分层架构决策](docs/adr/0001-layered-platform-architecture.md) | 架构决策记录 |

---

## 环境变量

| 变量 | 服务 | 说明 |
|------|------|------|
| `DATABASE_URL` | Platform | PostgreSQL 连接串（默认：内存存储） |
| `EXECUTOR_BASE_URL` | Platform | 独立执行器地址（默认：内嵌模式） |
| `AI_RUNTIME_BASE_URL` | Executor | Python AI 运行时地址 |
| `ANTHROPIC_API_KEY` | AI Runtime | Claude API Key |
| `AUTH_REQUIRED` | Platform | 全局启用 JWT 认证（`true`/`false`） |
| `DEV_API_KEY` | Platform | 开发环境 API Key（默认：`dev`） |

---

## 开源协议

MIT License — 详见 [LICENSE](LICENSE)

Copyright © 2026 北京祺智科技有限公司 · https://www.qzso.com/
