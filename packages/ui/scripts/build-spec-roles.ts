/**
 * Spec-role token generation from the canonical DESIGN.md.
 *
 * The repository root DESIGN.md is the only product design spec. Studio
 * chrome is its source. `src/styles/tokens/spec-roles.css` is GENERATED from
 * its Color / Type / Shape tables. Editing the generated file is refused by
 * `theme:check` — change the root DESIGN.md, then run `theme:build`.
 *
 * Role naming: table roles are kebab-cased (`Selected row` →
 * `--spec-selected-row`). Where a value cell carries two swatches
 * (`#1c1c1c` / `#222`) the FIRST is canonical and the full cell is kept in
 * the trailing comment; secondary swatches belong to the component that
 * needs them, not to the role contract.
 */
import { promises as fs } from 'node:fs'
import path from 'node:path'
import { fileURLToPath } from 'node:url'

const __filename = fileURLToPath(import.meta.url)
const __dirname = path.dirname(__filename)
export const DEFAULT_PACKAGE_ROOT = path.resolve(__dirname, '..')
export const DEFAULT_DESIGN_DOC = path.resolve(DEFAULT_PACKAGE_ROOT, '../../DESIGN.md')
const STYLES_DIR = path.join(DEFAULT_PACKAGE_ROOT, 'src/styles')
const OUTPUT_PATH = path.join(STYLES_DIR, 'tokens/spec-roles.css')

const KEBAB = /[^a-zA-Z0-9]+/g

const kebab = (value: string): string =>
  value.trim().toLowerCase().replace(KEBAB, '-').replace(/^-+|-+$/g, '')

interface SpecRow {
  role: string
  value: string
  note: string
}

/** Markdown table rows of one `## <n>. <Title>` section, cells split. */
function sectionRows(design: string, title: string): string[][] {
  const start = design.indexOf(`## ${title}`)
  if (start < 0) return []
  const next = design.indexOf('\n## ', start + 1)
  const body = design.slice(start, next < 0 ? undefined : next)
  const rows: string[][] = []
  for (const line of body.split('\n')) {
    const cells = line.split('|').map((cell) => cell.trim())
    // drop the leading/trailing empties of `| a | b |` and separator rows
    if (cells.length < 3) continue
    if (cells[0] === '' ) cells.shift()
    if (cells[cells.length - 1] === '') cells.pop()
    if (cells.every((cell) => /^:?-{2,}:?$/.test(cell) || cell === '')) continue
    if (cells.length < 2) continue
    rows.push(cells)
  }
  return rows.filter((cells) => !/^Role$/i.test(cells[0]) && !/^Element$/i.test(cells[0]))
}

const firstHex = (value: string): string | null => {
  const match = value.match(/#[0-9a-fA-F]{3,8}\b/)
  return match ? match[0].toLowerCase() : null
}

const firstPx = (value: string): string | null => {
  const match = value.match(/(\d+(?:\.\d+)?)px/)
  return match ? `${match[1]}px` : null
}

const parseColor = (design: string): SpecRow[] =>
  sectionRows(design, '2. Color')
    .map(([role, value, where]) => ({ role, value, note: where ?? '' }))
    .filter((row) => firstHex(row.value) !== null)
    .map((row) => ({ ...row, value: firstHex(row.value) as string }))

const parseType = (design: string): SpecRow[] =>
  sectionRows(design, '3. Type')
    .map(([role, size, weight, use]) => ({
      role,
      value: `${firstPx(size ?? '') ?? (size ?? '').split(',')[0].trim()} / ${weight ?? ''}`,
      note: use ?? ''
    }))
    .filter((row) => row.value.trim() !== '/' && row.value.split(' / ')[0] !== '')

const parseShape = (design: string): SpecRow[] =>
  sectionRows(design, '5. Shape, elevation, and focus')
    .map(([element, radius, shadow]) => {
      const first = (element ?? '').split(',')[0]
      const px = firstPx(radius ?? '')
      return [
        { role: `radius ${first}`, value: px ?? (radius ?? '').trim(), note: (element ?? '').trim() },
        { role: `shadow ${first}`, value: (shadow ?? '').trim(), note: (element ?? '').trim() }
      ]
    })
    .flat()
    .filter((row) => row.value !== '' && row.value !== 'none' ? true : row.value === 'none')

const cssVar = (role: string): string => `--spec-${kebab(role)}`

function renderSection(title: string, rows: SpecRow[]): string {
  if (rows.length === 0) return ''
  const body = rows
    .map((row) => `  ${cssVar(row.role)}: ${row.value.replaceAll(';', ',')}; /* ${row.role} — ${row.note} */`)
    .join('\n')
  return `  /* ==== ${title} ==== */\n${body}\n`
}

export function buildSpecRolesCss(design: string): string {
  const sections = [
    renderSection('Color roles (DESIGN.md §2)', parseColor(design)),
    renderSection('Type roles (DESIGN.md §3)', parseType(design)),
    renderSection('Shape roles (DESIGN.md §5)', parseShape(design))
  ].filter(Boolean)
  return `/**
 * GENERATED from DESIGN.md — DO NOT EDIT.
 * Change DESIGN.md, then run \`pnpm --filter @a3s-lab/ui theme:build\`.
 * The spec-role variables are the machine-readable projection of the
 * canonical design spec; semantic layers and the compatibility bridge
 * consume them instead of repeating literal values. Dark values live in
 * DESIGN.md prose (per-role "Dark #…" notes) and are consumed by the
 * semantic layers that own dark mirroring — no empty .dark block is
 * emitted (the flat-contract mirror rejects declaration-less blocks).
 */

:root {
${sections.join('\n')}}
`
}

export async function writeSpecRolesCss(
  designPath = DEFAULT_DESIGN_DOC,
  outputPath = OUTPUT_PATH
): Promise<void> {
  const design = await fs.readFile(designPath, 'utf8')
  await fs.writeFile(outputPath, buildSpecRolesCss(design), 'utf8')
}

if (process.argv[1] && path.resolve(process.argv[1]) === __filename) {
  void writeSpecRolesCss().catch((error: unknown) => {
    process.stderr.write(`${error instanceof Error ? error.message : String(error)}\n`)
    process.exitCode = 1
  })
}
