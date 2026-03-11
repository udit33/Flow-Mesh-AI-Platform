# SLO / SLA Baseline

## Production Tier Targets
- Availability: **99.9%** monthly for control APIs
- Chat latency (simple requests): **p95 3-5s** end-to-end
- Workflow reliability: **at-least-once** execution semantics
- Audit completeness: 100% for sensitive action paths

## Error Budgets
- Monthly downtime budget for 99.9%: ~43m 49s

## Measurement Notes
- Exclude upstream LLM provider outages from internal platform overhead metric, but include for end-user latency SLO dashboards.
- Emit tenant-scoped and service-scoped SLI dimensions.
