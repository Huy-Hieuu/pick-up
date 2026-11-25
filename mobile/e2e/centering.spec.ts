import { expect, test, type Locator } from '@playwright/test';
import { court, ids, login, mockApi } from './fixtures';

async function horizontalCenterDelta(locator: Locator) {
  return locator.evaluate((element) => {
    const item = element.getBoundingClientRect();
    let container = element.parentElement;
    while (container && getComputedStyle(container).flexWrap !== 'wrap') container = container.parentElement;
    const frame = (container || document.documentElement).getBoundingClientRect();
    return Math.abs((item.left + item.right) / 2 - (frame.left + frame.right) / 2);
  });
}

async function cardFor(locator: Locator) {
  return locator.evaluate((element) => {
    let card = element.parentElement;
    while (card && getComputedStyle(card).borderRadius !== '20px') card = card.parentElement;
    if (!card) throw new Error('Card container not found');
    const box = card.getBoundingClientRect();
    return { x: (box.left + box.right) / 2, y: (box.top + box.bottom) / 2 };
  });
}

test('single desktop result cards are horizontally centered', async ({ page }) => {
  await page.setViewportSize({ width: 1440, height: 900 });
  await mockApi(page);
  await page.addInitScript((id) => localStorage.setItem('pickup.saved.v1', JSON.stringify([id])), court.id);
  await page.goto('/');
  const courtLink = page.getByRole('link', { name: `Xem sân ${court.name}`, exact: true });
  await expect(courtLink).toBeVisible();
  expect(await horizontalCenterDelta(courtLink)).toBeLessThanOrEqual(2);
  await login(page, '/saved');
  await expect(page.getByText(court.name, { exact: true })).toBeVisible();
  expect(await horizontalCenterDelta(courtLink)).toBeLessThanOrEqual(2);
  await page.getByRole('link', { name: 'Trận đấu', exact: true }).click();
  const gameLink = page.getByRole('link', { name: `Xem trận ${court.name}`, exact: true });
  await expect(gameLink).toBeVisible();
  expect(await horizontalCenterDelta(gameLink)).toBeLessThanOrEqual(2);
});

test('empty state box is centered on both axes', async ({ page }) => {
  await page.setViewportSize({ width: 1440, height: 900 });
  await mockApi(page, 'empty');
  await page.goto('/');
  const message = page.getByText(/Chưa có sân cho bộ lọc này/);
  await expect(message).toBeVisible();
  const layout = await message.evaluate((element) => {
    let box = element.parentElement;
    while (box && getComputedStyle(box).maxWidth !== '560px') box = box.parentElement;
    if (!box) throw new Error('State box not found');
    const style = getComputedStyle(box);
    const rect = box.getBoundingClientRect();
    return {
      alignItems: style.alignItems,
      justifyContent: style.justifyContent,
      alignSelf: style.alignSelf,
      centerDelta: Math.abs((rect.left + rect.right) / 2 - innerWidth / 2),
    };
  });
  expect(layout).toEqual({ alignItems: 'center', justifyContent: 'center', alignSelf: 'center', centerDelta: expect.any(Number) });
  expect(layout.centerDelta).toBeLessThanOrEqual(2);
});

test('desktop create-game form is horizontally centered', async ({ page }) => {
  await page.setViewportSize({ width: 1440, height: 900 });
  await mockApi(page);
  await login(page, `/game/create?slot=${ids.slot}&court=${ids.court}`);
  await expect(page.getByText(court.name, { exact: true })).toBeVisible();
  const form = page.getByRole('button', { name: 'Tạo trận', exact: true });
  const centerDelta = await form.evaluate((element) => {
    let card = element.parentElement;
    while (card && getComputedStyle(card).maxWidth !== '760px') card = card.parentElement;
    if (!card) throw new Error('Create-game form not found');
    const rect = card.getBoundingClientRect();
    return Math.abs((rect.left + rect.right) / 2 - innerWidth / 2);
  });
  expect(centerDelta).toBeLessThanOrEqual(2);
});

test('desktop header active text is vertically centered in its pill', async ({ page }) => {
  await page.setViewportSize({ width: 1440, height: 900 });
  await mockApi(page);
  await page.goto('/');
  const link = page.getByRole('link', { name: 'Khám phá', exact: true });
  await expect(link).toBeVisible();
  const centers = await link.evaluate((element) => {
    const linkBox = element.getBoundingClientRect();
    const textBox = element.querySelector('span')!.getBoundingClientRect();
    return {
      link: (linkBox.top + linkBox.bottom) / 2,
      text: (textBox.top + textBox.bottom) / 2,
      height: linkBox.height,
    };
  });
  expect(centers.height).toBeCloseTo(44, 1);
  expect(Math.abs(centers.link - centers.text)).toBeLessThanOrEqual(1);
});

test('login form fits the embedded browser viewport without touching the signup panel', async ({ page }) => {
  await page.setViewportSize({ width: 1154, height: 785 });
  await mockApi(page);
  await page.goto('/login');
  const button = page.getByRole('button', { name: 'Đăng nhập', exact: true }).last();
  const signup = page.getByRole('link', { name: /Tạo tài khoản/ });
  await expect(button).toBeVisible();
  await expect(signup).toBeInViewport();
  await page.waitForTimeout(700);
  const buttonBox = await button.boundingBox();
  const signupBox = await signup.boundingBox();
  expect(buttonBox).not.toBeNull();
  expect(signupBox).not.toBeNull();
  expect(signupBox!.y - (buttonBox!.y + buttonBox!.height)).toBeGreaterThanOrEqual(24);
});

test('mobile primary buttons keep their accessible touch height', async ({ page }) => {
  await page.setViewportSize({ width: 390, height: 844 });
  await mockApi(page);
  await page.goto('/login');
  const button = page.getByRole('button', { name: 'Đăng nhập', exact: true }).last();
  await expect(button).toBeVisible();
  const box = await button.boundingBox();
  expect(box?.height).toBeGreaterThanOrEqual(44);
});

test('reduced motion keeps content visible without entrance transforms', async ({ page }) => {
  await page.emulateMedia({ reducedMotion: 'reduce' });
  await mockApi(page);
  await page.goto('/');
  await expect(page.getByRole('heading', { name: /Ra sân/ })).toBeVisible();
  await expect(page.getByText(court.name, { exact: true }).first()).toBeVisible();
});

test('desktop profile panels are vertically centered', async ({ page }) => {
  await page.setViewportSize({ width: 1440, height: 900 });
  await mockApi(page);
  await login(page, '/profile');
  await expect(page.getByRole('heading', { name: 'Hoạt động của bạn', exact: true })).toBeVisible();
  const profile = await cardFor(page.getByText('player@example.test', { exact: true }));
  const activity = await cardFor(page.getByRole('heading', { name: 'Hoạt động của bạn', exact: true }));
  expect(Math.abs(profile.y - activity.y)).toBeLessThanOrEqual(2);
});
