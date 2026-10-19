import { promises as fs } from 'node:fs'
import path from 'node:path'
import { fileURLToPath } from 'node:url'

import { buildThemeContractCss, loadThemeContractInputs } from './build-theme-css'
import { buildSpecRolesCss, DEFAULT_DESIGN_DOC } from './build-spec-roles'
import { validateMigrationContract } from './validate-migration-contract'
import { validateThemeContract } from './validate-theme-contract'

/**
 * Mirror of writeThemeContractCss's tail: the authored a3s compatibility bridge
 * is appended to the generated contract so both build and check compose identically.
 */
async function composeThemeCss(stylesDir: string): Promise<string> {
  const css = buildThemeContractCss(await loadThemeContractInputs(stylesDir))
  // Mirror of writeThemeContractCss's tail (spec-role projection + compat).
  const design = await fs.readFile(DEFAULT_DESIGN_DOC, 'utf8')
  const specRoles = buildSpecRolesCss(design)
  try {
    const compat = await fs.readFile(path.join(stylesDir, 'a3s-compat.css'), 'utf8')
    return `${css}\n${specRoles}\n/* ==================== */\n/* a3s compatibility bridge */\n/* ==================== */\n\n${compat}`
  } catch {
    return `${css}\n${specRoles}`
  }
}

const __filename = fileURLToPath(import.meta.url)
const __dirname = path.dirname(__filename)
const DEFAULT_REPOSITORY_ROOT = path.resolve(__dirname, '../../..')
const DEFAULT_STYLES_DIR = path.resolve(__dirname, '../src/styles')

export function assertGeneratedThemeCssCurrent(actual: string, expected: string): void {
  if (actual !== expected) {
    throw new Error('[theme-contract] generated theme.css is stale; run pnpm --filter @a3s/ui theme:build')
  }
}

export async function checkThemeContract(
  stylesDir = DEFAULT_STYLES_DIR,
  repositoryRoot = DEFAULT_REPOSITORY_ROOT
): Promise<void> {
  await validateThemeContract(stylesDir)

  const expectedThemeCss = await composeThemeCss(stylesDir)
  const actualThemeCss = await fs.readFile(path.join(stylesDir, 'theme.css'), 'utf8')

  assertGeneratedThemeCssCurrent(actualThemeCss, expectedThemeCss)

  // DESIGN.md is the canonical spec: the generated spec-roles projection must
  // match it exactly, or the design system is drifting from its own document.
  const design = await fs.readFile(DEFAULT_DESIGN_DOC, 'utf8')
  const expectedSpecRoles = buildSpecRolesCss(design)
  const actualSpecRoles = await fs.readFile(path.join(stylesDir, 'tokens/spec-roles.css'), 'utf8')
  if (actualSpecRoles !== expectedSpecRoles) {
    throw new Error(
      '[theme-contract] tokens/spec-roles.css is stale against DESIGN.md; run pnpm --filter @a3s-lab/ui theme:build'
    )
  }
  await validateMigrationContract(repositoryRoot)
}

if (process.argv[1] && path.resolve(process.argv[1]) === __filename) {
  void checkThemeContract().catch((error: unknown) => {
    const message = error instanceof Error ? (error.stack ?? error.message) : String(error)
    process.stderr.write(`${message}\n`)
    process.exitCode = 1
  })
}
