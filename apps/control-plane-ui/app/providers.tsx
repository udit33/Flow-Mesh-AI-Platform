'use client';

import Link from 'next/link';
import { createContext, useContext, useMemo, useState } from 'react';

type TenantContextValue = {
  tenantId: string;
  setTenantId: (value: string) => void;
};

const TenantContext = createContext<TenantContextValue | undefined>(undefined);

export function TenantProvider({ children }: { children: React.ReactNode }) {
  const [tenantId, setTenantId] = useState('11111111-1111-1111-1111-111111111111');

  const value = useMemo(() => ({ tenantId, setTenantId }), [tenantId]);

  return (
    <TenantContext.Provider value={value}>
      <div style={shellStyle}>
        <header style={headerStyle}>
          <div>
            <h1 style={{ margin: 0, fontSize: 20 }}>Flow Mesh Control Plane</h1>
            <p style={{ margin: '4px 0 0', color: '#9ca7cf', fontSize: 13 }}>Workflow operations console</p>
          </div>
          <label style={{ display: 'grid', gap: 6, minWidth: 360 }}>
            <span style={{ fontSize: 12, color: '#9ca7cf' }}>Tenant ID</span>
            <input value={tenantId} onChange={(e) => setTenantId(e.target.value)} style={inputStyle} />
          </label>
        </header>

        <nav style={navStyle}>
          <Link style={navLinkStyle} href="/workflows">
            Workflows
          </Link>
          <Link style={navLinkStyle} href="/runs">
            Run Console
          </Link>
          <Link style={navLinkStyle} href="/approvals">
            Approval Inbox
          </Link>
        </nav>

        <div style={{ padding: 24 }}>{children}</div>
      </div>
    </TenantContext.Provider>
  );
}

export function useTenant() {
  const ctx = useContext(TenantContext);
  if (!ctx) throw new Error('useTenant must be used inside TenantProvider');
  return ctx;
}

const shellStyle: React.CSSProperties = {
  minHeight: '100vh',
  background: '#0b1020',
  color: '#e6e9f2'
};

const headerStyle: React.CSSProperties = {
  display: 'flex',
  justifyContent: 'space-between',
  alignItems: 'end',
  gap: 16,
  borderBottom: '1px solid #222c55',
  padding: '18px 24px'
};

const navStyle: React.CSSProperties = {
  display: 'flex',
  gap: 12,
  borderBottom: '1px solid #222c55',
  padding: '10px 24px'
};

const navLinkStyle: React.CSSProperties = {
  color: '#d7ddf6',
  textDecoration: 'none',
  background: '#131a33',
  border: '1px solid #2c3768',
  padding: '8px 12px',
  borderRadius: 8,
  fontSize: 14
};

const inputStyle: React.CSSProperties = {
  background: '#121936',
  color: '#eaf0ff',
  border: '1px solid #2a3565',
  borderRadius: 8,
  padding: '10px 12px',
  boxSizing: 'border-box'
};
