import { fileURLToPath } from "node:url";
import { build } from "esbuild";

await build({
  entryPoints: ["src/IntelligentUI.tsx", "src/openui.ts"],
  outdir: "dist",
  bundle: true,
  format: "esm",
  platform: "browser",
  jsx: "automatic",
  packages: "external",
  absWorkingDir: fileURLToPath(new URL("../", import.meta.url)),
});
