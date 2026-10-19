import { createHash } from 'node:crypto'

/** Identical files below this size stay separate. The duplicates that matter are fonts and wasm. */
const MIN_BYTES = 32 * 1024

export type AssetBytes = { name: string; bytes: Uint8Array }

export type AssetDedupe = { keep: string; drop: string[] }

/** One file per byte string. A stable public path wins over a content-hashed copy. */
export function identicalAssetGroups(assets: readonly AssetBytes[]): AssetDedupe[] {
  const byHash = new Map<string, AssetBytes[]>()
  for (const asset of assets) {
    if (asset.bytes.byteLength < MIN_BYTES) continue
    const hash = createHash('sha256').update(asset.bytes).digest('hex')
    const list = byHash.get(hash)
    if (list) list.push(asset)
    else byHash.set(hash, [asset])
  }
  const groups: AssetDedupe[] = []
  for (const list of byHash.values()) {
    if (list.length < 2) continue
    const ranked = [...list].sort(
      (left, right) => keeperRank(left.name) - keeperRank(right.name) || left.name.localeCompare(right.name)
    )
    const keep = ranked[0]?.name
    if (!keep) continue
    groups.push({ keep, drop: ranked.slice(1).map((asset) => asset.name) })
  }
  return groups
}

function keeperRank(name: string): number {
  if (name.startsWith('vendor/') || name.startsWith('tenant-')) return 0
  return 1
}

export function publicAssetPath(name: string): string {
  return name.startsWith('/') ? name : `/${name}`
}

export function rewriteAssetReferences(text: string, groups: readonly AssetDedupe[]): string {
  let next = text
  for (const group of groups) {
    const target = publicAssetPath(group.keep).slice(1)
    for (const dropped of group.drop) {
      const source = publicAssetPath(dropped).slice(1)
      if (source !== target) next = next.split(source).join(target)
    }
  }
  return next
}

/** A face that already has woff2 does not need the same glyphs again as woff or ttf. */
export function woff2OnlySrc(value: string): string {
  const parts = value
    .split(',')
    .map((part) => part.trim())
    .filter((part) => part.length > 0)
  if (!parts.some((part) => /woff2/i.test(part))) return value
  const kept = parts.filter((part) => !/url\(/i.test(part) || /woff2/i.test(part))
  return kept.length === parts.length ? value : kept.join(', ')
}

export function keepWoff2FontSources() {
  return {
    postcssPlugin: 'studio-woff2-sources',
    OnceExit(root: {
      walkAtRules: (
        name: string,
        fn: (rule: { walkDecls: (prop: string, fn: (decl: { value: string }) => void) => void }) => void
      ) => void
    }) {
      root.walkAtRules('font-face', (rule) => {
        rule.walkDecls('src', (decl) => {
          decl.value = woff2OnlySrc(decl.value)
        })
      })
    }
  }
}
keepWoff2FontSources.postcss = true

type AssetSource = { source: () => string | Uint8Array; buffer?: () => Uint8Array }
type CompilationLike = {
  getAssets: () => { name: string; source: AssetSource }[]
  updateAsset: (name: string, source: unknown) => void
  deleteAsset: (name: string) => void
  hooks: {
    processAssets: {
      tap: (options: { name: string; stage: number }, fn: () => void) => void
    }
  }
}

type RspackLike = {
  Compilation: { PROCESS_ASSETS_STAGE_SUMMARIZE: number }
  sources: { RawSource: new (contents: string) => unknown }
}

function sourceBytes(source: AssetSource): Uint8Array {
  if (typeof source.buffer === 'function') return source.buffer()
  const raw = source.source()
  return typeof raw === 'string' ? new TextEncoder().encode(raw) : raw
}

function sourceText(source: AssetSource): string | null {
  const raw = source.source()
  if (typeof raw === 'string') return raw
  return null
}

/** Drop a second copy of the same bytes and point every reference at the copy that stays. */
export function studioDedupeAssetsPlugin(rspack: RspackLike) {
  return {
    name: 'studio-dedupe-identical-assets',
    apply(compiler: {
      hooks: { thisCompilation: { tap: (name: string, fn: (compilation: CompilationLike) => void) => void } }
    }) {
      compiler.hooks.thisCompilation.tap('studio-dedupe-identical-assets', (compilation) => {
        compilation.hooks.processAssets.tap(
          {
            name: 'studio-dedupe-identical-assets',
            stage: rspack.Compilation.PROCESS_ASSETS_STAGE_SUMMARIZE
          },
          () => {
            const assets = compilation.getAssets()
            const groups = identicalAssetGroups(
              assets.map((asset) => ({ name: asset.name, bytes: sourceBytes(asset.source) }))
            )
            if (groups.length === 0) return
            const dropped = new Set(groups.flatMap((group) => group.drop))
            for (const asset of assets) {
              if (dropped.has(asset.name)) continue
              if (!/\.(js|css|html)$/.test(asset.name)) continue
              const text = sourceText(asset.source)
              if (text == null) continue
              const next = rewriteAssetReferences(text, groups)
              if (next !== text) compilation.updateAsset(asset.name, new rspack.sources.RawSource(next))
            }
            for (const name of dropped) compilation.deleteAsset(name)
          }
        )
      })
    }
  }
}
