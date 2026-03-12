# Control Plane UI (Next.js)

Frontend for Flow Mesh workflow operations.

## Run
```bash
cd apps/control-plane-ui
npm install
npm run dev
```

Open `http://localhost:3000`.

## Environment
- `NEXT_PUBLIC_API_BASE_URL` (default: `http://127.0.0.1:8080`)

## Routes
- `/workflows` - workflow catalog/list with quick run launch
- `/runs` - run console (start run, refresh status, auto-poll, events timeline)
- `/approvals` - approval inbox with approve/reject actions

## Notes
- Tenant ID is entered once in the top header and reused across all pages.
- Run status polling calls `GET /v1/workflow-instances/{instance_id}` every 3 seconds until terminal state.
- Approval inbox expects workflow instance IDs (same as approval IDs in current API behavior).
