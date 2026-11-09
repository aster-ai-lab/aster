import { defineConfig } from 'astro/config';

export default defineConfig({
  site: 'https://aster-ai-lab.github.io/aster',
  output: 'static',
  outDir: 'dist',
  publicDir: 'node_modules/.cache/astro-public',
  build: { format: 'directory' },
  server: { host: '127.0.0.1', port: 7100 },
  devToolbar: { enabled: false },
});
