import { TenantProvider } from './providers';

export const metadata = {
  title: 'Flow Mesh Control Plane',
  description: 'Flow Mesh AI Platform UI'
};

export default function RootLayout({ children }: { children: React.ReactNode }) {
  return (
    <html lang="en">
      <body style={{ margin: 0, fontFamily: 'Inter, system-ui, Arial, sans-serif' }}>
        <TenantProvider>{children}</TenantProvider>
      </body>
    </html>
  );
}
