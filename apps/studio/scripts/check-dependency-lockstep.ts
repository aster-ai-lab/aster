/**
 * Guard for package families that only work when every member resolves to the same
 * release. Tiptap extensions declare an exact peer on `@tiptap/core`, and React refuses
 * to render when `react` and `react-dom` differ. A half-landed upgrade still installs
 * (pnpm only warns on unmet peers, and caret ranges resolve independently) and vitest
 * still passes, while the renderer bundle fails to link or the window throws on start.
 */
import * as fs from 'fs'
import * as path from 'path'
import { pathToFileURL } from 'url'

import semver from 'semver'
import { parse } from 'yaml'

export interface LockstepFamily {
  label: string
  /** Every package under this scope belongs to the family. */
  scope?: string
  /** Explicit members, for families that do not share a scope. */
  members?: string[]
  /** Members released on their own cadence, outside the family version. */
  independent?: string[]
}

export const LOCKSTEP_FAMILIES: LockstepFamily[] = [
  { label: '@tiptap/*', scope: '@tiptap/', independent: ['@tiptap/y-tiptap'] },
  { label: 'react/react-dom', members: ['react', 'react-dom'] }
]

const DEPENDENCY_SECTIONS = ['dependencies', 'devDependencies', 'optionalDependencies'] as const

export interface Manifest {
  dependencies?: Record<string, string>
  devDependencies?: Record<string, string>
  optionalDependencies?: Record<string, string>
}

export interface Workspace {
  overrides?: Record<string, string>
  patchedDependencies?: Record<string, string>
}

export interface InstalledPackage {
  name: string
  version: string
  peers: Record<string, string>
  /** Version each peer resolves to from the installed package, or null when missing. */
  resolvedPeers: Record<string, string | null>
}

function member(family: LockstepFamily, name: string): boolean {
  if (family.independent?.includes(name)) return false
  return Boolean(family.members?.includes(name) || (family.scope && name.startsWith(family.scope)))
}

function splitVersioned(key: string): { name: string; version: string } | null {
  const at = key.lastIndexOf('@')
  if (at <= 0) return null
  return { name: key.slice(0, at), version: key.slice(at + 1) }
}

function manifestErrors(family: LockstepFamily, manifest: Manifest, workspace: Workspace): string[] {
  const errors: string[] = []
  const declared = new Map<string, { section: string; version: string }[]>()
  for (const section of DEPENDENCY_SECTIONS) {
    for (const [name, version] of Object.entries(manifest[section] ?? {})) {
      if (!member(family, name)) continue
      declared.set(name, [...(declared.get(name) ?? []), { section, version }])
    }
  }
  for (const [name, entries] of declared) {
    if (entries.length > 1) {
      errors.push(`${name} is declared in ${entries.map((entry) => entry.section).join(' and ')}`)
    }
  }
  const versions = new Set([...declared.values()].flat().map((entry) => entry.version))
  if (versions.size > 1) {
    const listing = [...declared]
      .flatMap(([name, entries]) => entries.map((entry) => `${name}@${entry.version}`))
      .join(', ')
    errors.push(`${family.label} must share one version in package.json: ${listing}`)
    return errors
  }
  const [version] = versions
  if (version === undefined) return errors

  for (const [name, value] of Object.entries(workspace.overrides ?? {})) {
    if (member(family, name) && value !== version) {
      errors.push(`override ${name}@${value} differs from the ${family.label} version ${version}`)
    }
  }
  for (const key of Object.keys(workspace.patchedDependencies ?? {})) {
    const patched = splitVersioned(key)
    if (patched && member(family, patched.name) && patched.version !== version) {
      errors.push(`patch ${key} targets ${patched.version}, not the ${family.label} version ${version}`)
    }
  }
  return errors
}

function installedErrors(family: LockstepFamily, installed: InstalledPackage[]): string[] {
  const members = installed.filter((pkg) => member(family, pkg.name))
  const versions = new Set(members.map((pkg) => pkg.version))
  if (versions.size <= 1) return []
  const listing = members.map((pkg) => `${pkg.name}@${pkg.version}`).join(', ')
  return [`installed ${family.label} versions differ: ${listing}`]
}

export function lockstepErrors(
  manifest: Manifest,
  workspace: Workspace,
  installed: InstalledPackage[],
  families: LockstepFamily[] = LOCKSTEP_FAMILIES
): string[] {
  const errors = families.flatMap((family) => [
    ...manifestErrors(family, manifest, workspace),
    ...installedErrors(family, installed)
  ])
  for (const pkg of installed) {
    for (const [peer, range] of Object.entries(pkg.peers)) {
      if (!families.some((family) => member(family, peer))) continue
      const resolved = pkg.resolvedPeers[peer] ?? null
      if (resolved === null) {
        errors.push(`${pkg.name}@${pkg.version} needs ${peer}@${range}, which is not installed`)
      } else if (!semver.satisfies(resolved, range)) {
        errors.push(`${pkg.name}@${pkg.version} needs ${peer}@${range} but resolves ${peer}@${resolved}`)
      }
    }
  }
  return errors
}

function readJson<T>(file: string): T {
  return JSON.parse(fs.readFileSync(file, 'utf8')) as T
}

function findPackageJson(fromDir: string, name: string): string | null {
  let dir = fromDir
  for (;;) {
    const candidate = path.join(dir, 'node_modules', name, 'package.json')
    if (fs.existsSync(candidate)) return candidate
    const parent = path.dirname(dir)
    if (parent === dir) return null
    dir = parent
  }
}

function installedNames(root: string, family: LockstepFamily): string[] {
  const names = [...(family.members ?? [])]
  if (family.scope) {
    const scopeDir = path.join(root, 'node_modules', family.scope)
    if (fs.existsSync(scopeDir)) names.push(...fs.readdirSync(scopeDir).map((entry) => `${family.scope}${entry}`))
  }
  return names.filter((name) => member(family, name) && fs.existsSync(path.join(root, 'node_modules', name)))
}

export function readInstalled(root: string, families: LockstepFamily[] = LOCKSTEP_FAMILIES): InstalledPackage[] {
  return families.flatMap((family) =>
    installedNames(root, family).map((name) => {
      const dir = fs.realpathSync(path.join(root, 'node_modules', name))
      const manifest = readJson<{ version: string; peerDependencies?: Record<string, string> }>(
        path.join(dir, 'package.json')
      )
      const peers = manifest.peerDependencies ?? {}
      const resolvedPeers: Record<string, string | null> = {}
      for (const peer of Object.keys(peers)) {
        const found = findPackageJson(path.dirname(dir), peer)
        resolvedPeers[peer] = found ? readJson<{ version: string }>(found).version : null
      }
      return { name, version: manifest.version, peers, resolvedPeers }
    })
  )
}

export function main(root = path.resolve(__dirname, '..')): number {
  const manifest = readJson<Manifest>(path.join(root, 'package.json'))
  const workspace = (parse(fs.readFileSync(path.join(root, 'pnpm-workspace.yaml'), 'utf8')) ?? {}) as Workspace
  const errors = lockstepErrors(manifest, workspace, readInstalled(root))
  if (errors.length === 0) {
    console.log('deps:check: lockstep families agree')
    return 0
  }
  for (const error of errors) console.error(`deps:check: ${error}`)
  return 1
}

if (process.argv[1] && import.meta.url === pathToFileURL(process.argv[1]).href) {
  process.exit(main())
}
