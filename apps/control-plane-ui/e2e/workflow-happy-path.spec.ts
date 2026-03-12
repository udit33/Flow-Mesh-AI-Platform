import { expect, test } from '@playwright/test';

test.describe('Workflow UI happy path (stabilized)', () => {
  test('navigates workflows -> runs -> approvals and renders core panels', async ({ page }) => {
    await page.goto('/');

    await expect(page.getByRole('heading', { name: 'Flow Mesh Control Plane' })).toBeVisible();
    await expect(page.getByRole('heading', { name: 'Workflow List' })).toBeVisible();

    await page.getByRole('link', { name: 'Run Console' }).click();
    await expect(page.getByRole('heading', { name: 'Run Console' })).toBeVisible();
    await expect(page.getByRole('heading', { name: 'Events Timeline' })).toBeVisible();

    await page.getByRole('link', { name: 'Approval Inbox' }).click();
    await expect(page.getByRole('heading', { name: 'Approval Inbox' })).toBeVisible();
    await expect(page.getByRole('button', { name: 'Load Inbox' })).toBeVisible();
  });
});
