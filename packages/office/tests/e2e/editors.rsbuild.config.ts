import base from './rsbuild.config';

export default {
  ...base,
  source: { entry: { index: './tests/e2e/editorsHarness.tsx' } },
  output: { distPath: { root: '../../target/office-quality/editors-browser' } },
  server: { ...base.server, port: 58129 },
};
