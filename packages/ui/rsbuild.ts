import path from 'node:path';
import { createRequire } from 'node:module';
import { fileURLToPath } from 'node:url';

const require = createRequire(import.meta.url);

/** Absolute path to `@a3s/ui` source root (`packages/ui/src`). */
export function resolveUiSrc(): string {
  return path.dirname(require.resolve('@a3s-lab/ui'));
}

/**
 * Shared resolve settings for apps that consume `@a3s/ui` as source.
 * Pass `import.meta.url` from the app's `rsbuild.config.ts`.
 *
 * Keeps `@a3s/ui` on package exports (so `./styles.css` and `./rsbuild`
 * resolve correctly); only aliases `@` to the app `src`.
 */
export function a3sAppResolve(importMetaUrl: string) {
  const appDir = path.dirname(fileURLToPath(importMetaUrl));
  const appSrc = path.resolve(appDir, 'src');
  const uiSrc = resolveUiSrc();

  return {
    alias: {
      '@': appSrc,
    } as Record<string, string>,
    include: [uiSrc],
    dedupe: ['react', 'react-dom', 'react-router-dom'],
    // Default tab chrome is light → dark mark (`*-light-*` assets).
    favicon: path.join(uiSrc, 'assets/a3s-icon-light-transparent.svg'),
  };
}
