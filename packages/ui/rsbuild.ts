import path from 'node:path';
import { createRequire } from 'node:module';
import { fileURLToPath } from 'node:url';

const require = createRequire(import.meta.url);

/** Absolute path to `@aster/ui` source root (`packages/ui/src`). */
export function resolveUiSrc(): string {
  return path.dirname(require.resolve('@aster-ai-lab/ui'));
}

/**
 * Shared resolve settings for apps that consume `@aster/ui` as source.
 * Pass `import.meta.url` from the app's `rsbuild.config.ts`.
 *
 * Keeps `@aster/ui` on package exports (so `./styles.css` and `./rsbuild`
 * resolve correctly); only aliases `@` to the app `src`.
 */
export function asterAppResolve(importMetaUrl: string) {
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
    favicon: path.join(uiSrc, 'assets/aster-icon-light-transparent.svg'),
  };
}
