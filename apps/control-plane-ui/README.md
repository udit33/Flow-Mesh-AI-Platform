# Control Plane UI (Next.js)

Monorepo frontend scaffold for Flow Mesh control plane.

## Run
```bash
cd apps/control-plane-ui
npm install
npm run dev
```

Open `http://localhost:3000`.

## Environment
- `NEXT_PUBLIC_API_BASE_URL` (default: `http://127.0.0.1:8080`)

## Current Scope
- Health check against API gateway
- Runtime complete action
- Output rendering for trace/delegation steps
