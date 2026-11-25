import { defineConfig, devices } from '@playwright/test';

const webPort = process.env.PICKUP_WEB_PORT || '18081';
const apiUrl = process.env.PICKUP_API_URL || 'http://127.0.0.1:18080';
const webUrl = `http://127.0.0.1:${webPort}`;

export default defineConfig({
  testDir: './e2e',
  fullyParallel: true,
  forbidOnly: !!process.env.CI,
  retries: process.env.CI ? 2 : 0,
  workers: process.env.CI ? 2 : undefined,
  reporter: [['list'], ['html', { open: 'never' }]],
  use: {
    baseURL: webUrl,
    trace: 'retain-on-failure',
    screenshot: 'only-on-failure',
  },
  projects: [
    { name: 'mobile-chromium', use: { ...devices['Pixel 7'] } },
    { name: 'desktop-chromium', use: { ...devices['Desktop Chrome'] } },
  ],
  webServer: {
    command: `npx expo start --web --port ${webPort}`,
    url: webUrl,
    reuseExistingServer: !process.env.CI,
    timeout: 120_000,
    env: { CI: '1', EXPO_PUBLIC_API_URL: apiUrl },
  },
});
