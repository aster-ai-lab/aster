import base from './rsbuild.config';

export default {
  ...base,
  source: { entry: { index: './tests/e2e/pdfHarness.tsx' } },
  output: { distPath: { root: '../../target/office-quality/pdf-browser' } },
  server: { ...base.server, port: 58130 },
};
