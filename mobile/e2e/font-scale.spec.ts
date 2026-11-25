import { expect, test } from '@playwright/test';
import { court, ids, login, mockApi, session } from './fixtures';
import { enlargeText, expectTextFits } from './font-helpers';

for (const width of [320, 390]) {
  for (const screen of ['discovery', 'profile', 'court', 'game'] as const) {
    test(`${screen} fits ${width}px with 130% text`, async ({ page }, testInfo) => {
      await page.setViewportSize({ width, height: 900 });
      await mockApi(page);
      if (screen === 'profile') {
        await login(page, '/profile');
        await expect(page.getByRole('heading', { name: 'Chào Minh An.', exact: true })).toBeVisible();
      } else if (screen === 'discovery') {
        await page.goto('/');
        await expect(page.getByText(court.name, { exact: true }).first()).toBeVisible();
      } else {
        await login(page, `/${screen}/${ids[screen]}`);
        if (screen === 'court') {
          await page.getByRole('button', { name: /^Chọn khung giờ / }).click();
          await page.getByRole('button', { name: 'Đặt sân', exact: true }).click();
          await expect(page.getByRole('button', { name: 'Tạo trận từ lịch này', exact: true })).toBeVisible();
        } else {
          await page.getByRole('button', { name: 'Tham gia trận', exact: true }).click();
          await expect(page.getByRole('button', { name: 'Thanh toán phần của tôi', exact: true })).toBeVisible();
        }
      }
      await enlargeText(page);
      await expectTextFits(page);
      await page.screenshot({ path: `../artifacts/mobile-qa/${testInfo.project.name}-${screen}-font130-${width}.png`, fullPage: true });
    });
  }
}

for (const width of [320, 1440]) {
  test(`50-character account name fits ${width}px at 130% text`, async ({ page }, testInfo) => {
    await page.setViewportSize({ width, height: 900 });
    await mockApi(page);
    const longName = 'Nguyễn Văn Minh Anh Bình Long Phúc Trần Hoàng Nam'.padEnd(50, '!').slice(0, 50);
    const user = { ...session.user, display_name: longName };
    const apiUrl = process.env.PICKUP_API_URL || 'http://127.0.0.1:18080';
    await page.route(apiUrl + '/auth/login', (route) => route.fulfill({ contentType: 'application/json', headers: { 'access-control-allow-origin': '*' }, body: JSON.stringify({ ...session, user }) }));
    await page.route(apiUrl + '/auth/me', (route) => route.fulfill({ contentType: 'application/json', headers: { 'access-control-allow-origin': '*' }, body: JSON.stringify(user) }));
    await login(page, '/profile');
    await expect(page.getByRole('heading', { name: `Chào ${longName}.`, exact: true })).toBeVisible();
    await enlargeText(page);
    await expectTextFits(page);
    if (width >= 850) {
      const control = page.getByRole('button', { name: 'Tài khoản', exact: true });
      const headerButton = await control.boundingBox();
      const header = await control.locator('..').boundingBox();
      expect(headerButton).not.toBeNull();
      expect(header).not.toBeNull();
      expect(headerButton!.y, 'Account control must stay below the header top').toBeGreaterThanOrEqual(header!.y);
      expect(headerButton!.y + headerButton!.height, 'Account control must fit the desktop header').toBeLessThanOrEqual(header!.y + header!.height);
    }
    await page.screenshot({ path: `../artifacts/mobile-qa/${testInfo.project.name}-long-name-font130-${width}.png`, fullPage: true });
  });
}
