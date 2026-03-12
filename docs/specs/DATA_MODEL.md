# Data Model (v1)

## Core Entities
- Tenant
- Workspace
- User
- Role / UserRole
- Agent
- Tool
- Workflow
- WorkflowRun
- Policy
- AuditEvent

All entities are tenant-scoped by default.

## Isolation Rules
1. Every read/write must include tenant_id predicate.
2. Cross-tenant joins are forbidden.
3. Audit events are immutable and append-only.
4. WorkflowRun trace_id must be globally unique for correlation.

## Residency
- Tenant has `region` attribute.
- Runtime must route data plane calls to region-compatible stores.

## Notes
- Core schema: `db/migrations/0001_init_core.sql`
- Agent capabilities + invoke scopes extension: `db/migrations/0002_agents_capabilities_and_scopes.sql`
