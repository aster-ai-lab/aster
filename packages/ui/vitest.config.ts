import { resolve } from 'node:path'

import { defineConfig } from 'vitest/config'

export default defineConfig({
  resolve: {
    alias: {
      '@aster-ai-lab/ui': resolve(__dirname, 'src')
    }
  },
  test: {
    globals: true,
    setupFiles: ['./src/test/setup.ts'],
    environment: 'node',
    include: [
      'scripts/**/*.{test,spec}.{ts,tsx}',
      'src/**/*.{test,spec}.{ts,tsx}',
      'src/**/__tests__/**/*.{test,spec}.{ts,tsx}'
    ]
  }
})
