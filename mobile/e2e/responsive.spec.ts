import { expect, test } from '@playwright/test';
import { court, mockApi } from './fixtures';

for (const width of [320, 390, 768, 1440]) {
  test(`discovery remains usable at ${width}px`, async ({ page }, testInfo) => {
    await page.setViewportSize({ width, height: 900 });
    const failures: string[] = [];
    page.on('pageerror', (error) => failures.push(error.message));
    await mockApi(page);
    await page.goto('/');
    await expect(page).toHaveTitle('PickUp — Ra sân, nhập hội.');
    await expect(page.getByText(court.name, { exact: true }).first()).toBeVisible();
    await expect(page.getByText('Ra sân.', { exact: false }).first()).toBeVisible();
    for (const name of ['Khám phá', 'Trận đấu', 'Đã lưu']) await expect(page.getByRole('link', { name, exact: true })).toBeVisible();
    await expect(page.getByRole('link', { name: 'Khám phá', exact: true })).toHaveAttribute('href', '/');
    await expect(page.getByRole('link', { name: `Xem sân ${court.name}`, exact: true })).toHaveAttribute('href', new RegExp(`/court/${court.id}$`));
    if (width < 850) await expect(page.getByRole('link', { name: 'Tài khoản', exact: true })).toBeVisible();
    const overflow = await page.evaluate(() => [...document.querySelectorAll('body *')].filter((element) => {
      const box = element.getBoundingClientRect();
      const style = getComputedStyle(element);
      return box.width > 0 && style.visibility !== 'hidden' && style.display !== 'none' && (box.right > innerWidth + 1 || box.left < -1);
    }).map((element) => ({ tag: element.tagName, text: element.textContent?.slice(0, 80), width: element.getBoundingClientRect().width })));
    expect(overflow, 'Visible content must fit the viewport').toEqual([]);
    expect(failures).toEqual([]);
    await page.screenshot({ path: `../artifacts/mobile-qa/${testInfo.project.name}-${width}.png`, fullPage: true });
  });
}

test('server failure offers retry and recovers without a reload', async ({ page }) => {
  await mockApi(page, 'error');
  await page.goto('/');
  await expect(page.getByText('Chưa tải được dữ liệu').first()).toBeVisible();
  await page.unroute((process.env.PICKUP_API_URL || 'http://127.0.0.1:18080') + '/**');
  await mockApi(page);
  await page.getByRole('button', { name: 'Thử lại', exact: true }).first().click();
  await expect(page.getByText(court.name, { exact: true }).first()).toBeVisible();
});

test('empty discovery keeps navigation available', async ({ page }) => {
  await mockApi(page, 'empty');
  await page.goto('/');
  await expect(page.getByText(court.name, { exact: true })).toHaveCount(0);
  await expect(page.getByRole('link', { name: 'Trận đấu', exact: true })).toBeVisible();
  await expect(page.getByText(/Chưa (có|tìm)|Không tìm/).first()).toBeVisible();
});

test('loading stays explicit while discovery is pending', async ({ page }) => {
  await mockApi(page);
  let release!: () => void;
  const pending = new Promise<void>((resolve) => { release = resolve; });
  await page.route((process.env.PICKUP_API_URL || 'http://127.0.0.1:18080') + '/courts?**', async (route) => {
    await pending;
    await route.fallback();
  });
  await page.goto('/');
  await expect(page.getByText('Đang tải…', { exact: true }).first()).toBeVisible();
  release();
  await expect(page.getByText(court.name, { exact: true }).first()).toBeVisible();
});
