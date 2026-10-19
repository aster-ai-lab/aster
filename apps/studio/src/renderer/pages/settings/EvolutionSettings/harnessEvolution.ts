/** Per-version tool and skill record. The next run still reads the admitted part list. */

import type { HarnessAssembleId } from '@renderer/services/harnessAssemble'

import { admittedTool, ADMITTED_TOOLS } from './harnessDetailCatalog'

export const HARNESS_EVOLUTION_KEY = 'a3s.studio.harness-evolution.v1'

export const HOST_SKILL_ID = 'intent-review'

const LOOKUP_TOOLS = new Set(['read', 'ls', 'search', 'web_fetch', 'web_search', 'Skill'])

export interface HarnessToolEdition {
  name: string
  included: boolean
  description: string
}

export interface HarnessSkillEdition {
  id: string
  name: string
  description: string
  allowedTools: string
  content: string
}

export interface HarnessEvolution {
  scenario: string | null
  tools: HarnessToolEdition[]
  skills: HarnessSkillEdition[]
}

export interface EvolutionBook {
  default: HarnessEvolution
  lean: HarnessEvolution
  host: HarnessEvolution
}

export interface EvolutionDiffRow {
  asset: 'scenario' | 'tool' | 'skill'
  kind: 'added' | 'removed' | 'optimized'
  name: string
}

export type EvolutionStorage = Pick<Storage, 'getItem' | 'setItem'>

const listeners = new Set<() => void>()
let snapshotRaw: string | null = null
let snapshot: EvolutionBook | null = null

function toolSeed(included: (name: string) => boolean): HarnessToolEdition[] {
  return ADMITTED_TOOLS.map((tool) => ({ name: tool.name, included: included(tool.name), description: '' }))
}

function hostSkill(): HarnessSkillEdition {
  return { id: HOST_SKILL_ID, name: HOST_SKILL_ID, description: '', allowedTools: 'read,search', content: '' }
}

export function seedEvolution(id: HarnessAssembleId): HarnessEvolution {
  if (id === 'lean') return { scenario: null, tools: toolSeed((name) => LOOKUP_TOOLS.has(name)), skills: [] }
  if (id === 'host') return { scenario: null, tools: toolSeed(() => true), skills: [hostSkill()] }
  return { scenario: null, tools: toolSeed(() => true), skills: [] }
}

function seedBook(): EvolutionBook {
  return { default: seedEvolution('default'), lean: seedEvolution('lean'), host: seedEvolution('host') }
}

function clip(value: string, max: number): string {
  return value.length > max ? value.slice(0, max) : value
}

function parseSkill(value: unknown): HarnessSkillEdition | null {
  if (!value || typeof value !== 'object') return null
  const record = value as Record<string, unknown>
  if (typeof record.id !== 'string' || !record.id.trim()) return null
  return {
    id: clip(record.id.trim(), 80),
    name: typeof record.name === 'string' ? clip(record.name, 80) : '',
    description: typeof record.description === 'string' ? clip(record.description, 500) : '',
    allowedTools: typeof record.allowedTools === 'string' ? clip(record.allowedTools, 500) : '',
    content: typeof record.content === 'string' ? clip(record.content, 8000) : ''
  }
}

function mergeTools(seed: HarnessToolEdition[], stored: unknown): HarnessToolEdition[] {
  const saved = new Map<string, { included: boolean; description: string }>()
  if (Array.isArray(stored)) {
    for (const item of stored) {
      if (!item || typeof item !== 'object') continue
      const record = item as Record<string, unknown>
      if (typeof record.name !== 'string' || !admittedTool(record.name)) continue
      saved.set(record.name, {
        included: typeof record.included === 'boolean' ? record.included : true,
        description: typeof record.description === 'string' ? clip(record.description, 2000) : ''
      })
    }
  }
  return seed.map((tool) => {
    const item = saved.get(tool.name)
    return item ? { name: tool.name, included: item.included, description: item.description } : tool
  })
}

function mergeVersion(seed: HarnessEvolution, stored: unknown): HarnessEvolution {
  if (!stored || typeof stored !== 'object') return seed
  const record = stored as Record<string, unknown>
  return {
    scenario: typeof record.scenario === 'string' ? clip(record.scenario, 200) : null,
    tools: Array.isArray(record.tools) ? mergeTools(seed.tools, record.tools) : seed.tools,
    skills: Array.isArray(record.skills)
      ? record.skills.flatMap((item) => {
          const skill = parseSkill(item)
          return skill ? [skill] : []
        })
      : seed.skills
  }
}

function parseBook(raw: string | null): EvolutionBook {
  const seed = seedBook()
  if (!raw) return seed
  try {
    const parsed = JSON.parse(raw) as unknown
    if (!parsed || typeof parsed !== 'object') return seed
    const record = parsed as Record<string, unknown>
    return {
      default: mergeVersion(seed.default, record.default),
      lean: mergeVersion(seed.lean, record.lean),
      host: mergeVersion(seed.host, record.host)
    }
  } catch {
    return seed
  }
}

function storageOf(storage?: EvolutionStorage): EvolutionStorage | undefined {
  return storage ?? globalThis.localStorage
}

export function readHarnessEvolution(storage?: EvolutionStorage): EvolutionBook {
  const target = storageOf(storage)
  if (target !== globalThis.localStorage) return parseBook(target?.getItem(HARNESS_EVOLUTION_KEY) ?? null)
  const raw = target?.getItem(HARNESS_EVOLUTION_KEY) ?? null
  if (!snapshot || snapshotRaw !== raw) {
    snapshotRaw = raw
    snapshot = parseBook(raw)
  }
  return snapshot
}

