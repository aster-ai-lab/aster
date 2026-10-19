import { resolve } from 'node:path'

import react from '@vitejs/plugin-react'
import { defineConfig } from 'vite'

const project = resolve(__dirname, '../../..')
export default defineConfig({
  root: resolve(__dirname, 'fixture'),
  plugins: [react()],
  resolve: {
    dedupe: ['react', 'react-dom'],
    alias: {
      '@renderer': resolve(project, 'src/renderer'),
      '@shared': resolve(project, 'src/shared'),
      '@logger': resolve(project, 'src/renderer/services/LoggerService'),
      '@a3s-lab/ui/icons/providers': resolve(project, 'packages/ui/src/components/icons/providers'),
      '@a3s-lab/ui/icons': resolve(project, 'packages/ui/src/components/icons'),
      '@a3s-lab/ui': resolve(project, '../../packages/ui/src'),
      '@cherrystudio/provider-registry/node': resolve(project, 'packages/provider-registry/src/registry-loader'),
      '@cherrystudio/provider-registry': resolve(project, 'packages/provider-registry/src'),
      '@cherrystudio/ai-core/provider': resolve(project, 'packages/aiCore/src/core/providers'),
      '@cherrystudio/ai-core/built-in/plugins': resolve(project, 'packages/aiCore/src/core/plugins/built-in'),
      '@cherrystudio/ai-core': resolve(project, 'packages/aiCore/src'),
      '@cherrystudio/ai-sdk-provider': resolve(project, 'packages/ai-sdk-provider/src'),
      '@data': resolve(project, 'src/renderer/data')
    }
  },
  server: { host: '127.0.0.1', port: 4323, strictPort: true, fs: { allow: [resolve(project, '../..')] } }
})
