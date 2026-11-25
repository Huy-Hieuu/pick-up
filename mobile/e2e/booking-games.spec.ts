import { expect, test } from '@playwright/test';
import { court, ids, login, mockApi } from './fixtures';

test('booking creates a game and creator can start and complete it', async ({ page }, testInfo) => {
  const api = await mockApi(page);
  await login(page, `/court/${ids.court}`);
  await page.getByRole('button', { name: /^Chọn khung giờ / }).click();
  await page.getByRole('button', { name: 'Đặt sân', exact: true }).click();
  await expect(page.getByText('Đã đặt sân thành công.', { exact: true })).toBeVisible();
  await page.getByRole('button', { name: 'Tạo trận từ lịch này', exact: true }).click();
  await page.getByLabel('Số người tối đa', { exact: true }).fill('4');
  await page.getByLabel('Lời nhắn cho đồng đội', { exact: true }).fill('Chơi vui, tôn trọng đồng đội.');
  await page.getByRole('button', { name: 'Tạo trận', exact: true }).click();
  await expect(page).toHaveURL(new RegExp(`/game/${ids.game}$`));
  await expect(page.getByRole('heading', { name: court.name, exact: true })).toBeVisible();
  expect(api.calls.find((call) => call.method === 'POST' && call.path === '/games')?.body).toEqual({ court_slot_id: ids.slot, sport_type: 'pickleball', max_players: 4, description: 'Chơi vui, tôn trọng đồng đội.' });
  await page.screenshot({ path: `../artifacts/mobile-qa/${testInfo.project.name}-game.png`, fullPage: true });
  await page.getByRole('button', { name: 'Bắt đầu trận', exact: true }).click();
  await page.getByRole('button', { name: 'Hoàn tất trận', exact: true }).click();
  await expect(page.getByRole('button', { name: 'Hoàn tất trận', exact: true })).toHaveCount(0);
  expect(api.calls.filter((call) => call.path.endsWith('/status')).map((call) => call.body)).toEqual([{ status: 'in_progress' }, { status: 'completed' }]);
});

test('player joins, sees a bill, initiates payment and confirms refreshed payment status', async ({ page }) => {
  const api = await mockApi(page);
  await login(page, `/game/${ids.game}`);
  await expect(page.getByRole('button', { name: 'Thanh toán phần của tôi', exact: true })).toHaveCount(0);
  await page.getByRole('button', { name: 'Tham gia trận', exact: true }).click();
  await expect(page.getByText('Bạn đã tham gia trận.', { exact: true })).toBeVisible();
  await expect(page.getByText('Phần tiền của bạn', { exact: true })).toBeVisible();
  await page.getByRole('button', { name: 'ZaloPay', exact: true }).click();
  await page.getByRole('button', { name: 'Thanh toán phần của tôi', exact: true }).click();
  await expect(page.getByText('Đơn thanh toán đã được tạo. Mở trang nhà cung cấp để tiếp tục.', { exact: true })).toBeVisible();
  expect(api.calls.find((call) => call.path.endsWith('/payments/initiate'))?.body).toEqual({ provider: 'zalopay' });
  const popupPromise = page.waitForEvent('popup');
  await page.getByRole('button', { name: 'Mở trang thanh toán', exact: true }).click();
  const popup = await popupPromise;
  await expect(popup).toHaveURL('https://payment.example.test/checkout');
  await popup.close();
  api.setPaid();
  await page.getByRole('button', { name: 'Tải lại trạng thái', exact: true }).click();
  await expect(page.getByText('Phần của bạn đã thanh toán', { exact: true })).toBeVisible();
  await expect(page.getByRole('button', { name: 'Thanh toán phần của tôi', exact: true })).toHaveCount(0);
});

test('leaving a game requires confirmation and removes the player bill', async ({ page }) => {
  const api = await mockApi(page);
  await login(page, `/game/${ids.game}`);
  await page.getByRole('button', { name: 'Tham gia trận', exact: true }).click();
  await page.getByRole('button', { name: 'Rời trận', exact: true }).click();
  expect(api.calls.some((call) => call.path.endsWith('/leave'))).toBeFalsy();
  await page.getByRole('button', { name: 'Xác nhận rời trận', exact: true }).click();
  await expect(page.getByRole('button', { name: 'Tham gia trận', exact: true })).toBeVisible();
  await expect(page.getByRole('button', { name: 'Thanh toán phần của tôi', exact: true })).toHaveCount(0);
  expect(api.calls.filter((call) => call.path.endsWith('/leave'))).toHaveLength(1);
});

test('web sharing copies a usable game link', async ({ page, context }) => {
  await context.grantPermissions(['clipboard-read', 'clipboard-write']);
  await mockApi(page);
  await login(page, `/game/${ids.game}`);
  await page.getByRole('button', { name: 'Chia sẻ trận', exact: true }).click();
  await expect(page.getByText('Đã sao chép liên kết chia sẻ trận.', { exact: true })).toBeVisible();
  const copied = await page.evaluate(() => navigator.clipboard.readText());
  expect(copied).toContain(`https://pickup.example.test/games/${ids.game}`);
});

test('booking cancellation requires explicit confirmation', async ({ page }) => {
  const api = await mockApi(page);
  await login(page, `/court/${ids.court}`);
  await page.getByRole('button', { name: /^Chọn khung giờ / }).click();
  await page.getByRole('button', { name: 'Đặt sân', exact: true }).click();
  await page.getByRole('button', { name: 'Hủy lịch đã đặt', exact: true }).click();
  expect(api.calls.some((call) => call.path.endsWith('/cancel'))).toBeFalsy();
  await page.getByRole('button', { name: 'Xác nhận hủy lịch', exact: true }).click();
  await expect(page.getByRole('button', { name: /^Chọn khung giờ / })).toBeEnabled();
  await expect(page.getByRole('button', { name: 'Tạo trận từ lịch này', exact: true })).toHaveCount(0);
  expect(api.calls.filter((call) => call.path.endsWith('/cancel'))).toHaveLength(1);
});
