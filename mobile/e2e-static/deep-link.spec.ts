import { expect, test } from '@playwright/test';
import { court, mockApi } from '../e2e/fixtures';

test('exported web build supports a direct court link', async ({ page }) => {
  await mockApi(page, 'normal', 'https://api.example.com');
  await page.goto(`/court/${court.id}`);
  await expect(page.getByRole('heading', { name: court.name, exact: true })).toBeVisible();
  await expect(page.getByRole('button', { name: /^Chọn khung giờ / }).first()).toBeVisible();
});
