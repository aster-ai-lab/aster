import { defineConfig } from '@playwright/test'

/** Renderer preview only. It does not start or attach to a running ToWork. */
export default defineConfig({
  testDir: './tests/e2e/workbench-office',
  timeout: 90000,
  expect: { timeout: 20000 },
  fullyParallel: false,
  workers: 1,
  retries: 0,
  reporter: 'list',
  outputDir: './test-results/workbench-office',
  use: {
    baseURL: process.env.STUDIO_E2E_BASE_URL ?? 'http://127.0.0.1:4318',
    locale: 'zh-CN',
    headless: true,
    viewport: { width: 1280, height: 800 },
    actionTimeout: 20000,
    navigationTimeout: 30000,
    launchOptions: {
      slowMo: Number(process.env.STUDIO_E2E_SLOW_MO ?? 0),
      args: process.env.STUDIO_E2E_WATCH === '1' ? ['--window-position=72,40'] : []
    },
    trace: 'off',
    screenshot: 'only-on-failure',
    video: 'off'
  },
  projects: [
    {
      name: 'chromium',
      use: { browserName: 'chromium' }
    }
  ]
})
