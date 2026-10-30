import { resolve } from "node:path";
import { defineConfig } from "@rsbuild/core";
import { pluginReact } from "@rsbuild/plugin-react";

export default defineConfig({
  plugins: [pluginReact()],
  dev: { hmr: false, liveReload: false, lazyCompilation: false },
  source: { entry: {
    index: "./tests/e2e/markdownHarness.tsx",
    controls: "./tests/e2e/controlsHarness.tsx",
  } },
  resolve: {
    alias: {
      react: resolve("node_modules/react"),
      "react-dom": resolve("node_modules/react-dom"),
    },
  },
  output: { distPath: { root: "../../target/knowledge-e2e/markdown-browser" } },
  server: {
    host: "127.0.0.1",
    port: 58128,
    strictPort: true,
    publicDir: { name: "./tests/e2e/public" },
  },
});
