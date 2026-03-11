# ADR-0001 Event Bus Choice
Status: Accepted

Decision: Adopt Kafka (default) with abstraction to allow NATS in constrained environments.

Rationale:
- Strong replay semantics for workflow/runtime traces.
- Mature ecosystem for enterprise observability and DLQ patterns.
