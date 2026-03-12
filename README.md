# Flow Mesh AI Platform

Enterprise multi-tenant AI orchestration platform.

## Current Status
- Product/architecture/docs baseline complete
- Phase 0 bootstrap in progress
- Detailed progress report: `docs/FEATURE_PROGRESS.md`

## Repo Layout
- `services/` runtime services
- `crates/` shared Rust libraries
- `docs/` architecture, ADRs, specs
- `.github/workflows/` CI

## Quick Start
```bash
cargo build
cargo test
cargo run -p api-gateway
```

Then open:

- API Gateway health: `http://127.0.0.1:8080/health`
- Control Plane UI shell: `http://127.0.0.1:8080/`

## Frontend (Monorepo)
- `apps/control-plane-ui` (Next.js scaffold)
