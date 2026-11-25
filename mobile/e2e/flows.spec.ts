import { expect, test } from '@playwright/test';
import { court, ids, login, mockApi } from './fixtures';

test('saved courts persist after reload and can be removed', async ({ page }) => {
  await mockApi(page);
  await page.goto('/');
  await page.getByRole('button', { name: `Lưu sân ${court.name}`, exact: true }).click();
  await page.getByRole('link', { name: 'Đã lưu', exact: true }).click();
  await expect(page.getByText(court.name, { exact: true })).toBeVisible();
  await page.reload();
  await expect(page.getByText(court.name, { exact: true })).toBeVisible();
  await page.getByRole('button', { name: `Bỏ lưu sân ${court.name}`, exact: true }).click();
  await expect(page.getByText(court.name, { exact: true })).toHaveCount(0);
});

test('saved courts keep successful cards when one court fails to load', async ({ page }) => {
  await mockApi(page);
  const missing = '99999999-9999-4999-8999-999999999999';
  await page.addInitScript((ids) => localStorage.setItem('pickup.saved.v1', JSON.stringify(ids)), [court.id, missing]);
  await page.goto('/saved');
  await expect(page.getByText(court.name, { exact: true })).toBeVisible();
  await expect(page.getByText(/Không tải được 1 sân đã lưu/)).toBeVisible();
});

test('login retains a protected destination and logout ends the session', async ({ page }) => {
  const api = await mockApi(page);
  await login(page, `/game/${ids.game}`);
  await expect(page).toHaveURL(new RegExp(`/game/${ids.game}`));
  expect(api.calls.find((call) => call.path === '/auth/login')?.body).toEqual({ email: 'player@example.test', password: 'SecurePassword123!' });
  const profileLink = page.getByRole('link', { name: 'Tài khoản', exact: true });
  if (await profileLink.count()) await profileLink.click();
  else await page.getByRole('button', { name: 'Tài khoản', exact: true }).click();
  await expect(page.getByText('Minh An', { exact: true }).first()).toBeVisible();
  await page.getByRole('button', { name: 'Đăng xuất', exact: true }).click();
  await expect(page).toHaveURL((process.env.PICKUP_WEB_URL || 'http://127.0.0.1:18081') + '/');
  await page.goto('/profile');
  await expect(page.getByRole('button', { name: 'Đăng nhập để tiếp tục', exact: true })).toBeVisible();
  expect(api.calls.some((call) => call.path === '/auth/logout')).toBeTruthy();
});

test('registration sends only email initially, verifies full profile and retains destination', async ({ page }) => {
  const api = await mockApi(page);
  await page.goto('/register?next=%2Fprofile');
  await page.getByLabel('Email', { exact: true }).fill('player@example.test');
  await page.getByRole('button', { name: 'Gửi mã xác minh', exact: true }).click();
  await expect(page.getByLabel('Mã xác minh', { exact: true })).toBeVisible();
  expect(api.calls.find((call) => call.path === '/auth/register')?.body).toEqual({ email: 'player@example.test' });
  await page.getByLabel('Mã xác minh', { exact: true }).fill('123456');
  await page.getByLabel('Tên hiển thị', { exact: true }).fill('Minh An');
  await page.getByLabel('Số điện thoại', { exact: true }).fill('090 123 4567');
  await page.getByLabel('Mật khẩu', { exact: true }).fill('SecurePassword123!');
  await page.getByLabel('Nhập lại mật khẩu', { exact: true }).fill('SecurePassword123!');
  await page.getByRole('button', { name: 'Hoàn tất đăng ký', exact: true }).click();
  await expect(page).toHaveURL(/\/profile$/);
  expect(api.calls.find((call) => call.path === '/auth/verify-registration')?.body).toEqual({ email: 'player@example.test', code: '123456', phone: '0901234567', display_name: 'Minh An', password: 'SecurePassword123!' });
});

test('invalid login is rejected before sending credentials', async ({ page }) => {
  const api = await mockApi(page);
  await page.goto('/login');
  await page.getByLabel('Email', { exact: true }).fill('invalid');
  await page.getByLabel('Mật khẩu', { exact: true }).fill('short');
  await page.getByRole('button', { name: 'Đăng nhập', exact: true }).last().click();
  await expect(page.getByRole('alert').first()).toBeVisible();
  expect(api.calls.some((call) => call.path === '/auth/login')).toBeFalsy();
});

test('login rejects an external return target and keeps browser tokens out of storage', async ({ page }) => {
  await mockApi(page);
  await login(page, '//untrusted.example.test');
  await expect(page).toHaveURL((process.env.PICKUP_WEB_URL || 'http://127.0.0.1:18081') + '/');
  await expect(page.getByText(court.name, { exact: true }).first()).toBeVisible();
  const persisted = await page.evaluate(() => JSON.stringify({ local: { ...localStorage }, session: { ...sessionStorage } }));
  expect(persisted).not.toContain('fixture-access');
  expect(persisted).not.toContain('fixture-refresh');
});
