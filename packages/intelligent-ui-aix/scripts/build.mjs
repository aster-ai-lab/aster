import { createHash } from "node:crypto";
import { execFileSync } from "node:child_process";
import { mkdir, readFile, writeFile, rm, cp } from "node:fs/promises";
import { fileURLToPath } from "node:url";
import { resolve } from "node:path";
import { createRequire } from "node:module";

const root = fileURLToPath(new URL("../", import.meta.url));
const { build } = createRequire(resolve(root, "../intelligent-ui/package.json"))("esbuild");
const dist = resolve(root, "dist");
await mkdir(dist, { recursive: true });
const bundle = await build({
  entryPoints: [resolve(root, "src/runtime.tsx")],
  bundle: true,
  minify: true,
  write: false,
  format: "iife",
  platform: "browser",
  target: "es2022",
  jsx: "automatic",
  define: { "process.env.NODE_ENV": '"production"' },
  alias: {
    "@a3s-lab/intelligent-ui": resolve(root, "../intelligent-ui/src/IntelligentUI.tsx"),
    "@a3s-lab/ui": resolve(root, "../ui/src"),
    react: resolve(root, "../intelligent-ui/node_modules/react"),
    "react-dom": resolve(root, "../intelligent-ui/node_modules/react-dom"),
  },
});
await mkdir(resolve(root, "stage"), { recursive: true });
await cp(resolve(root, "../intelligent-ui/skills"), resolve(root, "stage/skills"), {
  recursive: true,
});
for (const name of ["a3s-use-extension.acl", "README.md"])
  await cp(resolve(root, name), resolve(root, "stage", name));
const script = bundle.outputFiles[0].text.replace(/<\/script/gi, "<\\/script");
execFileSync(
  process.execPath,
  [
    resolve(root, "../intelligent-ui/node_modules/@tailwindcss/cli/dist/index.mjs"),
    "-i",
    resolve(root, "src/styles.css"),
    "-o",
    resolve(dist, "styles.css"),
    "--minify",
  ],
  { cwd: root, stdio: "inherit" },
);
const css = await readFile(resolve(dist, "styles.css"), "utf8");
const hash = createHash("sha256").update(script).digest("base64");
const html = `<!doctype html><html><head><meta charset="utf-8"><meta name="viewport" content="width=device-width,initial-scale=1"><meta http-equiv="Content-Security-Policy" content="default-src 'none'; script-src 'sha256-${hash}'; style-src 'unsafe-inline'; img-src data:; connect-src 'none'; form-action 'none'; base-uri 'none'"><style>${css}</style></head><body><div id="root"></div><script>${script}</script></body></html>`;
const ui = resolve(root, "stage/ui/message");
await mkdir(ui, { recursive: true });
await writeFile(resolve(ui, "index.html"), html);
const archive = resolve(dist, "a3s-intelligent-ui-0.1.0.aix");
await rm(archive, { force: true });
execFileSync(
  "zip",
  ["-X", "-q", "-r", archive, "a3s-use-extension.acl", "README.md", "skills", "ui"],
  { cwd: resolve(root, "stage") },
);
const bytes = await readFile(archive);
await writeFile(
  `${archive}.sha256`,
  `${createHash("sha256").update(bytes).digest("hex")}  a3s-intelligent-ui-0.1.0.aix\n`,
);
