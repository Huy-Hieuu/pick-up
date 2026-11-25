import { expect, type Page } from '@playwright/test';

export async function enlargeText(page: Page) {
  // Browser text enlargement approximates layout pressure; native fontScale needs a device.
  await page.evaluate(() => {
    const sizes = [...document.querySelectorAll<HTMLElement>('[class*="css-text"], input, textarea')].filter((element) => /[\p{L}\p{N}]/u.test(element.textContent || (element as HTMLInputElement).value || '')).map((element) => {
      const style = getComputedStyle(element);
      return { element, font: parseFloat(style.fontSize), line: parseFloat(style.lineHeight) };
    });
    for (const { element, font, line } of sizes) {
      element.style.setProperty('font-size', `${font * 1.3}px`, 'important');
      if (Number.isFinite(line)) element.style.setProperty('line-height', `${line * 1.3}px`, 'important');
    }
  });
}

export async function expectTextFits(page: Page) {
  const overflow = await page.evaluate(() => [...document.querySelectorAll('body *')].filter((element) => {
    const box = element.getBoundingClientRect();
    const style = getComputedStyle(element);
    return box.width > 0 && style.visibility !== 'hidden' && style.display !== 'none' && (box.right > innerWidth + 1 || box.left < -1);
  }).map((element) => ({ text: element.textContent?.slice(0, 80), width: element.getBoundingClientRect().width })));
  expect(overflow, 'Enlarged visible text must remain within the viewport').toEqual([]);
}
