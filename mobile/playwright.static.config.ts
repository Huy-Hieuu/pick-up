import { defineConfig, devices } from '@playwright/test';

export default defineConfig({
  testDir: './e2e-static',
  fullyParallel: false,
  forbidOnly: !!process.env.CI,
  retries: process.env.CI ? 1 : 0,
  workers: 1,
  reporter: [['list'], ['html', { open: 'never', outputFolder: 'playwright-report-static' }]],
  use: {
    baseURL: 'http://127.0.0.1:8082',
    trace: 'retain-on-failure',
    screenshot: 'only-on-failure',
  },
  projects: [{ name: 'static-chromium', use: { ...devices['Pixel 7'] } }],
  webServer: {
    command: 'node scripts/serve-web.mjs',
    url: 'http://127.0.0.1:8082',
    reuseExistingServer: !process.env.CI,
    timeout: 30_000,
    env: { PORT: '8082', HOST: '127.0.0.1' },
  },
});
