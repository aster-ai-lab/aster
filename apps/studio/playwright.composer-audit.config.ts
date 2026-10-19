import { defineConfig } from '@playwright/test'

import workbench from './playwright.workbench-office.config'

/** Open acceptance findings run explicitly, outside the existing Office regression suite. */
export default defineConfig({
  ...workbench,
  testDir: './tests/e2e/composer-audit',
  outputDir: './test-results/composer-audit',
  projects: [
    { name: 'chromium', use: { browserName: 'chromium' } },
    { name: 'webkit', use: { browserName: 'webkit' } }
  ]
})
