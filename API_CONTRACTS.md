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
  "citations": [],
  "tool_calls": [],
  "latency_ms": 842
}
```

## 2) Workflow API
### POST /v1/workflows/{workflow_id}/run
Request:
```json
{
  "tenant_id": "t1",
  "workspace_id": "w1",
  "trigger": "api",
  "inputs": {"ticket_id": "INC-1021"}
}
```
Response:
```json
{
  "instance_id": "wf_run_001",
  "status": "running",
  "trace_id": "tr_456"
}
```

### GET /v1/workflow-instances/{instance_id}
Response:
```json
{
  "instance_id": "wf_run_001",
  "status": "waiting_approval",
  "current_node": "manager_approval",
  "started_at": "2026-03-11T00:00:00Z"
}
```

## 3) Tool Registry API
### POST /v1/tools
Registers tool metadata + schema + policy tags.

### POST /v1/tools/{tool_id}/invoke
Invokes tool with policy/permission checks.

## 4) Approval API
### POST /v1/approvals/{approval_id}/decision
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
