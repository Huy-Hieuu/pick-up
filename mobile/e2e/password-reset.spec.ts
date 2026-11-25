import { expect, test } from '@playwright/test';
import { mockApi } from './fixtures';

test('password reset requires a verified grant and returns to login', async ({ page }) => {
  await mockApi(page);
  const apiUrl = process.env.PICKUP_API_URL || 'http://127.0.0.1:18080';
  const grant = 'a'.repeat(64);
  const requests: { path: string; body: unknown }[] = [];
  await page.route(apiUrl + '/auth/{forgot-password,verify-reset-otp,set-new-password}', async (route) => {
    const path = new URL(route.request().url()).pathname;
    requests.push({ path, body: route.request().postDataJSON() });
    await route.fulfill({ contentType: 'application/json', headers: { 'access-control-allow-origin': '*' }, body: JSON.stringify(path.endsWith('/verify-reset-otp') ? { reset_token: grant } : { message: 'Instructions sent' }) });
  });
  await page.goto('/reset-password');
  await page.getByLabel('Email', { exact: true }).fill('player@example.test');
  await page.getByRole('button', { name: 'Gửi mã đặt lại', exact: true }).click();
  await page.getByLabel('Mã xác minh', { exact: true }).fill('123456');
  await page.getByRole('button', { name: 'Xác minh mã', exact: true }).click();
  await page.getByLabel('Mật khẩu mới', { exact: true }).fill('ReplacementPassword123!');
  await page.getByLabel('Nhập lại mật khẩu', { exact: true }).fill('ReplacementPassword123!');
  await page.getByRole('button', { name: 'Đổi mật khẩu', exact: true }).click();
  await expect(page).toHaveURL(/\/login\?reset=1$/);
  await expect(page.getByText('Đã đổi mật khẩu. Đăng nhập bằng mật khẩu mới.', { exact: true })).toBeVisible();
  expect(requests).toEqual([
    { path: '/auth/forgot-password', body: { email: 'player@example.test' } },
    { path: '/auth/verify-reset-otp', body: { email: 'player@example.test', code: '123456' } },
    { path: '/auth/set-new-password', body: { reset_token: grant, new_password: 'ReplacementPassword123!' } },
  ]);
});
