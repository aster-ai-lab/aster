import { defineConfig } from '@playwright/test'

/** Browser launch against the preview renderer. It does not start or attach to ToWork. */
export default defineConfig({
  testDir: './tests/e2e/task-conversation',
  timeout: 90000,
  expect: { timeout: 20000 },
  fullyParallel: false,
  workers: 1,
  retries: 0,
  reporter: 'list',
  outputDir: './test-results/task-conversation',
  use: {
    baseURL: 'http://127.0.0.1:4318',
    headless: true,
    actionTimeout: 20000,
    navigationTimeout: 30000,
    trace: 'off',
    screenshot: 'off',
    video: 'off'
  },
  projects: [
    {
      name: 'chromium',
      use: { browserName: 'chromium' }
    }
  ]
})
