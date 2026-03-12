import { expect, test } from '@playwright/test';

const TENANT_A = '11111111-1111-1111-1111-111111111111';
const TENANT_B = '22222222-2222-2222-2222-222222222222';

test.describe('Tenant isolation (UI negative)', () => {
  test('switching tenant remains stable and keeps core workflow actions available', async ({ page }) => {
    await page.goto('/workflows');

    await expect(page.getByRole('heading', { name: /Workflow List/i })).toBeVisible();

    const tenantInput = page.getByLabel('Tenant ID');
    await tenantInput.fill(TENANT_A);
    await expect(tenantInput).toHaveValue(TENANT_A);

    await tenantInput.fill(TENANT_B);
    await expect(tenantInput).toHaveValue(TENANT_B);

    await expect(page.getByRole('button', { name: /Start Run/i }).first()).toBeVisible();
  });
});
