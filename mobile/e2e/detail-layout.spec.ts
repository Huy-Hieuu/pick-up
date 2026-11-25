import { expect, test } from '@playwright/test';
import { court, ids, login, mockApi } from './fixtures';

for (const width of [320, 390, 768, 1440]) {
  for (const screen of ['court', 'game'] as const) {
    test(`${screen} content fits ${width}px`, async ({ page }, testInfo) => {
      await page.setViewportSize({ width, height: 900 });
      await mockApi(page);
      await login(page, `/${screen}/${ids[screen]}`);
      await expect(page.getByRole('heading', { name: court.name, exact: true })).toBeVisible();
      const overflow = await page.evaluate(() => [...document.querySelectorAll('body *')].filter((element) => {
        const box = element.getBoundingClientRect();
        const style = getComputedStyle(element);
        return box.width > 0 && style.visibility !== 'hidden' && style.display !== 'none' && (box.right > innerWidth + 1 || box.left < -1);
      }).map((element) => ({ tag: element.tagName, text: element.textContent?.slice(0, 80), width: element.getBoundingClientRect().width })));
      expect(overflow, 'Visible detail content must fit the viewport').toEqual([]);
      await page.screenshot({ path: `../artifacts/mobile-qa/${testInfo.project.name}-${screen}-${width}.png`, fullPage: true });
    });
  }
}
