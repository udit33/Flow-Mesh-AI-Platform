# Flow Mesh Feature Progress Report

Last updated: 2026-03-12

## 1) Executive Snapshot
- Overall state: **Phase 0 bootstrap partially complete**.
- UI status: **Professional workflow control-plane shell implemented** (builder canvas + ops tabs), static frontend only.
- Backend status: **API gateway + agent-runtime skeletons are running**.
- Core product capabilities (workflow engine, tools runtime, approvals API, model gateway, RAG): **not implemented yet**.

## 2) Planned vs Done (By Execution Plan Phase)

### Phase 0 — Foundation (planned)
Planned:
- Monorepo layout + service templates
- Local dev stack (compose) with Postgres/Redis/object storage mock
- Auth skeleton (tenant/user/workspace context propagation)
- CI baseline (fmt, lint, test)
- Architecture docs + ADR baseline

Done:
- Monorepo and service skeletons exist (`services/api-gateway`, `services/agent-runtime`, `crates/platform-common`).
- Auth/tenant context skeleton exists:
  - JWT decode + tenant context in shared crate.
  - Header fallback (`x-tenant-id`, `x-roles`) in gateway middleware.
- CI baseline exists (`.github/workflows/ci.yml`: fmt, clippy, test).
- Architecture/ADR/docs baseline exists (`ARCHITECTURE.md`, `docs/adr/*`, `PRD.md`, `API_CONTRACTS.md`).
- Initial DB schema migration exists (`db/migrations/0001_init_core.sql`).

Not done / partial:
- No `make dev` flow and no local compose stack committed yet.
- No Redis/object-storage mock runtime integration yet.

### Phase 1 — MVP Runtime (planned)
Planned:
- Agent runtime planner + tool invocation loop
- Tool registry v1 + execution runtime v1
- LLM gateway with 2 providers + fallback
- Chat completion API + web chat prototype
- Trace IDs + structured logs

Done:
- `POST /v1/runtime/complete` endpoint exists as a stub and returns a bootstrap response with trace_id.
- Structured logging setup exists in services (`tracing_subscriber`).

Not done:
- No planner/executor loop.
- No tool registry/execution APIs.
- No LLM gateway/provider integration.
- No chat channel/web chat behavior beyond static control-plane shell.

### Phase 2 — Workflow + HITL (planned)
Planned:
- Workflow DSL + DAG executor
- Retries/conditions/loops/checkpoints
- Human approval node + notification channel
- Workflow trigger support (API + schedule)

Done:
- Builder UI shell exists for workflow authoring experience (static data).

Not done:
- No workflow execution APIs implemented.
- No workflow run state machine/DAG executor.
- No approval decision API or notification workflow.

### Phase 3+ (Governance/RAG/Developer Platform/Hardening)
Done:
- Only foundational docs/schemas are present.

Not done:
- Policy engine implementation, audit service, RAG pipeline, SDKs, quotas, resilience hardening are not implemented.

## 3) Planned vs Done (By PRD Functional Requirement)

- FR-1 Experience Channels: **Partial**
  - Done: control-plane UI shell on `/`.
  - Missing: web chat UI, embeddable widget, most public APIs.

- FR-2 Multi-Agent Runtime: **Not started (functional)**
  - Done: runtime stub endpoint only.
  - Missing: routing, planner/executor, memory, retries/fallback policies.

- FR-3 Tooling Framework: **Not started (functional)**
  - Done: tools table in DB schema.
  - Missing: registry/invoke endpoints, validation, policy checks.

- FR-4 Workflow Engine: **Not started (functional)**
  - Done: static workflow builder UI shell; workflow tables in schema.
  - Missing: DSL parser, DAG runner, approval waits, checkpoint/resume.

- FR-5 LLM Gateway: **Not started**
  - Missing provider adapters, routing, fallback, guardrails, spend metering.

- FR-6 Knowledge/RAG: **Not started**
  - Missing ingestion, embeddings, retrieval, ACL filtering.

- FR-7 Security/Governance: **Partial foundation**
  - Done: tenant context model, basic role gate on admin endpoint, migration tables for policies/audit events.
  - Missing: SSO integration, policy engine runtime, full audit trail pipeline, DLP/PII controls.

- FR-8 Developer Platform: **Not started**
  - Missing SDK and extension lifecycle.

## 4) Implemented APIs (Current Runtime Behavior)

### API Gateway (`services/api-gateway`)
- `GET /` -> serves control-plane HTML UI.
- `GET /health` -> `{ "status": "ok", "service": "api-gateway" }`.
- `POST /v1/runtime/complete` -> authenticated stub response.
- `GET /v1/admin/tenants` -> requires tenant context and `tenant_admin` role; returns mock tenant item when DB is absent.

Auth behavior validated:
- Missing tenant/JWT -> `401`.
- Non-admin role on admin endpoint -> `403`.
- `tenant_admin` role -> `200`.

### Agent Runtime (`services/agent-runtime`)
- `GET /health` -> `{ "status": "ok", "service": "agent-runtime" }`.

### Contract gaps (currently 404)
- `POST /v1/workflows/{workflow_id}/run`
- `GET /v1/workflow-instances/{instance_id}`
- `POST /v1/tools`
- `POST /v1/tools/{tool_id}/invoke`
- `POST /v1/approvals/{approval_id}/decision`

## 5) Quality and Test Status
- CI checks configured: `cargo fmt`, `cargo clippy`, `cargo test`.
- Current test count: **0 tests** across crates/services.
- Functional confidence currently relies on manual smoke checks.

## 6) What Was Just Completed (UI Track)
- Replaced placeholder cards with a professional workflow workspace UI:
  - topbar with environment badges
  - left workflow catalog + node library
  - center workflow canvas with node visualization
  - right inspector/readiness/live queue
  - operational tabs for Runs/Approvals/Admin/Observability
- Added inline favicon to remove browser 404 noise.

## 7) Recommended Next Build Order
1. Implement `POST /v1/workflows/{id}/run` with request validation + workflow run record creation.
2. Implement `GET /v1/workflow-instances/{id}` to expose run lifecycle state.
3. Add approval decision API and run-state transitions.
4. Add integration tests for auth middleware and first workflow endpoints.
5. Start tool registry/invoke v1 and wire into runtime completion flow.
