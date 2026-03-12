'use client';

import { useState } from 'react';
import { getHealth, runtimeComplete } from '../lib/api';

export default function HomePage() {
  const [tenantId, setTenantId] = useState('11111111-1111-1111-1111-111111111111');
  const [input, setInput] = useState('run workflow for approval');
  const [health, setHealth] = useState<string>('idle');
  const [output, setOutput] = useState<string>('');
  const [loading, setLoading] = useState(false);

  return (
    <main style={{ maxWidth: 980, margin: '0 auto', padding: 24 }}>
      <h1 style={{ marginTop: 8 }}>Flow Mesh Control Plane UI</h1>
      <p style={{ color: '#aeb5c9' }}>Monorepo frontend scaffold (Next.js) with API wiring.</p>

      <section style={{ display: 'grid', gap: 12, marginTop: 20 }}>
        <label>
          Tenant ID
          <input value={tenantId} onChange={(e) => setTenantId(e.target.value)} style={inputStyle} />
        </label>

        <label>
          Runtime Input
          <textarea value={input} onChange={(e) => setInput(e.target.value)} rows={4} style={inputStyle} />
        </label>

        <div style={{ display: 'flex', gap: 12 }}>
          <button
            style={btnStyle}
            onClick={async () => {
              try {
                setLoading(true);
                const h = await getHealth();
                setHealth(JSON.stringify(h, null, 2));
              } catch (e) {
                setHealth(String(e));
              } finally {
                setLoading(false);
              }
            }}
          >
            Check API Health
          </button>

          <button
            style={btnStyle}
            onClick={async () => {
              try {
                setLoading(true);
                const result = await runtimeComplete(input, tenantId);
                setOutput(JSON.stringify(result, null, 2));
              } catch (e) {
                setOutput(String(e));
              } finally {
                setLoading(false);
              }
            }}
          >
            Run Runtime Complete
          </button>
        </div>

        <div style={panelStyle}>
          <h3>Health</h3>
          <pre style={preStyle}>{health || 'No response yet'}</pre>
        </div>

        <div style={panelStyle}>
          <h3>Runtime Output</h3>
          <pre style={preStyle}>{output || 'No output yet'}</pre>
        </div>

        {loading && <p style={{ color: '#9ad' }}>Loading…</p>}
      </section>
    </main>
  );
}

const inputStyle: React.CSSProperties = {
  marginTop: 6,
  width: '100%',
  background: '#121936',
  color: '#eaf0ff',
  border: '1px solid #2a3565',
  borderRadius: 8,
  padding: '10px 12px',
  boxSizing: 'border-box'
};

const btnStyle: React.CSSProperties = {
  background: '#2d6cf6',
  color: 'white',
  border: 0,
  borderRadius: 8,
  padding: '10px 14px',
  cursor: 'pointer'
};

const panelStyle: React.CSSProperties = {
  background: '#0f1530',
  border: '1px solid #273056',
  borderRadius: 10,
  padding: 12
};

const preStyle: React.CSSProperties = {
  margin: 0,
  whiteSpace: 'pre-wrap',
  wordBreak: 'break-word',
  fontSize: 13,
  color: '#d5dcf5'
};
