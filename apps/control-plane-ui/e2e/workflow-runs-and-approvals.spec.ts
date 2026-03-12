import { expect, test } from '@playwright/test';

const TENANT_A = '11111111-1111-1111-1111-111111111111';

test.describe('Workflow runs + approvals', () => {
  test('runs page loads and supports status refresh controls', async ({ page }) => {
    await page.goto('/runs');

    await expect(page.getByRole('heading', { name: /Run Console/i })).toBeVisible();

    const tenantInput = page.getByLabel('Tenant ID');
    await tenantInput.fill(TENANT_A);

    await page.getByRole('button', { name: /Refresh Status/i }).click();
    await expect(page.getByText(/Run status/i)).toBeVisible();
  });

  test('approvals page renders decision controls', async ({ page }) => {
    await page.goto('/approvals');

    await expect(page.getByRole('heading', { name: /Approval Inbox/i })).toBeVisible();

    const tenantInput = page.getByLabel('Tenant ID');
    await tenantInput.fill(TENANT_A);

    await expect(page.getByRole('button', { name: /Approve/i })).toBeVisible();
    await expect(page.getByRole('button', { name: /Reject/i })).toBeVisible();
  });
});
