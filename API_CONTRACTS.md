# Flow Mesh AI Platform — API Contracts (Draft v1)

## 1) Runtime API
### POST /v1/runtime/complete
Headers:
- `Authorization: Bearer <jwt>` (preferred; when JWT is configured)
- `x-tenant-id: <uuid>` (bootstrap/dev fallback)
- `x-roles: tenant_admin,builder,...` (bootstrap/dev fallback)

Request:
```json
{
  "workspace_id": "w1",
  "user_id": "u1",
  "session_id": "s1",
  "input": "Summarize escalations and open Jira tickets",
  "mode": "sync",
  "context": {"channel": "web"}
}
```
Response:
```json
{
  "trace_id": "tr_123",
  "output": "...",
  "selected_agent": "supervisor",
  "delegated_to": "workflow-specialist",
  "steps": []
}
```

Notes:
- API gateway forwards runtime requests to `AGENT_RUNTIME_URL/v1/agent/execute`.
- Returns `502` when agent-runtime is unavailable.

### POST /v1/agent/execute (agent-runtime service)
Request:
```json
{
  "tenant_id": "<uuid>",
  "user_id": "<uuid>",
  "input": "run workflow for approval",
  "preferred_agent": null
}
```
Response:
```json
{
  "trace_id": "tr_xxx",
  "selected_agent": "supervisor",
  "delegated_to": "workflow-specialist",
  "steps": [
    {"step":1,"actor":"supervisor","action":"intent_classification","result":"..."}
  ],
  "output": "Request processed by supervisor..."
}
```

## 2) Workflow API
### POST /v1/workflows/{workflow_id}/run
Headers:
- `Authorization: Bearer <jwt>` (preferred) OR bootstrap headers

Request:
```json
{
  "trigger": "api",
  "inputs": {"ticket_id": "INC-1021"}
}
```
Response (202 Accepted):
```json
{
  "instance_id": "wf_run_001",
  "status": "running",
  "trace_id": "tr_456",
  "trigger": "api",
  "inputs": {"ticket_id": "INC-1021"}
}
```

### GET /v1/workflow-instances/{instance_id}
Response:
```json
{
  "instance_id": "wf_run_001",
  "tenant_id": "...",
  "workflow_id": "...",
  "status": "running",
  "current_node": "start",
  "trace_id": "tr_456",
  "started_at": "2026-03-11T00:00:00Z",
  "completed_at": null
}
```

## 3) Tool Registry API
### POST /v1/tools
Registers tool metadata + schema.

Headers:
- `Authorization: Bearer <jwt>` (preferred) OR bootstrap headers
- Role required: `builder` or `tenant_admin`

Request:
```json
{
  "name": "jira_search",
  "kind": "http",
  "version": "1.0.0",
  "schema_json": {
    "type": "object",
    "properties": {
      "query": { "type": "string" }
    },
    "required": ["query"]
  }
}
```

Response (201 Created):
```json
{
  "id": "<tool_uuid>",
  "tenant_id": "<tenant_uuid>",
  "workspace_id": null,
  "name": "jira_search",
  "kind": "http",
  "version": "1.0.0",
  "schema_json": {"type":"object","properties":{"query":{"type":"string"}},"required":["query"]},
  "created_at": "2026-03-12T12:00:00Z"
}
```

### GET /v1/tools
Lists tools for current tenant.

Headers:
- `Authorization: Bearer <jwt>` (preferred) OR bootstrap headers

Response:
```json
{
  "items": [
    {
      "id": "<tool_uuid>",
      "tenant_id": "<tenant_uuid>",
      "workspace_id": null,
      "name": "jira_search",
      "kind": "http",
      "version": "1.0.0",
      "schema_json": {"type":"object"},
      "created_at": "2026-03-12T12:00:00Z"
    }
  ]
}
```

### POST /v1/tools/{tool_id}/invoke
Invokes a registered tool with tenant/role checks.

Headers:
- `Authorization: Bearer <jwt>` (preferred) OR bootstrap headers
- User invoke requires `user` role plus scoped permission: one of
  - `tool:invoke:*`
  - `tool:invoke:{tool_id}`
  - `tool:invoke:{scope}`
- `builder`/`tenant_admin` can invoke without extra scope role

Request:
```json
{
  "scope": "support",
  "input": {
    "query": "project = MESH ORDER BY updated DESC"
  }
}
```

Response:
```json
{
  "trace_id": "tr_...",
  "tool_id": "<tool_uuid>",
  "result": {
    "ok": true,
    "tool_id": "<tool_uuid>",
    "scope": "support",
    "echo": {
      "query": "project = MESH ORDER BY updated DESC"
    }
  }
}
```

Notes:
- Registration and invocation emit audit events (`tool.registered`, `tool.invoked`).
- With `DATABASE_URL` configured, records are persisted to `tools` and `audit_events`; otherwise they use in-memory fallback stores.

## 4) Approval API
### POST /v1/approvals/{approval_id}/decision
> Bootstrap semantics: `approval_id` maps to workflow `instance_id` for now.

Request:
```json
{
  "decision": "approve",
  "comment": "validated by security"
}
```

## 5) Common Error Envelope
```json
{
  "error": {
    "code": "policy_denied",
    "message": "Action requires approval",
    "trace_id": "tr_999"
  }
}
```
