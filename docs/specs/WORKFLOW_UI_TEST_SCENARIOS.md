# Workflow UI Test Scenarios (Detailed)

## Scope
Covers the end-to-end UI workflow lifecycle:
- workflow discovery
- run execution
- approval decisioning
- timeline visibility
- tenant isolation behavior

## Preconditions
- `agent-runtime` running on `:8081`
- `api-gateway` running on `:8080`
- UI running on `:3000`
- Test tenant available (`11111111-1111-1111-1111-111111111111`)

## A. Smoke / Health
1. Home page loads and top navigation is visible.
2. Health check returns status and is rendered in UI panel.
3. Runtime complete action returns JSON output (trace_id + selected/delegated agent).

## B. Workflow Catalog
4. Workflows page loads and fetches list for tenant.
5. Empty state shown when no workflows exist.
6. Workflow row renders id/name/version/status.
7. Selecting workflow opens run console context.

## C. Run Console
8. Start run succeeds for published workflow.
9. Starting run for non-published workflow shows deterministic error.
10. Run details panel shows instance_id + status + trace_id.
11. Polling refresh updates status without full page refresh.
12. Cancel action transitions run to cancelled.
13. Retry action creates/updates run state to running.

## D. Approval Inbox
14. Approvals list shows pending approvals only when filter=pending.
15. Approver can approve request and status transitions to approved.
16. Approver can reject request and status transitions to rejected.
17. Duplicate identical decision is idempotent and UI stays stable.
18. Conflicting second decision shows conflict message.

## E. Timeline
19. Timeline shows ordered events for run:
    - run_created
    - node_started
    - approval_waiting
    - approval_decided
    - run_completed / run_failed / run_cancelled
20. Timeline survives refresh and re-fetch.

## F. Authz / RBAC
21. Non-builder cannot start run (403 mapped to clear UI message).
22. Non-approver cannot decide approvals.
23. Builder/admin can register tools (if exposed in UI later).

## G. Tenant Isolation (Critical)
24. Tenant A cannot see Tenant B workflows in list.
25. Tenant A cannot open Tenant B run instance details.
26. Tenant A cannot approve Tenant B approvals.
27. UI handles 403/404 with safe user-facing errors (no raw stack traces).

## H. Resilience / UX
28. API unavailable shows retry-friendly error banners.
29. Long-running poll does not freeze UI thread.
30. Loading states are shown on all async actions.

## Execution Modes
- **Headless (default):**
  - `cd apps/control-plane-ui && npm run test:e2e`
- **Headed/debug:**
  - `cd apps/control-plane-ui && npm run test:e2e:headed`

## Minimal CI Gate
- One required smoke spec must pass in headless Chromium:
  - load home
  - run runtime complete
  - verify output panel

## Expansion Plan
- Add mocks/fixtures for deterministic run + approval data.
- Add visual regression snapshots for workflow/run/approval pages.
- Add parallelized tenant-isolation matrix tests.
