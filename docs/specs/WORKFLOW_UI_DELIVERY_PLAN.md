# Workflow UI Delivery Plan

## Goal
Ship a usable workflow lifecycle UI that supports create → publish → run → approval → completion visibility with tenant-safe behavior.

## Milestone Checklist

### M1 — Foundations
- [ ] Define workflow API client contracts (create/list/get/publish/run/approvals).
- [ ] Add shared UI types for `Workflow`, `WorkflowRun`, `Approval`.
- [ ] Establish environment config (`API_BASE_URL`, auth header strategy).

### M2 — Workflow Catalog
- [ ] Workflow list page with tenant-scoped query.
- [ ] Workflow detail page with definition preview.
- [ ] Create workflow form with basic validation feedback.

### M3 — Publish + Run
- [ ] Publish action from workflow detail.
- [ ] Run workflow action with trigger/inputs form.
- [ ] Run status panel (trace id, current node, lifecycle events).

### M4 — Approval UX
- [ ] Approval inbox (pending/approved/rejected filters).
- [ ] Approval detail and decision actions.
- [ ] Decision result propagation to run status.

### M5 — Quality Gates
- [ ] Rust E2E integration tests covering create/publish/run/approval/complete.
- [ ] Tenant isolation negative tests for list/get/run/approvals.
- [ ] Playwright happy-path scaffold committed and runnable in CI.

## Acceptance Criteria
- Tenant A cannot view or mutate Tenant B workflow catalog, runs, or approvals.
- A workflow cannot be run before publish.
- Running a published workflow creates/links an approval request.
- Approval decision updates run status to terminal state (`succeeded` or `failed`).
- UI provides deterministic success/error messaging for each lifecycle step.
- CI includes Rust test execution and a documented Playwright command for e2e.

## Test/CI Notes
- Rust: `cargo fmt && cargo test`
- UI e2e (when Playwright deps are installed):
  - `cd apps/control-plane-ui`
  - `npx playwright test e2e/workflow-happy-path.spec.ts`
