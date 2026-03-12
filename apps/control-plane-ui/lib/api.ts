const API_BASE = process.env.NEXT_PUBLIC_API_BASE_URL || 'http://127.0.0.1:8080';

export async function getHealth() {
  const res = await fetch(`${API_BASE}/health`, { cache: 'no-store' });
  if (!res.ok) throw new Error(`health failed: ${res.status}`);
  return res.json();
}

export async function runtimeComplete(input: string, tenantId: string) {
  const res = await fetch(`${API_BASE}/v1/runtime/complete`, {
    method: 'POST',
    headers: {
      'content-type': 'application/json',
      'x-tenant-id': tenantId,
      'x-roles': 'builder,tenant_admin'
    },
    body: JSON.stringify({ input })
  });

  const body = await res.json().catch(() => ({}));
  if (!res.ok) throw new Error(`runtime failed: ${res.status} ${JSON.stringify(body)}`);
  return body;
}
