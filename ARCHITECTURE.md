# Flow Mesh AI Platform — Architecture

## 1) Architecture Style
- **Control Plane + Runtime Plane** split.
- Stateless APIs where possible, state in managed data stores.
- Event-driven async execution for long-running tasks.
- Policy-first execution path (authz/policy/audit gates before sensitive actions).

## 2) Logical Planes

## Experience Layer
- Web App (Chat, Admin, Builder Studio)
- Embedded Widget SDK
- Public API Gateway
- Channel adapters (Slack/Teams/Email/Mobile)

## Control Plane
- Tenant/Org/Workspace service
- Identity + RBAC service
- Registry service (agents/tools/workflows/prompts)
- Policy management service
- Metering/Billing service
- Audit + analytics service

## Runtime Plane
- Agent Runtime service
- Workflow Orchestrator
- Tool Execution service
- Session/Memory service
- Event Bus + Worker pools
- Notification/Approval service

## Intelligence Layer
- LLM Gateway + provider adapters
- Prompt template service
- Planner service
- Guardrails service
- RAG Retrieval service
- Embedding service
- Evaluation service (offline/online)

## Integration Layer
- Connector framework (SaaS, DB, files, internal APIs)
- Secrets integration and credential broker
- Webhook ingestion
- Data ingestion jobs

## Data Layer
- Relational DB (OLTP metadata)
- Vector DB (embeddings/retrieval)
- Object storage (documents/artifacts/log bundles)
- Cache/session store (hot state)
- Search index (queryable logs/artifacts)
- Time-series/observability backend

## 3) Core Execution Flows

### A) Chat/Completion Flow
1. Request arrives with tenant/user context.
2. AuthN/AuthZ + policy pre-check.
3. Context assembler builds memory + retrieval context.
4. Planner chooses action/tool/workflow.
5. Tool/workflow execution with trace events.
6. Guardrails post-check.
7. Response returned with citations + trace id.

### B) Workflow Flow
1. Trigger fires (API/schedule/webhook/event).
2. Workflow instance created with checkpoint state.
3. DAG executor dispatches ready nodes.
4. Nodes run tools/agents; retries/compensation if needed.
5. HITL nodes pause and await approval.
6. Completion emits outcome + metrics + audit event.

## 4) Suggested Service Breakdown
- `api-gateway`
- `identity-service`
- `tenant-service`
- `agent-runtime-service`
- `workflow-service`
- `tool-registry-service`
- `tool-execution-service`
- `connection-service`
- `llm-gateway-service`
- `retrieval-service`
- `document-ingestion-service`
- `policy-service`
- `audit-service`
- `notification-service`
- `catalog-service`
- `billing-metering-service`

## 5) Security & Governance Controls
- Tenant-scoped resource IDs on all entities and mandatory tenant context propagation.
- Zero-trust service-to-service security (workload identity + authenticated/authorized calls).
- Mandatory policy checks for tool invocations and sensitive workflows.
- Signed audit trail for admin actions and execution events.
- Secret references only (no plaintext credentials in configs).
- Optional approval requirements by policy condition (risk-based).
- Regional deployment controls for tenant-level data residency.

## 6) Data Stores
- **PostgreSQL**: tenants, users, roles, agents, tools, workflows, runs, policies, connection metadata
- **Redis**: sessions, short-term memory, rate limits, job-state cache
- **Vector DB**: embeddings + retrieval metadata
- **Object Storage**: uploaded docs, workflow artifacts, execution payload snapshots
- **Search Index**: logs, catalog discovery, keyword search

## 7) High-Level Request Flow
1. User request arrives via web chat/API.
2. API Gateway authenticates and routes.
3. Agent Runtime loads tenant policy, user context, memory, and available tools.
4. Planner chooses: direct answer, tool call, workflow invocation, or sub-agent delegation.
5. Tool Execution runs connector actions.
6. LLM Gateway handles model calls and routing.
7. Retrieval Service fetches enterprise knowledge if required.
8. Runtime composes final response + execution trace.
9. Audit/metrics/logs emitted asynchronously.
10. Long-running jobs tracked by Workflow Service with notifications.

## 8) Scalability Strategy
- Stateless API replicas behind load balancers.
- Queue partitioning by tenant/workspace/priority.
- Worker autoscaling by queue depth and p95 latency.
- Retrieval/search read scaling with dedicated replicas.
- Backpressure and quotas enforced by policy/metering plane.

## 7) Reference Deployment Architecture
- Frontend: React/Next.js
- API Gateway: Kong / Apigee / AWS API Gateway / NGINX
- Core services: containerized microservices on Kubernetes
- Async messaging: Kafka or NATS
- Workflow engine: Temporal (preferred)
- Relational DB: PostgreSQL
- Vector DB: pgvector, Pinecone, or Weaviate
- Cache/session: Redis
- Object storage: S3-compatible storage
- Search: OpenSearch / Elasticsearch
- Secrets: HashiCorp Vault or cloud-native secrets manager
- Observability: OpenTelemetry + Prometheus + Grafana + Loki/Tempo

## 8) Open Questions
- Multi-region requirements and data residency per tenant?
- Strict consistency vs eventual consistency for analytics counters?
- Approval SLA/escalation model and ownership mapping?
