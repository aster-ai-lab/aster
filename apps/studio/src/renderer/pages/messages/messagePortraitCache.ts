export type CachedPortrait = { name: string; phone: string; avatar: string }

const portraits = new Map<string, CachedPortrait>()
let selfPortrait: CachedPortrait = { name: '', phone: '', avatar: '' }

export function readCachedPortraits(): Record<string, CachedPortrait> {
  return Object.fromEntries(portraits)
}

export function readCachedSelfPortrait(): CachedPortrait {
  return selfPortrait
}

export function writeCachedPortraits(next: Record<string, CachedPortrait>, self: CachedPortrait) {
  portraits.clear()
  for (const [id, portrait] of Object.entries(next)) portraits.set(id, portrait)
  selfPortrait = self
}

export function mergeCachedPortraits(next: Record<string, CachedPortrait>) {
  for (const [id, portrait] of Object.entries(next)) portraits.set(id, portrait)
}

export function clearCachedPortraits() {
  portraits.clear()
  selfPortrait = { name: '', phone: '', avatar: '' }
}
