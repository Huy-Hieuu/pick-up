import { expect, test } from '@playwright/test';
import { login, mockApi } from './fixtures';

for (const width of [320, 390, 768, 1440]) {
  test(`account and registration fit ${width}px`, async ({ page }, testInfo) => {
    await page.setViewportSize({ width, height: 900 });
    await mockApi(page);
    await login(page, '/profile');
    await expect(page.getByRole('heading', { name: 'Chào Minh An.', exact: true })).toBeVisible();
    const profileOverflow = await page.evaluate(() => [...document.querySelectorAll('body *')].filter((element) => {
      const box = element.getBoundingClientRect();
      const style = getComputedStyle(element);
      return box.width > 0 && style.visibility !== 'hidden' && style.display !== 'none' && (box.right > innerWidth + 1 || box.left < -1);
    }).map((element) => element.textContent?.slice(0, 80)));
    expect(profileOverflow).toEqual([]);
    await page.screenshot({ path: `../artifacts/mobile-qa/${testInfo.project.name}-profile-${width}.png`, fullPage: true });
    // A fresh document deliberately clears the browser's in-memory bearer session.
    await page.goto('/register');
    await page.getByLabel('Email', { exact: true }).fill('player@example.test');
    await page.getByRole('button', { name: 'Gửi mã xác minh', exact: true }).click();
    await expect(page.getByLabel('Mã xác minh', { exact: true })).toBeVisible();
    const registrationOverflow = await page.evaluate(() => [...document.querySelectorAll('body *')].filter((element) => {
      const box = element.getBoundingClientRect();
      const style = getComputedStyle(element);
      return box.width > 0 && style.visibility !== 'hidden' && style.display !== 'none' && (box.right > innerWidth + 1 || box.left < -1);
    }).map((element) => element.textContent?.slice(0, 80)));
    expect(registrationOverflow).toEqual([]);
    await page.screenshot({ path: `../artifacts/mobile-qa/${testInfo.project.name}-register-${width}.png`, fullPage: true });
  });
}
