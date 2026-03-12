import { expect, test } from '@playwright/test';

test.describe('Workflow runs + approvals', () => {
  test('runs page loads and can start a run', async ({ page }) => {
    await page.goto('/runs');

    await expect(page.getByRole('heading', { name: /Run Console/i })).toBeVisible();

    await page.getByRole('button', { name: /Start Run/i }).click();

    await expect(page.getByRole('heading', { name: /Run Status/i })).toBeVisible();
    await expect(page.getByText('No run selected.')).not.toBeVisible();
  });

  test('approvals page renders inbox controls', async ({ page }) => {
    await page.goto('/approvals');

    await expect(page.getByRole('heading', { name: /Approval Inbox/i })).toBeVisible();
    await expect(page.getByRole('button', { name: /Load Inbox/i })).toBeVisible();
    await expect(page.getByText(/No approvals waiting/i)).toBeVisible();
  });
});
