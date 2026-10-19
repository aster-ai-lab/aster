import path from 'node:path';
import { defineConfig } from 'vitest/config';

// Source-level aliases: the vendored tree tests against the monorepo's
// packages directly (this tree is not a workspace member; file: installs
// of source-exported workspace packages loop). The aliases point at the
// same sources the consuming apps resolve through their own links.
const repo = (p: string) => path.resolve(__dirname, '../../../../packages', p);

// ONE React instance for every module context: this tree's installed
// react serves the tests' testing-library, the source, AND the
// alias-loaded packages/ui sources alike — bare react, every react
// subpath (the automatic JSX runtime injects react/jsx-dev-runtime),
// and react-dom all resolve these physical files.
const react = path.resolve(__dirname, 'node_modules/react');
const reactDom = path.resolve(__dirname, 'node_modules/react-dom');

export default defineConfig({
  esbuild: { jsx: 'automatic' },
  resolve: {
    alias: [
      { find: /^react$/, replacement: react },
      { find: /^react\/(.*)$/, replacement: path.join(react, '$1') },
      { find: /^react-dom$/, replacement: reactDom },
      { find: /^react-dom\/(.*)$/, replacement: path.join(reactDom, '$1') },
            // Marking-only fakes: initialization-marks without enhancement, so the
      // fallback controllers own interaction (the tests' semantic model).
      { find: '@a3s-lab/ce/basecoat', replacement: repo('ce/dist/js/basecoat.js') },
      { find: '@a3s-lab/ce/select', replacement: path.resolve(__dirname, 'tests/__stubs__/ce-module.ts') },
      { find: '@a3s-lab/ce/code-editor', replacement: repo('ce/dist/js/code-editor.js') },
      { find: '@a3s-lab/form', replacement: repo('form/src') },
      { find: '@a3s-lab/ui', replacement: repo('ui/src') },
    ],
  },
  test: {
    // Inline the store deps that own their own react peer: externoized
    // node_modules resolve react through the store (19.3.0) and bypass
    // the alias — inlining them into the vite pipeline keeps one React.
    server: { deps: { inline: [/@uiw\//, /@codemirror\//, /@a3s-lab\//, /@replit\//, /@lezer\//] } },
    environment: 'jsdom',
    globals: true,
    setupFiles: ['./tests/setup.ts'],
    include: ['tests/**/*.test.{ts,tsx}'],
  },
});
