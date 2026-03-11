# ADR-0002 Workflow Engine
Status: Accepted

Decision: Use Temporal for durable orchestration in production tier.

Rationale:
- Built-in retries, timers, long-running execution, and visibility.
- Strong fit for approval waits and resumable workflows.
