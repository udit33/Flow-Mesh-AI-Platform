import { expect, test } from '@playwright/test';

test.describe('Runtime happy path', () => {
  test('can run runtime complete and show agent response', async ({ page }) => {
    await page.goto('/');

    await expect(page.getByRole('heading', { name: 'Flow Mesh Control Plane UI' })).toBeVisible();

    await page.getByLabel('Tenant ID').fill('11111111-1111-1111-1111-111111111111');
    await page.getByLabel('Runtime Input').fill('run workflow for approval');

    await page.getByRole('button', { name: 'Run Runtime Complete' }).click();

    await expect(page.getByRole('heading', { name: 'Runtime Output' })).toBeVisible();
    await expect(page.locator('pre')).toContainText('selected_agent');
    await expect(page.locator('pre')).toContainText('trace_id');
  });
});
