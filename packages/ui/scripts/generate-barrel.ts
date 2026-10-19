/**
 * Generator for the merged public barrel `src/index.ts`.
 *
 * Expands every `export *` in the cherry-derived barrels into explicit named
 * re-exports (studio barrel rule: re-export only, no `export *`, no nesting),
 * appends the console-kit section with its rename map, and fails on duplicate
 * export names. Run: `bun scripts/generate-barrel.ts`
 */
import { Project, SyntaxKind } from 'ts-morph'
import { readdirSync, writeFileSync } from 'node:fs'
import { dirname, join, resolve } from 'node:path'
import { fileURLToPath } from 'node:url'

const __dirname = dirname(fileURLToPath(import.meta.url))
const srcRoot = resolve(__dirname, '../src')

const project = new Project({ tsConfigFilePath: resolve(__dirname, '../tsconfig.json') })

type Kind = 'value' | 'type'

/** Collect exported names of a module file, following its own `export *`s (not namespaces). */
function collectExports(filePath: string, seen = new Set<string>()): Map<string, Kind> {
  if (seen.has(filePath)) return new Map()
  seen.add(filePath)
  const out = new Map<string, Kind>()
  const source = project.addSourceFileAtPath(filePath)

  for (const stmt of source.getStatements()) {
    if (!stmt.isKind(SyntaxKind.ExportDeclaration)) continue
    const exportDecl = stmt as unknown as import('ts-morph').ExportDeclaration
    const moduleSpecifier = exportDecl.getModuleSpecifierValue()
    if (!moduleSpecifier) {
      for (const named of exportDecl.getNamedExports()) {
        out.set(named.getAliasNode()?.getText() ?? named.getNameNode().getText(), 'value')
      }
      continue
    }
    const namespaceMatch = stmt.getText().match(/^export\s+\*\s+as\s+(\w+)/)
    if (namespaceMatch) {
      out.set(namespaceMatch[1], 'value')
      continue
    }
    const target = resolve(dirname(filePath), moduleSpecifier)
    const candidates = [target, `${target}.ts`, `${target}.tsx`, join(target, 'index.ts')]
    const found = candidates.find((c) => {
      try {
        project.addSourceFileAtPath(c)
        return true
      } catch {
        return false
      }
    })
    if (!found) continue
    for (const [name, kind] of collectExports(found, seen)) out.set(name, kind)
  }

  for (const symbol of source.getExportSymbols()) {
    const name = symbol.getName()
    if (name === 'default') continue
    out.set(name, symbolKind(symbol))
  }
  return out
}

/** Classify a symbol: an explicit `type` modifier on its export specifier is authoritative. */
function symbolKind(symbol: import('ts-morph').Symbol): Kind {
  for (const d of symbol.getDeclarations()) {
    if (d.getKind() === SyntaxKind.ExportSpecifier) {
      const spec = d as unknown as import('ts-morph').ExportSpecifier
      if (spec.isTypeOnly()) return 'type'
    }
    if (
      d.getKind() === SyntaxKind.TypeAliasDeclaration ||
      d.getKind() === SyntaxKind.InterfaceDeclaration
    ) {
      return 'type'
    }
  }
  try {
    if (symbol.isAlias()) {
      const aliased = symbol.getAliasedSymbol()
      if (aliased && typeof aliased.getDeclarations === 'function' && aliased !== symbol) {
        return symbolKind(aliased)
      }
    }
  } catch {
    // keep the value default for non-followable symbols
  }
  return 'value'
}

function resolveModule(from: string, specifier: string): string | null {
  const target = resolve(dirname(from), specifier)
  for (const c of [target, `${target}.ts`, `${target}.tsx`, join(target, 'index.ts'), join(target, 'index.tsx')]) {
    try {
      project.addSourceFileAtPath(c)
      return c
    } catch {
      // try next candidate
    }
  }
  return null
}

