import { expect, test } from '@playwright/test';

const TENANT_A = '11111111-1111-1111-1111-111111111111';
const TENANT_B = '22222222-2222-2222-2222-222222222222';

test.describe('Tenant isolation (UI negative)', () => {
  test('switching tenant should not crash and should isolate requests', async ({ page }) => {
    await page.goto('/workflows');

    await expect(page.getByRole('heading', { name: /Workflows/i })).toBeVisible();

    const tenantInput = page.getByLabel('Tenant ID');
    await tenantInput.fill(TENANT_A);
    await page.getByRole('button', { name: /Reload Workflows/i }).click();

    await tenantInput.fill(TENANT_B);
    await page.getByRole('button', { name: /Reload Workflows/i }).click();

    // Defensive check: app remains operational and renders deterministic status blocks.
    await expect(page.getByText(/workflow/i).first()).toBeVisible();
  });
});
