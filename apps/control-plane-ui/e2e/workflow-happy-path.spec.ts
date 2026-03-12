import { expect, test } from '@playwright/test';

test.describe('Workflow UI happy path (skeleton)', () => {
  test('loads home and can trigger runtime action', async ({ page }) => {
    await page.goto(process.env.UI_BASE_URL ?? 'http://localhost:3000/');

    await expect(page.getByRole('heading', { name: 'Flow Mesh Control Plane UI' })).toBeVisible();

    const tenantId = page.getByLabel('Tenant ID');
    await tenantId.fill('11111111-1111-1111-1111-111111111111');

    const runtimeInput = page.getByLabel('Runtime Input');
    await runtimeInput.fill('run workflow for approval');

    await page.getByRole('button', { name: 'Check API Health' }).click();
    await expect(page.getByRole('heading', { name: 'Health' })).toBeVisible();

    await page.getByRole('button', { name: 'Run Runtime Complete' }).click();
    await expect(page.getByRole('heading', { name: 'Runtime Output' })).toBeVisible();

    // NOTE: This is intentionally a scaffold. Once stable API fixtures are wired,
    // assert exact response payloads and workflow state transitions in the UI.
  });
});
