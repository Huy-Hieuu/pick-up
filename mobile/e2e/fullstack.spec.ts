import { expect, test } from '@playwright/test';
import { dateKey, day } from '../src/utils/format';

test('real preview API: login, book, create, recover history, and cancel', async ({ page, request }, testInfo) => {
  test.skip(process.env.PICKUP_FULLSTACK !== '1', 'Opt in against the disposable preview API with PICKUP_FULLSTACK=1.');
  test.skip(testInfo.project.name !== 'mobile-chromium', 'Run once to avoid sharing a mutable preview account between projects.');
  test.setTimeout(60_000);
  const api = process.env.PICKUP_API_URL || 'http://127.0.0.1:18080';
  const courtId = '10000000-0000-4000-8000-000000000001';
  const readiness = await request.get(api + '/health/ready');
  expect(readiness.status()).toBe(200);
  const evidence: Record<string, unknown> = { readiness: readiness.status() };
  let token: string | undefined;
  let slotId: string | undefined;
  let gameId: string | undefined;
  let cancelled = false;
  try {
    await page.goto('/');
    await expect(page.getByRole('link', { name: 'Xem sân Thao Dien Pickleball Club', exact: true })).toBeVisible();
    for (const width of [390, 1440]) {
      await page.setViewportSize({ width, height: 900 });
      await page.screenshot({ path: `../artifacts/mobile-qa/actual-home-${width}.png`, fullPage: true });
    }
    await page.setViewportSize({ width: 390, height: 900 });
    await page.goto(`/login?next=${encodeURIComponent('/court/' + courtId)}`);
    await page.getByLabel('Email', { exact: true }).fill('preview@example.com');
    await page.getByLabel('Mật khẩu', { exact: true }).fill('Pickup-test-2026');
    const loginResponse = page.waitForResponse((response) => response.url() === api + '/auth/login' && response.request().method() === 'POST');
    await page.getByRole('button', { name: 'Đăng nhập', exact: true }).last().click();
    const auth = await loginResponse;
    expect(auth.status()).toBe(200);
    token = (await auth.json()).access_token;
    await expect(page).toHaveURL(new RegExp(`/court/${courtId}$`));
    await page.getByRole('button', { name: day(dateKey(1) + 'T12:00:00+07:00'), exact: true }).click();
    const available = page.getByRole('button', { name: /^Chọn khung giờ /, disabled: false });
    await expect(available.last()).toBeEnabled();
    await available.last().click();
    const bookingResponse = page.waitForResponse((response) => response.url().endsWith('/book') && response.request().method() === 'POST');
    await page.getByRole('button', { name: 'Đặt sân', exact: true }).click();
    const booked = await bookingResponse;
    expect(booked.ok()).toBeTruthy();
    slotId = (await booked.json()).slot_id;
    evidence.booking = { status: booked.status(), slotId };
    await expect(page.getByText('Đã đặt sân thành công.', { exact: true })).toBeVisible();
    await page.getByRole('button', { name: 'Tạo trận từ lịch này', exact: true }).click();
    await page.getByLabel('Số người tối đa', { exact: true }).fill('4');
    await page.getByLabel('Lời nhắn cho đồng đội', { exact: true }).fill('Disposable browser verification game');
    const creationResponse = page.waitForResponse((response) => /\/games\/?$/.test(response.url()) && response.request().method() === 'POST');
    await page.getByRole('button', { name: 'Tạo trận', exact: true }).click();
    const created = await creationResponse;
    expect(created.ok()).toBeTruthy();
    gameId = (await created.json()).id;
    evidence.game = { status: created.status(), gameId };
    await expect(page).toHaveURL(new RegExp(`/game/${gameId}$`));
    await expect(page.getByRole('button', { name: 'Hủy trận', exact: true })).toBeVisible();
    // SPA navigation preserves the intentional in-memory browser session.
    await page.getByRole('link', { name: 'Tài khoản', exact: true }).click();
    await expect(page.getByRole('heading', { name: 'Chào Minh Nguyen.', exact: true })).toBeVisible();
    await expect(page.getByRole('button', { name: 'Xem trận của booking', exact: true }).first()).toBeVisible();
    await expect(page.getByRole('link', { name: 'Xem trận Thao Dien Pickleball Club', exact: true }).first()).toBeVisible();
    evidence.history = 'Booking and created game visible through authenticated profile APIs';
    await page.screenshot({ path: '../artifacts/mobile-qa/fullstack-history.png', fullPage: true });
    await page.getByRole('button', { name: 'Xem trận của booking', exact: true }).first().click();
    await page.getByRole('button', { name: 'Hủy trận', exact: true }).click();
    const cancellationResponse = page.waitForResponse((response) => response.url() === `${api}/games/${gameId}/cancel`);
    await page.getByRole('button', { name: 'Xác nhận hủy trận', exact: true }).click();
    const cancellation = await cancellationResponse;
    expect(cancellation.ok()).toBeTruthy();
    cancelled = true;
    await expect(page.getByText('Đã hủy', { exact: true }).filter({ visible: true })).toBeVisible();
    evidence.cancellation = cancellation.status();
  } finally {
    if (token && !cancelled) {
      const path = gameId ? `/games/${gameId}/cancel` : slotId ? `/courts/${courtId}/slots/${slotId}/cancel` : undefined;
      if (path) evidence.fallbackCleanup = (await request.post(api + path, { headers: { Authorization: 'Bearer ' + token } })).status();
    }
    await testInfo.attach('actual-preview-evidence', { body: JSON.stringify(evidence, null, 2), contentType: 'application/json' });
  }
});