function publish(next: EvolutionBook, storage?: EvolutionStorage) {
  const target = storageOf(storage)
  const raw = JSON.stringify(next)
  target?.setItem(HARNESS_EVOLUTION_KEY, raw)
  if (target === globalThis.localStorage) {
    snapshotRaw = raw
    snapshot = next
    for (const listener of listeners) listener()
  }
}

function update(
  id: HarnessAssembleId,
  recipe: (current: HarnessEvolution) => HarnessEvolution,
  storage?: EvolutionStorage
) {
  const target = storageOf(storage)
  const current = readHarnessEvolution(target)
  publish({ ...current, [id]: recipe(current[id]) }, target)
}

export function writeHarnessScenario(id: HarnessAssembleId, scenario: string, storage?: EvolutionStorage): void {
  update(id, (current) => ({ ...current, scenario: clip(scenario, 200) }), storage)
}

export function writeHarnessTool(
  id: HarnessAssembleId,
  name: string,
  patch: { included?: boolean; description?: string },
  storage?: EvolutionStorage
): void {
  if (!admittedTool(name)) return
  update(
    id,
    (current) => ({
      ...current,
      tools: current.tools.map((tool) => {
        if (tool.name !== name) return tool
        return {
          name,
          included: patch.included ?? tool.included,
          description: patch.description === undefined ? tool.description : clip(patch.description, 2000)
        }
      })
    }),
    storage
  )
}

export function addHarnessSkill(id: HarnessAssembleId, storage?: EvolutionStorage): string {
  const skill: HarnessSkillEdition = {
    id: `skill-${crypto.randomUUID()}`,
    name: '',
    description: '',
    allowedTools: '',
    content: ''
  }
  update(id, (current) => ({ ...current, skills: [...current.skills, skill] }), storage)
  return skill.id
}

export function writeHarnessSkill(
  id: HarnessAssembleId,
  skillId: string,
  patch: Partial<Pick<HarnessSkillEdition, 'name' | 'description' | 'allowedTools' | 'content'>>,
  storage?: EvolutionStorage
): void {
  update(
    id,
    (current) => ({
      ...current,
      skills: current.skills.map((skill) => {
        if (skill.id !== skillId) return skill
        return {
          ...skill,
          name: patch.name === undefined ? skill.name : clip(patch.name, 80),
          description: patch.description === undefined ? skill.description : clip(patch.description, 500),
          allowedTools: patch.allowedTools === undefined ? skill.allowedTools : clip(patch.allowedTools, 500),
          content: patch.content === undefined ? skill.content : clip(patch.content, 8000)
        }
      })
    }),
    storage
  )
}

export function removeHarnessSkill(id: HarnessAssembleId, skillId: string, storage?: EvolutionStorage): void {
  update(id, (current) => ({ ...current, skills: current.skills.filter((skill) => skill.id !== skillId) }), storage)
}

export function subscribeHarnessEvolution(listener: () => void): () => void {
  listeners.add(listener)
  const onStorage = (event: StorageEvent) => {
    if (event.key !== HARNESS_EVOLUTION_KEY) return
    snapshotRaw = event.newValue
    snapshot = parseBook(event.newValue)
    listener()
  }
  window.addEventListener('storage', onStorage)
  return () => {
    listeners.delete(listener)
    window.removeEventListener('storage', onStorage)
  }
}

function toolText(tool: HarnessToolEdition): string {
  const custom = tool.description.trim()
  if (custom) return custom
  return admittedTool(tool.name)?.description ?? ''
}

function skillKey(skill: HarnessSkillEdition): string {
  return skill.name.trim() || skill.id
}

function skillText(skill: HarnessSkillEdition): string {
  return `${skill.description}\n${skill.allowedTools}\n${skill.content}`
}

/** Tool and skill create, delete, and text deltas. Component membership stays on `diffComponents`. */
export function diffEvolution(
  base: HarnessEvolution,
  next: HarnessEvolution,
  scenarios: { base: string; next: string }
): EvolutionDiffRow[] {
  const rows: EvolutionDiffRow[] = []
  if (scenarios.base.trim() !== scenarios.next.trim())
    rows.push({ asset: 'scenario', kind: 'optimized', name: 'scenario' })

  const baseTools = new Map(base.tools.map((tool) => [tool.name, tool]))
  const nextTools = new Map(next.tools.map((tool) => [tool.name, tool]))
  for (const tool of ADMITTED_TOOLS) {
    const before = baseTools.get(tool.name)
    const after = nextTools.get(tool.name)
    const beforeOn = before?.included ?? false
    const afterOn = after?.included ?? false
    if (beforeOn && !afterOn) rows.push({ asset: 'tool', kind: 'removed', name: tool.name })
    else if (!beforeOn && afterOn) rows.push({ asset: 'tool', kind: 'added', name: tool.name })
    else if (beforeOn && afterOn && before && after && toolText(before) !== toolText(after)) {
      rows.push({ asset: 'tool', kind: 'optimized', name: tool.name })
    }
  }

  const baseSkills = new Map(base.skills.map((skill) => [skillKey(skill), skill]))
  const nextSkills = new Map(next.skills.map((skill) => [skillKey(skill), skill]))
  for (const [key, skill] of baseSkills) {
    if (!nextSkills.has(key)) rows.push({ asset: 'skill', kind: 'removed', name: skill.name.trim() || key })
  }
  for (const [key, skill] of nextSkills) {
    const previous = baseSkills.get(key)
    if (!previous) rows.push({ asset: 'skill', kind: 'added', name: skill.name.trim() || key })
    else if (skillText(previous) !== skillText(skill))
      rows.push({ asset: 'skill', kind: 'optimized', name: skill.name.trim() || key })
  }
  return rows
}
