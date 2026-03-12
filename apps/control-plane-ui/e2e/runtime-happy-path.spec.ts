import { expect, test } from '@playwright/test';

test.describe('Runtime happy path', () => {
  test('can run runtime complete and show agent response', async ({ page }) => {
    await page.goto('/workflows');

    await expect(page.getByRole('heading', { name: 'Flow Mesh Control Plane' })).toBeVisible();
    await expect(page.getByRole('heading', { name: 'Workflow List' })).toBeVisible();

    await page.getByRole('link', { name: 'Run Console' }).click();
    await expect(page.getByRole('heading', { name: 'Run Console' })).toBeVisible();

    await page.getByRole('button', { name: 'Start Run' }).click();

    await expect(page.getByRole('heading', { name: 'Run Status' })).toBeVisible();
    await expect(page.getByText(/trace/i)).toBeVisible();
  });
});
