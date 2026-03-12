export type WorkflowItem = {
  id: string;
  name: string;
  version: string;
  summary: string;
};

export const DEFAULT_WORKFLOWS: WorkflowItem[] = [
  {
    id: '11111111-1111-1111-1111-111111111101',
    name: 'Invoice Reconciliation',
    version: 'v12',
    summary: 'Classify invoice, fetch ERP context, route for manager approval.'
  },
  {
    id: '11111111-1111-1111-1111-111111111102',
    name: 'Vendor Onboarding',
    version: 'draft',
    summary: 'Collect onboarding docs, run KYC checks, and approve activation.'
  },
  {
    id: '11111111-1111-1111-1111-111111111103',
    name: 'Claims Fraud Review',
    version: 'v4',
    summary: 'Evaluate claim risk score and request manual escalation approval.'
  }
];
