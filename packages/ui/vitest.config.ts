import { resolve } from 'node:path'

import { defineConfig } from 'vitest/config'

export default defineConfig({
  resolve: {
    alias: {
      '@a3s-lab/ui': resolve(__dirname, 'src')
    }
  },
  test: {
    globals: true,
    environment: 'node',
    include: [
      'scripts/**/*.{test,spec}.{ts,tsx}',
      'src/**/*.{test,spec}.{ts,tsx}',
      'src/**/__tests__/**/*.{test,spec}.{ts,tsx}'
    ]
  }
})
