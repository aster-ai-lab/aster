import { rm } from "node:fs/promises";
import { fileURLToPath } from "node:url";
import { build } from "esbuild";

await rm(new URL("../dist", import.meta.url), { recursive: true, force: true });

await build({
  entryPoints: ["src/IntelligentUI.tsx", "src/jsonRender.ts"],
  outdir: "dist",
  bundle: true,
  format: "esm",
  platform: "browser",
  jsx: "automatic",
  packages: "external",
  absWorkingDir: fileURLToPath(new URL("../", import.meta.url)),
});