function wrap(list: string[], prefix = 'export { '): string {
  const rows: string[] = []
  let line = ''
  for (const item of list) {
    const piece = item.length + 2
    if (line.length + piece > 96) {
      rows.push(line)
      line = item
    } else {
      line = line ? `${line}, ${item}` : item
    }
  }
  if (line) rows.push(line)
  return rows
    .map((r, i) => (i === 0 ? `${prefix}${r}` : `  ${r}`))
    .join(',\n')
}

/** Expand one source barrel into explicit per-module export lines. */
function expandBarrelFile(
  barrelPath: string,
  renames: Record<string, string> = {},
  exclude: string[] = [],
  specifierPrefix = ''
): string[] {
  const lines: string[] = []
  const source = project.addSourceFileAtPath(barrelPath)
  for (const stmt of source.getStatements()) {
    if (!stmt.isKind(SyntaxKind.ExportDeclaration)) continue
    const exportDecl = stmt as unknown as import('ts-morph').ExportDeclaration
    const moduleSpecifier = exportDecl.getModuleSpecifierValue()
    if (!moduleSpecifier || !moduleSpecifier.startsWith('.')) {
      lines.push(stmt.getText())
      continue
    }
    const rewrite = (text: string) =>
      specifierPrefix ? text.replace(`'${moduleSpecifier}'`, `'${specifierPrefix}${moduleSpecifier.slice(1)}'`) : text
    const namespaceMatch = stmt.getText().match(/^export\s+\*\s+as\s+(\w+)/)
    if (namespaceMatch) {
      lines.push(rewrite(stmt.getText()))
      continue
    }
    if (exportDecl.getNamedExports().length > 0) {
      const moduleFile = resolveModule(barrelPath, moduleSpecifier)
      const kinds = moduleFile ? collectExports(moduleFile, new Set()) : new Map()
      const entries = exportDecl
        .getNamedExports()
        .map((named) => {
          const local = named.getNameNode().getText()
          const exported = named.getAliasNode()?.getText() ?? local
          const kind = kinds.get(local) ?? (named.isTypeOnly() ? 'type' : 'value')
          return { local, exported, kind }
        })
        .filter((e) => e.exported !== 'default')
      const values = entries.filter((e) => e.kind === 'value').map((e) => (e.local === e.exported ? e.local : `${e.local} as ${e.exported}`))
      const types = entries.filter((e) => e.kind === 'type').map((e) => (e.local === e.exported ? e.local : `${e.local} as ${e.exported}`))
      const spec = specifierPrefix ? `'${specifierPrefix}${moduleSpecifier.slice(1)}'` : `'${moduleSpecifier}'`
      if (values.length) lines.push(wrap(values) + ` } from ${spec}`)
      if (types.length) lines.push(wrap(types, 'export type { ') + ` } from ${spec}`)
      continue
    }
    const moduleFile = resolveModule(barrelPath, moduleSpecifier)
    if (!moduleFile) {
      lines.push(stmt.getText())
      continue
    }
    const exported = [...collectExports(moduleFile, new Set()).entries()]
      .filter(([n]) => n !== 'default' && !exclude.includes(n))
      .sort((a, b) => a[0].localeCompare(b[0]))
    const values = exported
      .filter(([, kind]) => kind === 'value')
      .map(([n]) => (renames[n] ? `${n} as ${renames[n]}` : n))
    const types = exported
      .filter(([, kind]) => kind === 'type')
      .map(([n]) => (renames[n] ? `${n} as ${renames[n]}` : n))
    const spec = specifierPrefix ? `'${specifierPrefix}${moduleSpecifier.slice(1)}'` : `'${moduleSpecifier}'`
    if (values.length) lines.push(wrap(values) + ` } from ${spec}`)
    if (types.length) lines.push(wrap(types, 'export type { ') + ` } from ${spec}`)
  }
  return lines
}

