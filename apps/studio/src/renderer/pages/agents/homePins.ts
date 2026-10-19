import { useSyncExternalStore } from 'react'

/** How many plugins the home grid holds. */
export const HOME_PLUGIN_PIN_LIMIT = 6

export const HOME_PLUGIN_PIN_EVENT = 'a3s:home-plugin-pins'

const SLUG = /^[a-z][a-z0-9-]{0,63}$/
const MEMBER = /^[A-Za-z0-9_-]{1,80}$/
const STORAGE_PREFIX = 'workbench.home.plugin-pins:v1:'

type PinCache = { raw: string | null; pins: readonly string[] }

const cache = new Map<string, PinCache>()

function scope(memberId: string | null) {
  const id = memberId && MEMBER.test(memberId) ? memberId : 'local'
  return `${STORAGE_PREFIX}${id}`
}

function parsePins(raw: string | null): readonly string[] {
  if (raw == null) return []
  try {
    const parsed: unknown = JSON.parse(raw)
    if (!Array.isArray(parsed)) return []
    const pins: string[] = []
    for (const item of parsed) {
      if (typeof item !== 'string' || !SLUG.test(item) || pins.includes(item)) continue
      pins.push(item)
      if (pins.length === HOME_PLUGIN_PIN_LIMIT) break
    }
    return pins
  } catch {
    return []
  }
}

function readRaw(key: string): string | null {
  try {
    return localStorage.getItem(key)
  } catch {
    return null
  }
}

/** Pins for this member on this machine. A missing record is an empty home. */
export function readHomePluginPins(memberId: string | null): readonly string[] {
  const key = scope(memberId)
  const raw = readRaw(key)
  const hit = cache.get(key)
  if (hit && hit.raw === raw) return hit.pins
  const pins = parsePins(raw)
  cache.set(key, { raw, pins })
  return pins
}

export function writeHomePluginPins(memberId: string | null, ids: readonly string[]): readonly string[] {
  const pins = parsePins(JSON.stringify(ids))
  const key = scope(memberId)
  const raw = JSON.stringify(pins)
  try {
    localStorage.setItem(key, raw)
  } catch {
    return readHomePluginPins(memberId)
  }
  cache.set(key, { raw, pins })
  if (typeof window !== 'undefined') window.dispatchEvent(new CustomEvent(HOME_PLUGIN_PIN_EVENT))
  return pins
}

export function pinHomePlugin(memberId: string | null, slug: string): readonly string[] {
  const current = readHomePluginPins(memberId)
  if (!SLUG.test(slug) || current.includes(slug) || current.length >= HOME_PLUGIN_PIN_LIMIT) return current
  return writeHomePluginPins(memberId, [...current, slug])
}

export function unpinHomePlugin(memberId: string | null, slug: string): readonly string[] {
  return writeHomePluginPins(
    memberId,
    readHomePluginPins(memberId).filter((item) => item !== slug)
  )
}

export function homePluginPinsFull(memberId: string | null) {
  return readHomePluginPins(memberId).length >= HOME_PLUGIN_PIN_LIMIT
}

/** A package can occupy a home cell only when it declares home cards. */
export function packageHasHomeCards(pack: { home?: readonly unknown[] } | undefined) {
  return (pack?.home?.length ?? 0) > 0
}

/**
 * Drops pins whose materialized package has no home cards.
 * A pin whose package is not in the catalogue yet stays until that package appears.
 */
export function reconcileHomePluginPins(
  memberId: string | null,
  packages: readonly { slug: string; home?: readonly unknown[] }[]
): readonly string[] {
  const current = readHomePluginPins(memberId)
  const next = current.filter((slug) => {
    const pack = packages.find((item) => item.slug === slug)
    if (!pack) return true
    return packageHasHomeCards(pack)
  })
  if (next.length === current.length && next.every((slug, index) => slug === current[index])) return current
  return writeHomePluginPins(memberId, next)
}

function subscribePins(listener: () => void) {
  const onChange = () => listener()
  window.addEventListener(HOME_PLUGIN_PIN_EVENT, onChange)
  window.addEventListener('storage', onChange)
  return () => {
    window.removeEventListener(HOME_PLUGIN_PIN_EVENT, onChange)
    window.removeEventListener('storage', onChange)
  }
}

export function useHomePluginPins(memberId: string | null): readonly string[] {
  return useSyncExternalStore(
    subscribePins,
    () => readHomePluginPins(memberId),
    () => readHomePluginPins(memberId)
  )
}
