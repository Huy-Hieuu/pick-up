import { expect, test } from '@playwright/test';
import { Buffer } from 'node:buffer';
import { ids, login, mockApi, session } from './fixtures';

const png = Buffer.from('iVBORw0KGgoAAAANSUhEUgAAAAEAAAABCAQAAAC1HAwCAAAAC0lEQVR42mP8/x8AAwMCAO+aJCkAAAAASUVORK5CYII=', 'base64');

test('avatar uses signed upload, completion and profile patch; removal clears it', async ({ page }) => {
  await mockApi(page);
  const apiUrl = process.env.PICKUP_API_URL || 'http://127.0.0.1:18080';
  let user = { ...session.user };
  const order: string[] = [];
  const patches: unknown[] = [];
  const publicUrl = 'https://assets.example.test/avatar.png';
  const uploadId = '77777777-7777-4777-8777-777777777777';
  await page.route(apiUrl + '/auth/me', async (route) => {
    if (route.request().method() === 'PATCH') { const patch = route.request().postDataJSON(); patches.push(patch); order.push('patch'); user = { ...user, ...patch }; }
    await route.fulfill({ contentType: 'application/json', headers: { 'access-control-allow-origin': '*' }, body: JSON.stringify(user) });
  });
  await page.route(apiUrl + '/upload/avatar', async (route) => {
    const body = route.request().postDataJSON();
    expect(body).toEqual({ content_type: 'image/png', content_length: png.length });
    order.push('ticket');
    await route.fulfill({ contentType: 'application/json', headers: { 'access-control-allow-origin': '*' }, body: JSON.stringify({ upload_id: uploadId, upload_url: 'https://uploads.example.test/avatar', object_key: 'fixture/avatar', expires_secs: 900, content_length: body.content_length, max_upload_bytes: 8388608 }) });
  });
  await page.context().route('https://uploads.example.test/avatar', async (route) => {
    if (route.request().method() === 'OPTIONS') return route.fulfill({ status: 204, headers: { 'access-control-allow-origin': '*', 'access-control-allow-methods': 'PUT', 'access-control-allow-headers': 'content-type' } });
    expect(route.request().method()).toBe('PUT');
    expect(route.request().headers().authorization).toBeUndefined();
    expect(route.request().headers()['content-type']).toBe('image/png');
    expect(route.request().postDataBuffer()).toEqual(png);
    order.push('put');
    await route.fulfill({ status: 200, headers: { 'access-control-allow-origin': '*' } });
  });
  await page.route(apiUrl + '/upload/avatar/complete', async (route) => {
    expect(route.request().postDataJSON()).toEqual({ upload_id: uploadId });
    order.push('complete');
    await route.fulfill({ contentType: 'application/json', headers: { 'access-control-allow-origin': '*' }, body: JSON.stringify({ public_url: publicUrl }) });
  });
  await page.context().route(publicUrl, (route) => route.fulfill({ contentType: 'image/png', body: png }));
  await login(page, '/profile');
  await expect(page.getByRole('button', { name: 'Đổi ảnh đại diện', exact: true })).toBeEnabled();
  const picker = page.waitForEvent('filechooser');
  await page.getByRole('button', { name: 'Đổi ảnh đại diện', exact: true }).click();
  await (await picker).setFiles({ name: 'avatar.png', mimeType: 'image/png', buffer: png });
  await expect(page.getByText('Đã cập nhật ảnh đại diện.', { exact: true })).toBeVisible();
  await expect(page.getByRole('img', { name: 'Ảnh đại diện của bạn', exact: true })).toBeVisible();
  expect(order).toEqual(['ticket', 'put', 'complete', 'patch']);
  expect(patches).toEqual([{ avatar_url: publicUrl }]);
  await page.getByRole('button', { name: 'Xóa ảnh đại diện', exact: true }).click();
  await expect(page.getByText('Đã xóa ảnh đại diện.', { exact: true })).toBeVisible();
  await expect(page.getByRole('button', { name: 'Xóa ảnh đại diện', exact: true })).toHaveCount(0);
  expect(patches).toEqual([{ avatar_url: publicUrl }, { avatar_url: null }]);
});

test('oversized photo is rejected before requesting an upload', async ({ page }) => {
  const api = await mockApi(page);
  await login(page, '/profile');
  await expect(page.getByRole('button', { name: 'Đổi ảnh đại diện', exact: true })).toBeEnabled();
  const picker = page.waitForEvent('filechooser');
  await page.getByRole('button', { name: 'Đổi ảnh đại diện', exact: true }).click();
  const oversized = Buffer.alloc(9 * 1024 * 1024); png.copy(oversized);
  await (await picker).setFiles({ name: 'oversized.png', mimeType: 'image/png', buffer: oversized });
  await expect(page.getByText('Chọn ảnh JPEG, PNG hoặc WebP có dung lượng tối đa 8 MB.', { exact: true })).toBeVisible();
  expect(api.calls.some((call) => call.path.startsWith('/upload/'))).toBeFalsy();
});

test('logout revoke failure clears local session and leaves a visible Vietnamese explanation', async ({ page }) => {
  await mockApi(page);
  const apiUrl = process.env.PICKUP_API_URL || 'http://127.0.0.1:18080';
  await page.route(apiUrl + '/auth/logout', (route) => route.fulfill({ status: 503, contentType: 'application/json', headers: { 'access-control-allow-origin': '*' }, body: JSON.stringify({ error: { code: 'UNAVAILABLE', message: 'Internal fixture failure' } }) }));
  await login(page, '/profile');
  await page.getByRole('button', { name: 'Đăng xuất', exact: true }).click();
  await expect(page.getByRole('button', { name: 'Đăng nhập để tiếp tục', exact: true })).toBeVisible();
  await expect(page.getByText(/Đã đăng xuất trên thiết bị\. Chưa xác nhận được việc thu hồi phiên trên máy chủ\./)).toBeVisible();
  await expect(page.getByRole('button', { name: 'Đăng xuất', exact: true })).toHaveCount(0);
});

test('expired pending payment cannot open a checkout link', async ({ page }) => {
  await mockApi(page);
  const apiUrl = process.env.PICKUP_API_URL || 'http://127.0.0.1:18080';
  await page.route(apiUrl + `/games/${ids.game}/payments/initiate`, (route) => route.fulfill({ contentType: 'application/json', headers: { 'access-control-allow-origin': '*' }, body: JSON.stringify({ id: '88888888-8888-4888-8888-888888888888', amount: 150000, provider: 'momo', status: 'pending', pay_url: 'https://payment.example.test/checkout', expires_at: '2000-01-01T00:00:00Z' }) }));
  await login(page, `/game/${ids.game}`);
  await page.getByRole('button', { name: 'Tham gia trận', exact: true }).click();
  await page.getByRole('button', { name: 'Thanh toán phần của tôi', exact: true }).click();
  await expect(page.getByText('Liên kết thanh toán đã hết hạn. Tải lại trạng thái trước khi tạo yêu cầu mới.', { exact: true })).toBeVisible();
  await expect(page.getByRole('button', { name: 'Mở trang thanh toán', exact: true })).toHaveCount(0);
});
