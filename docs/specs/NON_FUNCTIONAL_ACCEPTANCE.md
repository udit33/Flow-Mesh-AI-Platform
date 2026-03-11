# Non-Functional Acceptance Criteria

## Multi-tenancy
- Every API request enforces tenant context.
- No cross-tenant reads in integration tests.

## Zero-trust
- All service-to-service communication authenticated.
- Policy check required before tool execution.

## Data Residency
- Tenant policy supports region pinning.
- Storage backends validated per region policy.

## Auditability
- Prompt, tool call, workflow step, approval, and admin actions are auditable with trace IDs.