/** Console section: per-file exports with the rename map applied. */
const consoleRenames: Record<string, string> = {
  Dialog: 'ConsoleDialog',
  DialogProps: 'ConsoleDialogProps',
  ConfirmDialog: 'ConsoleConfirmDialog',
  ConfirmDialogProps: 'ConsoleConfirmDialogProps',
  Drawer: 'ConsoleDrawer',
  DrawerProps: 'ConsoleDrawerProps',
  DropdownMenu: 'ConsoleDropdownMenu',
  DropdownMenuItem: 'ConsoleDropdownMenuItem',
  FormField: 'ConsoleFormField',
  FormFieldProps: 'ConsoleFormFieldProps',
  ToastProvider: 'ConsoleToastProvider',
  EmptyState: 'ConsoleEmptyState',
  EmptyStateProps: 'ConsoleEmptyStateProps',
  PageHeader: 'ConsolePageHeader',
  PageHeaderProps: 'ConsolePageHeaderProps',
  Pagination: 'ConsolePagination',
  PaginationProps: 'ConsolePaginationProps',
  Table: 'ConsoleTable',
  Textarea: 'ConsoleTextarea',
  TextareaProps: 'ConsoleTextareaProps',
  DataTable: 'ResourceTable',
  DataTableProps: 'ResourceTableProps',
  DataTableColumn: 'ResourceTableColumn',
  DataTableLabels: 'ResourceTableLabels',
  DataTableSortState: 'ResourceTableSortState',
  DataTablePaginationState: 'ResourceTablePaginationState',
  DataTableColumnFilterState: 'ResourceTableColumnFilterState',
  DataTableSortValue: 'ResourceTableSortValue',
  // collides with the command primitive's CommandItem
  CommandItem: 'ConsoleCommandItem'
}

const consoleLines: string[] = []
for (const file of readdirSync(join(srcRoot, 'components', 'console')).filter((f) => f.endsWith('.tsx')).sort()) {
  if (file === 'calendar.tsx') continue // internal to DateRangePicker
  // DataTable re-exports the resource-table model types; ./lib/resourceTableModel is canonical
  const fileExcludes = file === 'DataTable.tsx'
    ? ['ResourceTableSortState', 'ResourceTablePaginationState', 'ResourceTableColumnFilterState', 'ResourceTableSortValue', 'SortDirection', 'ResourceTableColumnModel']
    : []
  const entries = [...collectExports(join(srcRoot, 'components', 'console', file)).entries()]
    .filter(([n]) => n !== 'default' && !fileExcludes.includes(n))
    .sort((a, b) => a[0].localeCompare(b[0]))
  const values = entries.map(([n, kind]) => (kind === 'value' ? (consoleRenames[n] ? `${n} as ${consoleRenames[n]}` : n) : null)).filter(Boolean) as unknown as string[]
  const types = entries.map(([n, kind]) => (kind === 'type' ? (consoleRenames[n] ? `${n} as ${consoleRenames[n]}` : n) : null)).filter(Boolean) as unknown as string[]
  const spec = `'./components/console/${file.replace(/\.tsx$/, '')}'`
  if (values.length) consoleLines.push(wrap(values) + ` } from ${spec}`)
  if (types.length) consoleLines.push(wrap(types, 'export type { ') + ` } from ${spec}`)
}

const sections: string[] = []

