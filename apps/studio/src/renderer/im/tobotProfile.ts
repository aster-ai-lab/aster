/** Local ToBot identity. The avatar geometry lives with the messages mark. */
import { useSyncExternalStore } from 'react'

export const TOBOT_SHAPES = ['blob', 'bean', 'egg', 'drop', 'cloud', 'leaf', 'cube', 'capsule'] as const
export const TOBOT_EXPRESSIONS = [
  'rest',
  'listen',
  'excited',
  'surprised',
  'skeptical',
  'angry',
  'sleepy',
  'glad',
  'curious',
  'confused',
  'bored',
  'proud',
  'shy',
  'sad',
  'laugh',
  'scared',
  'playful'
] as const
export const TOBOT_COLORS = [
  'ink',
  'brown',
  'red',
  'orange',
  'gold',
  'green',
  'teal',
  'blue',
  'violet',
  'pink',
  'gray'
] as const
export const TOBOT_EYES = ['pill', 'round', 'dot', 'line', 'wink'] as const

export type ToBotShape = (typeof TOBOT_SHAPES)[number]
export type ToBotExpression = (typeof TOBOT_EXPRESSIONS)[number]
export type ToBotColor = (typeof TOBOT_COLORS)[number]
export type ToBotEyes = (typeof TOBOT_EYES)[number]

export type ToBotProfile = {
  name: string
  personality: string
  shape: ToBotShape
  expression: ToBotExpression
  color: ToBotColor
  eyes: ToBotEyes
}

export type ToBotHostContext = {
  personaName: string
  persona: string
}

export const TOBOT_PROFILE_KEY = 'messages.tobot.profile.v1'
export const TOBOSS_PROFILE_KEY = 'messages.toboss.profile.v1'
const NAME_LIMIT = 32
const PERSONALITY_LIMIT = 400

const DEFAULT_PROFILE: ToBotProfile = {
  name: '',
  personality: '',
  shape: 'blob',
  expression: 'listen',
  color: 'ink',
  eyes: 'pill'
}

const BOSS_PROFILE: ToBotProfile = {
  name: '',
  personality: '',
  shape: 'cube',
  expression: 'proud',
  color: 'violet',
  eyes: 'round'
}

const listeners = new Set<() => void>()
const caches = new Map<string, { key: string; value: ToBotProfile }>()

export function defaultFor(storageKey: string): ToBotProfile {
  return storageKey === TOBOSS_PROFILE_KEY ? BOSS_PROFILE : DEFAULT_PROFILE
}

export function defaultToBotProfile(): ToBotProfile {
  return DEFAULT_PROFILE
}

export function subscribeToBotProfile(listener: () => void): () => void {
  listeners.add(listener)
  return () => listeners.delete(listener)
}

export function useBotProfile(storageKey: string): ToBotProfile {
  return useSyncExternalStore(
    subscribeToBotProfile,
    () => readBotProfile(storageKey),
    () => defaultFor(storageKey)
  )
}

export function useToBotProfile(): ToBotProfile {
  return useBotProfile(TOBOT_PROFILE_KEY)
}

export function readBotProfile(storageKey: string): ToBotProfile {
  const raw = typeof localStorage === 'undefined' ? null : localStorage.getItem(storageKey)
  const key = raw ?? ''
  const slot = caches.get(storageKey)
  if (slot && slot.key === key) return slot.value
  const value = parseProfile(raw, storageKey)
  caches.set(storageKey, { key, value })
  return value
}

export function readToBotProfile(): ToBotProfile {
  return readBotProfile(TOBOT_PROFILE_KEY)
}

export function writeBotProfile(storageKey: string, input: ToBotProfile): ToBotProfile {
  const next = normalizeProfile(input, defaultFor(storageKey))
  const raw = JSON.stringify(next)
  if (typeof localStorage !== 'undefined') localStorage.setItem(storageKey, raw)
  caches.set(storageKey, { key: raw, value: next })
  for (const listener of listeners) listener()
  return next
}

export function writeToBotProfile(input: ToBotProfile): ToBotProfile {
  return writeBotProfile(TOBOT_PROFILE_KEY, input)
}

/** Empty name keeps the translated ToBot label. */
export function tobotDisplayName(profile: ToBotProfile, fallback: string): string {
  const name = cleanName(profile.name)
  return name || fallback
}

/**
 * Standing identity for the ToBot process.
 * An unchanged name and an empty personality stay off the turn body.
 */
export function tobotHostContext(profile: ToBotProfile, fallbackName: string): ToBotHostContext | null {
  const personaName = cleanName(tobotDisplayName(profile, fallbackName)).slice(0, NAME_LIMIT)
  const persona = cleanPersonality(profile.personality)
  const fallback = cleanName(fallbackName).slice(0, NAME_LIMIT)
  if (!personaName) return null
  if (personaName === fallback && persona.length === 0) return null
  return { personaName, persona }
}

function parseProfile(raw: string | null, storageKey: string): ToBotProfile {
  const fallback = defaultFor(storageKey)
  if (!raw) return fallback
  try {
    return normalizeProfile(JSON.parse(raw) as unknown, fallback)
  } catch {
    return fallback
  }
}

export function normalizeProfile(value: unknown, fallback: ToBotProfile = DEFAULT_PROFILE): ToBotProfile {
  const record = value && typeof value === 'object' ? (value as Record<string, unknown>) : {}
  return {
    name: cleanName(record.name),
    personality: cleanPersonality(record.personality),
    shape: oneOf(record.shape, TOBOT_SHAPES, fallback.shape),
    expression: oneOf(record.expression, TOBOT_EXPRESSIONS, fallback.expression),
    color: oneOf(record.color, TOBOT_COLORS, fallback.color),
    eyes: oneOf(record.eyes, TOBOT_EYES, fallback.eyes)
  }
}

function oneOf<T extends string>(value: unknown, allowed: readonly T[], fallback: T): T {
  return typeof value === 'string' && (allowed as readonly string[]).includes(value) ? (value as T) : fallback
}

function cleanName(value: unknown): string {
  if (typeof value !== 'string') return ''
  return value.replace(/\0/g, '').replace(/\s+/g, ' ').trim().slice(0, NAME_LIMIT)
}

function cleanPersonality(value: unknown): string {
  if (typeof value !== 'string') return ''
  return value.replace(/\0/g, '').trim().slice(0, PERSONALITY_LIMIT)
}