sections.push('// Shadcn primitives and composites (cherry base)')
sections.push(
  expandBarrelFile(
    join(srcRoot, 'components/index.ts'),
    {},
    [
      // dialog timing constants are canonically exported via @a3s/ui/utils
      'DIALOG_CLOSE_DURATION_MS',
      'DIALOG_UNMOUNT_DELAY_MS',
      // exported explicitly by ./composites/menu-list; the context-menu `export *` also carries it
      'menuItemVariants',
      // cherry index drift: these live in dedicated files, re-exported below
      'HeadingSlugger',
      'RehypeHeadingIdsOptions',
      'WithFullMarkdownOptions',
      'WithMathOptions'
    ],
    './components'
  ).join('\n')
)
sections.push(
  [
    "export type { HeadingSlugger, RehypeHeadingIdsOptions } from './components/composites/markdown/plugins/rehype-heading-ids'",
    "export type { WithFullMarkdownOptions, WithMathOptions } from './components/composites/markdown/presets'"
  ].join('\n')
)
sections.push('// Hooks')
sections.push(expandBarrelFile(join(srcRoot, 'hooks/index.ts'), {}, [], './hooks').join('\n'))
sections.push('// Hooks (console page-loading orchestration)')
sections.push(
  wrap(
    [...collectExports(join(srcRoot, 'hooks/useRoutePageLoading.ts')).keys()].sort((a, b) =>
      a.localeCompare(b)
    )
  ) + ` } from './hooks/useRoutePageLoading'`
)
sections.push('// Utils')
sections.push(expandBarrelFile(join(srcRoot, 'utils/index.ts'), {}, [], './utils').join('\n'))
sections.push('// Lib helpers')
sections.push(`export { cn } from './lib/utils'`)
// lib modules declare exports inline (no export statements), so emit from their symbol set
for (const [mod, renames] of [
  ['date', {}],
  ['resourceTableModel', consoleRenames]
] as const) {
  const entries = [...collectExports(join(srcRoot, 'lib', `${mod}.ts`)).entries()]
    .filter(([n]) => n !== 'default')
    .sort((a, b) => a[0].localeCompare(b[0]))
  const values = entries
    .map(([n, kind]) => (kind === 'value' ? (renames[n as keyof typeof renames] ?? n) : null))
    .filter(Boolean) as unknown as string[]
  const types = entries
    .map(([n, kind]) => (kind === 'type' ? (renames[n as keyof typeof renames] ?? n) : null))
    .filter(Boolean) as unknown as string[]
  const spec = `'./lib/${mod}'`
  if (values.length) sections.push(wrap(values) + ` } from ${spec}`)
  if (types.length) sections.push(wrap(types, 'export type { ') + ` } from ${spec}`)
}
sections.push('// Console kit')
sections.push(consoleLines.join('\n'))
sections.push('// Legacy names still imported by apps/web')
sections.push(
  [
    "export { DateRangePicker as DateTimePickRange } from './components/console/DateRangePicker'",
    "export type { DateRangePickerProps as DateTimePickRangeProps } from './components/console/DateRangePicker'"
  ].join('\n')
)

// uniqueness audit across every emitted export
const sources = new Map<string, string[]>()
const conflicts: string[] = []
{
  const temp = join(srcRoot, '__barrel_audit.ts')
  writeFileSync(temp, sections.join('\n'))
  const audit = project.addSourceFileAtPath(temp)
  for (const stmt of audit.getStatements()) {
    if (!stmt.isKind(SyntaxKind.ExportDeclaration)) continue
    const exportDecl = stmt as unknown as import('ts-morph').ExportDeclaration
    for (const named of exportDecl.getNamedExports()) {
      const exported = named.getAliasNode()?.getText() ?? named.getNameNode().getText()
      const from = exportDecl.getModuleSpecifierValue() ?? '?'
      const list = sources.get(exported) ?? []
      list.push(from)
      sources.set(exported, list)
    }
  }
  project.removeSourceFile(audit)
  for (const [exported, list] of sources) {
    if (list.length > 1) conflicts.push(`${exported}: ${list.join(' AND ')}`)
  }
}
if (conflicts.length) {
  console.error('Duplicate export names:\n' + conflicts.join('\n'))
  process.exit(1)
}

const header = `/**
 * @a3s/ui public barrel. Generated by \`scripts/generate-barrel.ts\` — edit that
 * script (or the component sources), not this list, then rerun it.
 */
`
writeFileSync(join(srcRoot, 'index.ts'), header + sections.join('\n\n') + '\n')
console.log(`wrote src/index.ts with ${sources.size} exports`)
