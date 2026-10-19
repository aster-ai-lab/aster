import {
  Button,
  Dialog,
  DialogContent,
  DialogDescription,
  DialogFooter,
  DialogHeader,
  DialogTitle,
  Input,
  SearchSelect,
  SegmentedControl,
  Switch,
  Tooltip
} from '@a3s-lab/ui'
import i18next from 'i18next'
import {
  Bot,
  ChartCandlestick,
  Coffee,
  Database,
  Files,
  Folder,
  GitBranch,
  LayoutPanelLeft,
  Minus,
  Plus,
  Sparkles,
  Terminal,
  type LucideIcon
} from 'lucide-react'
import { use, useEffect, useId, useMemo, useRef, useState, useSyncExternalStore } from 'react'
import { useTranslation } from 'react-i18next'

import {
  aixWorkbenchPackageIdentity,
  findAixWindow,
  openAixAppletHome,
  readAixCatalogue,
  readAppletHomeHoldsWork,
  stageAppletHomeSeed,
  subscribeAixCatalogue,
  waitAppletHomeDelivery
} from '@renderer/services/plugins'
import type { AixWorkbenchCatalogue } from '@shared/types/aixWorkbench'

import { codeBranchName, isCodeBranchName, resolveCodeBranch, type CodePlacement } from './codeMode'
import { WorkbenchHomeActiveContext } from './workbenchComposerHost'

export const HOST_CHAT_MODE_ID = 'chat'
export const HOST_CHAT_MODE_LABEL = '日常办公'
export const HOST_CODE_MODE_ID = 'code'
export const HOST_CODE_MODE_LABEL = '代码开发'
export const HOME_DRAFT_KEPT_NOTICE = 'plugins.composer.draft_kept'

export type HomeComposerControl = {
  id: string
  kind: 'number' | 'select' | 'toggle' | 'text'
  label: string
  min?: number
  max?: number
  options?: readonly string[]
  optionLabels?: readonly string[]
  initial?: string | boolean
}

export type HomeComposerModeTask = {
  id: string
  title: string
  prompt: string
}

export type HomeComposerMode = {
  key: string
  identity: string
  id: string
  title: string
  prompt?: string
  order: number
  agent: string
  editor?: string
  keptNotice?: 'draft' | 'editor'
  /** Homepage send starts a task conversation and leaves the editor window closed. */
  task?: true
  taskPrompt?: string
  packageId: string
  slug: string
  icon?: string
  controls: readonly HomeComposerControl[]
  tasks: readonly HomeComposerModeTask[]
}

export type HomeComposerDeck = {
  title: string
  topic: string
  lines: readonly string[]
}

export type HomeComposerSeed = { content: string } & Record<string, unknown>

export type HomeComposerSendEffect = {
  kind: 'seed' | 'chat' | 'kept' | 'refused'
  modeId: string | null
  permissionMode: string
  research: false
  autoReview: false
  homeTurnDirectives: null
  opened: { slug: string; view: string } | null
  seed: HomeComposerSeed | null
}

type SelectionState = {
  modeId: string
  codePlacement: CodePlacement
  codeSuffix: string
  codeBranchDraft: string
  codeExistingName: string
  codeBase: string
  values: Record<string, Record<string, string | boolean>>
  notice: string | null
  fieldError: string | null
  effect: HomeComposerSendEffect | null
}

function freshCodeSuffix(): string {
  const bytes = new Uint8Array(2)
  if (globalThis.crypto?.getRandomValues) globalThis.crypto.getRandomValues(bytes)
  else bytes.set([(Date.now() % 256) & 255, ((Date.now() >> 8) % 256) & 255])
  return [...bytes].map((value) => value.toString(16).padStart(2, '0')).join('')
}

const selectionListeners = new Set<() => void>()
let selection: SelectionState = {
  modeId: HOST_CHAT_MODE_ID,
  codePlacement: 'checkout',
  codeSuffix: freshCodeSuffix(),
  codeBranchDraft: '',
  codeExistingName: '',
  codeBase: '',
  values: {},
  notice: null,
  fieldError: null,
  effect: null
}

let codeTopic = ''
const codeTopicListeners = new Set<() => void>()

export function noteHomeComposerTopic(text: string) {
  if (codeTopic === text) return
  codeTopic = text
  for (const listener of codeTopicListeners) listener()
}

function subscribeCodeTopic(listener: () => void) {
  codeTopicListeners.add(listener)
  return () => codeTopicListeners.delete(listener)
}

function readCodeTopic() {
  return codeTopic
}

export type HomeComposerSendInput = {
  surface: 'fallback' | 'live'
  text: string
  permissionMode: string
}

export type HomeComposerCodePlan = {
  placement: CodePlacement
  branch: string | null
  suffix: string
  base: string | null
  existing: boolean
}

export type HomeComposerSendOutcome = {
  handled: boolean
  cleared: boolean
  text?: string
  deck?: HomeComposerDeck
  code?: HomeComposerCodePlan
}

type PendingHomeSend = {
  input: HomeComposerSendInput
  resolve: (outcome: HomeComposerSendOutcome) => void
}

let pendingSend: PendingHomeSend | null = null
let confirmingSend = false
const pendingListeners = new Set<() => void>()

function emitSelection() {
  for (const listener of selectionListeners) listener()
}

function writeSelection(next: SelectionState) {
  selection = next
  emitSelection()
}

export function resetHomeComposerModeForTests() {
  const pending = pendingSend
  pendingSend = null
  confirmingSend = false
  writeSelection({
    modeId: HOST_CHAT_MODE_ID,
    codePlacement: 'checkout',
    codeSuffix: freshCodeSuffix(),
    codeBranchDraft: '',
    codeExistingName: '',
    codeBase: '',
    values: {},
    notice: null,
    fieldError: null,
    effect: null
  })
  noteHomeComposerTopic('')
  emitPending()
  pending?.resolve({ handled: true, cleared: false })
}

export function readHomeComposerSendEffect(): HomeComposerSendEffect | null {
  return selection.effect
}

function subscribeSelection(listener: () => void) {
  selectionListeners.add(listener)
  return () => selectionListeners.delete(listener)
}

function readSelection() {
  return selection
}

function asControl(value: unknown): HomeComposerControl | null {
  if (!value || typeof value !== 'object') return null
  const control = value as HomeComposerControl
  if (typeof control.id !== 'string' || typeof control.label !== 'string') return null
  if (control.kind !== 'number' && control.kind !== 'select' && control.kind !== 'toggle' && control.kind !== 'text') {
    return null
  }
  return control
}

function asTask(value: unknown): HomeComposerModeTask | null {
  if (!value || typeof value !== 'object') return null
  const task = value as { id?: unknown; title?: unknown; prompt?: unknown }
  if (typeof task.id !== 'string' || task.id.length === 0) return null
  if (typeof task.title !== 'string' || task.title.trim().length === 0) return null
  if (typeof task.prompt !== 'string' || task.prompt.trim().length === 0) return null
  return { id: task.id, title: task.title, prompt: task.prompt }
}

export function admittedHomeComposerModes(catalogue: AixWorkbenchCatalogue): HomeComposerMode[] {
  const modes: HomeComposerMode[] = []
  for (const item of catalogue.packages) {
    const declared = (item.composerModes ?? []) as readonly unknown[]
    for (const entry of declared) {
      if (!entry || typeof entry !== 'object') continue
      const mode = entry as {
        id?: string
        title?: string
        prompt?: string
        order?: number
        scope?: string
        agent?: string
        editor?: string
        keptNotice?: string
        icon?: string
        controls?: readonly unknown[]
        tasks?: readonly unknown[]
        task?: unknown
        taskPrompt?: unknown
      }
      if (mode.scope !== 'workspace' || typeof mode.id !== 'string' || typeof mode.title !== 'string') continue
      if (typeof mode.agent !== 'string') continue
      let editor: string | undefined
      if (typeof mode.editor === 'string') {
        if (!findAixWindow(catalogue, item.slug, mode.editor)) continue
        editor = mode.editor
      }
      const controls = (mode.controls ?? [])
        .map(asControl)
        .filter((control): control is HomeComposerControl => control !== null)
      const tasks = (mode.tasks ?? [])
        .map(asTask)
        .filter((task): task is HomeComposerModeTask => task !== null)
        .slice(0, 8)
      const keptNotice = mode.keptNotice === 'draft' || mode.keptNotice === 'editor' ? mode.keptNotice : undefined
      const taskPrompt =
        typeof mode.taskPrompt === 'string' && mode.taskPrompt.trim().length > 0 ? mode.taskPrompt : undefined
      modes.push({
        key: `${item.packageId}/${mode.id}`,
        identity: aixWorkbenchPackageIdentity(item),
        id: mode.id,
        title: mode.title,
        prompt: typeof mode.prompt === 'string' ? mode.prompt : undefined,
        order: typeof mode.order === 'number' ? mode.order : 100,
        agent: mode.agent,
        ...(editor ? { editor } : {}),
        ...(keptNotice ? { keptNotice } : {}),
        ...(mode.task === true ? { task: true as const } : {}),
        ...(taskPrompt ? { taskPrompt } : {}),
        packageId: item.packageId,
        slug: item.slug,
        ...(typeof mode.icon === 'string' ? { icon: mode.icon } : {}),
        controls,
        tasks
      })
    }
  }
  modes.sort((left, right) => left.order - right.order || left.id.localeCompare(right.id))
  return modes.slice(0, 32)
}

function initialControlValue(control: HomeComposerControl): string | boolean {
  if (control.kind === 'toggle') return control.initial !== false
  if (control.kind === 'number') return ''
  if (typeof control.initial === 'string') return control.initial
  return control.options?.[0] ?? ''
}

function valuesFor(mode: HomeComposerMode, current: Record<string, string | boolean> | undefined) {
  const next: Record<string, string | boolean> = { ...current }
  for (const control of mode.controls) {
    if (next[control.id] === undefined) next[control.id] = initialControlValue(control)
  }
  return next
}

export function selectCodePlacement(placement: CodePlacement) {
  if (placement !== 'checkout' && placement !== 'worktree') return
  writeSelection({ ...selection, codePlacement: placement, notice: null, fieldError: null })
}

function chooseCodeBranch(next: { draft?: string; existingName?: string; base?: string }) {
  const codeBranchDraft = next.draft ?? selection.codeBranchDraft
  const codeExistingName = next.existingName ?? selection.codeExistingName
  const codeBase = next.base ?? selection.codeBase
  if (
    selection.codeBranchDraft === codeBranchDraft &&
    selection.codeExistingName === codeExistingName &&
    selection.codeBase === codeBase &&
    !selection.fieldError
  ) {
    return
  }
  writeSelection({
    ...selection,
    codeBranchDraft,
    codeExistingName,
    codeBase,
    notice: null,
    fieldError: null
  })
}

export function selectHomeComposerMode(modeId: string) {
  const modes = admittedHomeComposerModes(readAixCatalogue())
  const matches = modes.filter((entry) => entry.key === modeId || entry.id === modeId)
  const mode = matches.length === 1 ? matches[0] : undefined
  if (!mode && modeId === HOST_CODE_MODE_ID) {
    const values = { ...selection.values }
    const leaving = modes.find((entry) => entry.key === selection.modeId)
    if (leaving) delete values[leaving.key]
    writeSelection({ ...selection, modeId: HOST_CODE_MODE_ID, values, notice: null, fieldError: null })
    return
  }
  const nextId = mode?.key ?? HOST_CHAT_MODE_ID
  const values = { ...selection.values }
  if (!mode) {
    const leaving = modes.find((entry) => entry.key === selection.modeId)
    if (leaving) delete values[leaving.key]
  }
  writeSelection({
    ...selection,
    modeId: nextId,
    values: mode ? { ...values, [mode.key]: valuesFor(mode, values[mode.key]) } : values,
    notice: null,
    fieldError: null
  })
}

export function setHomeComposerControl(modeId: string, controlId: string, value: string | boolean) {
  const current = selection.values[modeId] ?? {}
  writeSelection({
    ...selection,
    values: { ...selection.values, [modeId]: { ...current, [controlId]: value } },
    fieldError: null
  })
}

function resolvedMode(): HomeComposerMode | null {
  const modes = admittedHomeComposerModes(readAixCatalogue())
  return modes.find((mode) => mode.key === selection.modeId) ?? null
}

function keptNotice(mode: HomeComposerMode): string {
  return i18next.t(mode.keptNotice === 'draft' ? HOME_DRAFT_KEPT_NOTICE : 'plugins.composer.editor_draft_kept')
}

function quietEffect(
  kind: HomeComposerSendEffect['kind'],
  permissionMode: string,
  modeId: string | null,
  extra: Partial<HomeComposerSendEffect> = {}
): HomeComposerSendEffect {
  return {
    kind,
    modeId,
    permissionMode,
    research: false,
    autoReview: false,
    homeTurnDirectives: null,
    opened: null,
    seed: null,
    ...extra
  }
}

function controlSeed(mode: HomeComposerMode, topic: string): { seed: HomeComposerSeed } | { error: string } {
  const values = valuesFor(mode, selection.values[mode.key])
  const seed: HomeComposerSeed = { content: topic }
  for (const control of mode.controls) {
    const value = values[control.id]
    if (control.kind === 'number') {
      const text = typeof value === 'string' ? value.trim() : ''
      if (!text) {
        seed[control.id] = null
        continue
      }
      const parsed = Number(text)
      const min = control.min ?? 1
      const max = control.max ?? 20
      if (!Number.isInteger(parsed) || parsed < min || parsed > max) {
        return { error: i18next.t('plugins.composer.integer', { label: control.label, min, max }) }
      }
      seed[control.id] = parsed
      continue
    }
    if (control.kind === 'select') {
      const selected = typeof value === 'string' ? value : ''
      if (!control.options?.includes(selected))
        return { error: i18next.t('plugins.composer.select', { label: control.label }) }
      seed[control.id] = selected
      continue
    }
    if (control.kind === 'toggle') {
      seed[control.id] = value === true
      continue
    }
    const text = typeof value === 'string' ? value : ''
    const max = control.max ?? 4000
    if ([...text].length > max)
      return { error: i18next.t('plugins.composer.max_length', { label: control.label, max }) }
    seed[control.id] = text
  }
  return { seed }
}

function shownControl(control: HomeComposerControl, value: unknown): string {
  if (control.kind === 'number') {
    return value === null || value === '' || value === undefined ? i18next.t('plugins.composer.auto') : String(value)
  }
  if (control.kind === 'toggle') {
    return value === true ? i18next.t('plugins.composer.on') : i18next.t('plugins.composer.off')
  }
  if (control.kind === 'select' && typeof value === 'string') {
    const index = control.options?.indexOf(value) ?? -1
    return (index >= 0 ? control.optionLabels?.[index] : undefined) || value
  }
  return typeof value === 'string' ? value : ''
}

function controlLines(mode: HomeComposerMode, seed: HomeComposerSeed): string[] {
  const lines: string[] = []
  for (const control of mode.controls) {
    const value = seed[control.id]
    if (control.kind === 'text' && (typeof value !== 'string' || value.trim().length === 0)) continue
    const shown = shownControl(control, value)
    if (!shown) continue
    lines.push(`${control.label}：${shown}`)
  }
  return lines
}

function composerTaskText(mode: HomeComposerMode, seed: HomeComposerSeed): string {
  const lines = [seed.content, ...controlLines(mode, seed)]
  if (mode.taskPrompt) lines.push(mode.taskPrompt)
  return lines.join('\n')
}

function taskDeck(mode: HomeComposerMode, seed: HomeComposerSeed): HomeComposerDeck {
  return { title: mode.title, topic: seed.content, lines: controlLines(mode, seed) }
}

export async function submitHomeComposerDraft(input: HomeComposerSendInput): Promise<HomeComposerSendOutcome> {
  const mode = resolvedMode()
  const start = selection
  const topic = input.text.trim()
  if (!mode && selection.modeId === HOST_CODE_MODE_ID) {
    if (!topic) return { handled: true, cleared: false }
    const placement = selection.codePlacement
    const existingName = placement === 'worktree' ? selection.codeExistingName.trim() : ''
    const existing = existingName.length > 0
    const branch = !existing
      ? placement === 'worktree'
        ? resolveCodeBranch(topic, selection.codeSuffix, selection.codeBranchDraft)
        : null
      : isCodeBranchName(existingName)
        ? existingName
        : null
    if (placement === 'worktree' && !branch) {
      writeSelection({
        ...selection,
        notice: null,
        fieldError: i18next.t('workbench.composer.code.branch_invalid', {
          defaultValue: '分支名只能使用字母、数字、点、下划线、斜线和连字符'
        })
      })
      return { handled: true, cleared: false }
    }
    const base = placement === 'worktree' && !existing && selection.codeBase.trim() ? selection.codeBase.trim() : null
    writeSelection({
      ...selection,
      notice: null,
      fieldError: null,
      effect: quietEffect('chat', input.permissionMode, HOST_CODE_MODE_ID)
    })
    return {
      handled: input.surface !== 'live',
      cleared: false,
      code: { placement, branch, suffix: selection.codeSuffix, base, existing }
    }
  }
  if (!mode) {
    if (!topic) {
      if (input.surface === 'live') return { handled: false, cleared: false }
      return { handled: true, cleared: false }
    }
    writeSelection({
      ...selection,
      modeId: HOST_CHAT_MODE_ID,
      notice: null,
      fieldError: null,
      effect: quietEffect('chat', input.permissionMode, null)
    })
    if (input.surface === 'live') return { handled: false, cleared: false }
    return { handled: true, cleared: false, text: topic }
  }
  if (!topic) {
    writeSelection({
      ...selection,
      fieldError: null,
      effect: quietEffect('refused', input.permissionMode, mode.id)
    })
    return { handled: true, cleared: false }
  }
  // The editor id stays so a composer-only window is not offered as 打开. A task mode still starts a conversation.
  if (!mode.editor || mode.task) {
    const built = controlSeed(mode, topic)
    if ('error' in built) {
      writeSelection({
        ...selection,
        notice: null,
        fieldError: built.error,
        effect: quietEffect('refused', input.permissionMode, mode.id)
      })
      return { handled: true, cleared: false }
    }
    writeSelection({
      ...selection,
      notice: null,
      fieldError: null,
      effect: quietEffect('chat', input.permissionMode, mode.id, { seed: built.seed })
    })
    const text = composerTaskText(mode, built.seed)
    const deck = mode.task ? taskDeck(mode, built.seed) : undefined
    if (mode.task && mode.editor) stageAppletHomeSeed(mode.slug, mode.editor, built.seed)
    if (input.surface === 'live') {
      return mode.task ? { handled: false, cleared: false, text, deck } : { handled: false, cleared: false, text }
    }
    if (mode.task && deck) return { handled: true, cleared: true, text, deck }
    return { handled: true, cleared: false }
  }
  const editor = mode.editor
  const holdsWork = await readAppletHomeHoldsWork(mode.slug, editor)
  const current = () =>
    selection.modeId === start.modeId && selection.values === start.values && resolvedMode()?.identity === mode.identity
  if (!current()) return { handled: true, cleared: false }
  if (holdsWork) {
    writeSelection({
      ...selection,
      notice: keptNotice(mode),
      fieldError: null,
      effect: quietEffect('kept', input.permissionMode, mode.id)
    })
    return { handled: true, cleared: false }
  }
  const built = controlSeed(mode, topic)
  if ('error' in built) {
    writeSelection({
      ...selection,
      notice: null,
      fieldError: built.error,
      effect: quietEffect('refused', input.permissionMode, mode.id)
    })
    return { handled: true, cleared: false }
  }
  const opened = await openAixAppletHome(mode.slug, editor, mode.title, built.seed)
  if (!opened.opened || opened.delivery <= 0) {
    writeSelection({
      ...selection,
      notice: null,
      fieldError: null,
      effect: quietEffect('refused', input.permissionMode, mode.id)
    })
    return { handled: true, cleared: false }
  }
  const receipt = await waitAppletHomeDelivery(mode.slug, editor, opened.delivery)
  if (!current()) return { handled: true, cleared: false }
  if (receipt?.applied !== true) {
    writeSelection({
      ...selection,
      notice: receipt?.holdsWork ? keptNotice(mode) : null,
      fieldError: null,
      effect: quietEffect(receipt?.holdsWork ? 'kept' : 'refused', input.permissionMode, mode.id, {
        opened: { slug: mode.slug, view: editor }
      })
    })
    return { handled: true, cleared: false }
  }
  writeSelection({
    ...selection,
    notice: null,
    fieldError: null,
    effect: quietEffect('seed', input.permissionMode, mode.id, {
      opened: { slug: mode.slug, view: editor },
      seed: built.seed
    })
  })
  return { handled: true, cleared: true }
}

function emitPending() {
  for (const listener of pendingListeners) listener()
}

function subscribePending(listener: () => void) {
  pendingListeners.add(listener)
  return () => pendingListeners.delete(listener)
}

function readPending() {
  return pendingSend
}

function readConfirmingSend() {
  return confirmingSend
}

/** Opens the mode's control form when that mode declares controls. Otherwise commits immediately. */
export function requestHomeComposerSend(input: HomeComposerSendInput): Promise<HomeComposerSendOutcome> {
  const mode = resolvedMode()
  const topic = input.text.trim()
  if (!mode || mode.controls.length === 0 || topic.length === 0) return submitHomeComposerDraft(input)
  if (pendingSend) return Promise.resolve({ handled: true, cleared: false })
  if (selection.fieldError || selection.notice) {
    writeSelection({ ...selection, fieldError: null, notice: null })
  }
  return new Promise((resolve) => {
    pendingSend = { input, resolve }
    emitPending()
  })
}

function cancelHomeComposerSend() {
  if (confirmingSend) return
  const pending = pendingSend
  if (!pending) return
  pendingSend = null
  if (selection.fieldError) writeSelection({ ...selection, fieldError: null })
  emitPending()
  pending.resolve({ handled: true, cleared: false })
}

async function confirmHomeComposerSend() {
  const pending = pendingSend
  if (!pending || confirmingSend) return
  confirmingSend = true
  emitPending()
  try {
    const outcome = await submitHomeComposerDraft(pending.input)
    if (pendingSend !== pending) return
    if (selection.fieldError) return
    pendingSend = null
    emitPending()
    pending.resolve(outcome)
  } catch {
    if (pendingSend === pending) {
      writeSelection({ ...selection, fieldError: i18next.t('plugins.composer.send_form.failed') })
    }
  } finally {
    if (!confirmingSend) return
    confirmingSend = false
    emitPending()
  }
}

export function useHomeComposerModeState() {
  const catalogue = useSyncExternalStore(subscribeAixCatalogue, readAixCatalogue, readAixCatalogue)
  const current = useSyncExternalStore(subscribeSelection, readSelection, readSelection)
  const modes = useMemo(() => admittedHomeComposerModes(catalogue), [catalogue])
  const selected = modes.find((mode) => mode.key === current.modeId) ?? null
  const hostCode = !selected && current.modeId === HOST_CODE_MODE_ID
  const selectedId = selected?.key ?? (hostCode ? HOST_CODE_MODE_ID : HOST_CHAT_MODE_ID)
  const values = selected ? valuesFor(selected, current.values[selected.key]) : {}
  return {
    modes,
    selected,
    selectedId,
    hostCode,
    codePlacement: current.codePlacement,
    codeSuffix: current.codeSuffix,
    codeBranchDraft: current.codeBranchDraft,
    codeExistingName: current.codeExistingName,
    codeBase: current.codeBase,
    values,
    notice: current.notice,
    fieldError: current.fieldError,
    prompt: selected?.prompt ?? null
  }
}

const MODE_ICONS: Record<string, LucideIcon> = {
  bot: Bot,
  database: Database,
  files: Files,
  terminal: Terminal,
  panels: LayoutPanelLeft,
  'chart-candlestick': ChartCandlestick
}

function modeIcon(name: string | undefined, hostChat: boolean): LucideIcon {
  if (hostChat) return Coffee
  if (name && MODE_ICONS[name]) return MODE_ICONS[name]
  return Sparkles
}

function ModeGlyph({ icon, selected }: { icon: LucideIcon; selected: boolean }) {
  const Glyph = icon
  return <Glyph aria-hidden size={16} strokeWidth={1.7} className={selected ? 'text-white' : 'text-current'} />
}

function radioClass(selected: boolean) {
  return selected
    ? 'inline-flex h-8 shrink-0 items-center gap-1.5 whitespace-nowrap rounded-full bg-[#171717] px-3.5 text-[13px] font-medium text-white shadow-[0_1px_2px_rgba(0,0,0,0.12)] transition-colors duration-150'
    : 'inline-flex h-8 shrink-0 items-center gap-1.5 whitespace-nowrap rounded-full px-3.5 text-[13px] text-[#3f3f3f] transition-colors duration-150 hover:bg-[#ececec] hover:text-[#171717]'
}

function placementButtonClass(selected: boolean) {
  const base =
    'inline-flex h-8 w-8 shrink-0 items-center justify-center rounded-[8px] outline-none focus-visible:ring-2 focus-visible:ring-black/10'
  return selected
    ? `${base} bg-[#f2f3f5] text-[#171717]`
    : `${base} text-[#3f3f3f] hover:bg-[#f7f7f7] hover:text-[#171717]`
}

function useHorizontalFade() {
  const [node, setNode] = useState<HTMLDivElement | null>(null)
  const [fade, setFade] = useState(false)
  useEffect(() => {
    if (!node) return
    const update = () => setFade(node.scrollWidth > node.clientWidth + 1)
    update()
    node.addEventListener('scroll', update)
    const observer = typeof ResizeObserver === 'undefined' ? null : new ResizeObserver(update)
    observer?.observe(node)
    return () => {
      node.removeEventListener('scroll', update)
      observer?.disconnect()
    }
  }, [node])
  return { fade, ref: setNode }
}

function controlRaw(value: string | boolean | undefined) {
  return typeof value === 'string' ? value : ''
}

function subscribeComposerLanguage(listener: () => void) {
  i18next.on('languageChanged', listener)
  return () => {
    i18next.off('languageChanged', listener)
  }
}

function composerLanguage() {
  return i18next.resolvedLanguage ?? i18next.language
}

function composerWord(
  key:
    | 'plugins.composer.auto'
    | 'plugins.composer.on'
    | 'plugins.composer.off'
    | 'plugins.composer.decrease'
    | 'plugins.composer.increase'
    | 'plugins.composer.no_match'
    | 'plugins.composer.search'
    | 'plugins.composer.select'
    | 'plugins.composer.send_form.cancel'
    | 'plugins.composer.send_form.close'
    | 'plugins.composer.send_form.confirm'
    | 'plugins.composer.send_form.description',
  language: string,
  values?: Record<string, string>
) {
  return i18next.t(key, { lng: language, ...values })
}

const SEGMENTED_SELECT_MAX = 3

function segmentedSelect(control: HomeComposerControl) {
  const count = control.options?.length ?? 0
  return control.kind === 'select' && count >= 2 && count <= SEGMENTED_SELECT_MAX
}

function keepSelectPanel(event: { target: EventTarget | null; preventDefault: () => void }) {
  if (event.target instanceof Element && event.target.closest('.ui-search-select__panel')) event.preventDefault()
}

function selectOptions(control: HomeComposerControl) {
  return (control.options ?? []).map((option, index) => ({
    value: option,
    label: control.optionLabels?.[index] || option
  }))
}

const NEW_BRANCH_CHOICE = '__new__'

type CodeBranchList = { head?: unknown; branches?: unknown }

function readCodeBranchList(projectPath: string | null): Promise<CodeBranchList | null> {
  if (!projectPath) return Promise.resolve(null)
  const invoke = (
    globalThis as {
      __TAURI_INTERNALS__?: { invoke?: (command: string, args: Record<string, unknown>) => Promise<unknown> }
    }
  ).__TAURI_INTERNALS__?.invoke
  if (!invoke) return Promise.resolve(null)
  return invoke('code_branches', { cwd: projectPath }).then((result) =>
    result && typeof result === 'object' ? (result as CodeBranchList) : null
  )
}

function branchListLabel(head: string, current: string) {
  return head && head !== 'HEAD' ? `${current}（${head}）` : current
}

export function CodeWorkspacePlacement({ projectPath = null }: { projectPath?: string | null }) {
  const { t } = useTranslation()
  const { hostCode, codePlacement, codeSuffix, codeBranchDraft, codeExistingName, codeBase } =
    useHomeComposerModeState()
  const topic = useSyncExternalStore(subscribeCodeTopic, readCodeTopic, readCodeTopic)
  useSyncExternalStore(subscribeComposerLanguage, composerLanguage, composerLanguage)
  const [listed, setListed] = useState<{ head: string; branches: string[] }>({ head: '', branches: [] })
  const [createOpen, setCreateOpen] = useState(false)
  const [createName, setCreateName] = useState('')
  const [createBase, setCreateBase] = useState('')
  const [createError, setCreateError] = useState<string | null>(null)
  const nameId = useId()
  useEffect(() => {
    if (!hostCode || codePlacement !== 'worktree') setCreateOpen(false)
  }, [codePlacement, hostCode])
  useEffect(() => {
    if (!hostCode || codePlacement !== 'worktree' || !projectPath) return
    let cancel = false
    void readCodeBranchList(projectPath)
      .then((result) => {
        if (cancel || !result) return
        const head = typeof result.head === 'string' ? result.head : ''
        const branches = Array.isArray(result.branches)
          ? result.branches.filter((name): name is string => typeof name === 'string')
          : []
        setListed({ head, branches })
      })
      .catch(() => {
        if (!cancel) setListed({ head: '', branches: [] })
      })
    return () => {
      cancel = true
    }
  }, [codePlacement, hostCode, projectPath])
  if (!hostCode) return null
  const generated = codeBranchName(topic, codeSuffix)
  const newName = codeBranchDraft.trim() ? codeBranchDraft : generated
  const usingExisting = codeExistingName.trim().length > 0
  const branchShown = usingExisting ? codeExistingName : newName
  const checkout = codePlacement === 'checkout'
  const currentLabel = branchListLabel(listed.head, t('workbench.composer.code.base_current', '当前分支'))
  const known = listed.branches.filter((name) => name !== 'HEAD' && name !== newName)
  const branchOptions = [
    { value: '', label: newName },
    ...known.map((name) => ({ value: name, label: name })),
    { value: NEW_BRANCH_CHOICE, label: t('workbench.composer.code.branch_create', '新建分支') }
  ]
  const baseOptions = [
    { value: '', label: currentLabel },
    ...listed.branches
      .filter((name) => name !== listed.head && name !== 'HEAD')
      .map((name) => ({ value: name, label: name }))
  ]
  const openCreate = () => {
    setCreateName(newName)
    setCreateBase(usingExisting ? '' : codeBase)
    setCreateError(null)
    setCreateOpen(true)
  }
  const confirmCreate = () => {
    const name = createName.trim() || generated
    if (!isCodeBranchName(name)) {
      setCreateError(t('workbench.composer.code.branch_invalid', '分支名只能使用字母、数字、点、下划线、斜线和连字符'))
      return
    }
    if (listed.branches.includes(name)) {
      setCreateError(t('workbench.composer.code.branch_exists', '这个分支已经存在'))
      return
    }
    const base = createBase.trim()
    if (base && !isCodeBranchName(base)) {
      setCreateError(t('workbench.composer.code.branch_invalid', '分支名只能使用字母、数字、点、下划线、斜线和连字符'))
      return
    }
    chooseCodeBranch({ draft: name === generated ? '' : name, existingName: '', base })
    setCreateOpen(false)
  }
  return (
    <>
      <div
        data-ui="workbench.composer.code.placement"
        role="radiogroup"
        aria-label={t('workbench.composer.code.placement', '工作区')}
        className="flex min-w-0 items-center gap-0.5">
        <Tooltip
          content={t('workbench.composer.code.checkout', '当前目录')}
          delay={400}
          placement="top"
          sideOffset={6}
          asChild>
          <button
            type="button"
            role="radio"
            aria-checked={checkout}
            aria-label={t('workbench.composer.code.checkout', '当前目录')}
            className={placementButtonClass(checkout)}
            onClick={() => selectCodePlacement('checkout')}>
            <Folder aria-hidden size={16} strokeWidth={1.7} />
          </button>
        </Tooltip>
        <Tooltip
          content={t('workbench.composer.code.worktree', '新工作树')}
          delay={400}
          placement="top"
          sideOffset={6}
          asChild>
          <button
            type="button"
            role="radio"
            aria-checked={!checkout}
            aria-label={t('workbench.composer.code.worktree', '新工作树')}
            className={placementButtonClass(!checkout)}
            onClick={() => selectCodePlacement('worktree')}>
            <GitBranch aria-hidden size={16} strokeWidth={1.7} />
          </button>
        </Tooltip>
        {checkout ? null : (
          <Tooltip
            content={t('workbench.composer.code.branch_pick', '分支')}
            delay={400}
            placement="top"
            sideOffset={6}
            asChild>
            <span
              data-code-branch={branchShown}
              className="inline-flex max-w-[9.5rem] min-w-0 [&_.ui-search-select]:!max-w-full [&_.ui-search-select]:!min-w-0 [&_.ui-search-select__trigger]:!min-w-0">
              <SearchSelect
                aria-label={t('workbench.composer.code.branch_pick', '分支')}
                size="sm"
                allowClear={false}
                searchable={branchOptions.length > 8}
                options={branchOptions}
                value={usingExisting ? codeExistingName : ''}
                onChange={(value) => {
                  if (value === NEW_BRANCH_CHOICE) {
                    openCreate()
                    return
                  }
                  if (!value) {
                    chooseCodeBranch({ existingName: '' })
                    return
                  }
                  chooseCodeBranch({ existingName: value, base: '' })
                }}
              />
            </span>
          </Tooltip>
        )}
      </div>
      <Dialog open={createOpen && !checkout} onOpenChange={setCreateOpen}>
        <DialogContent
          motion="fade-scale"
          closeLabel={t('common.close', '关闭')}
          overlayClassName="z-[220]"
          onFocusOutside={keepSelectPanel}
          onInteractOutside={keepSelectPanel}
          onPointerDownOutside={keepSelectPanel}
          className="z-[220] w-[320px] sm:max-w-[320px]">
          <DialogHeader className="gap-1 text-left">
            <DialogTitle className="text-[15px] leading-[1.4] font-medium text-[#171717]">
              {t('workbench.composer.code.branch_create', '新建分支')}
            </DialogTitle>
            <DialogDescription className="text-[13px] leading-[1.5] text-[#8a8a8a]">
              {t('workbench.composer.code.branch_create_hint', '名称留空就用自动名称。起点留空就用当前分支。')}
            </DialogDescription>
          </DialogHeader>
          <form
            className="flex flex-col gap-4"
            onSubmit={(event) => {
              event.preventDefault()
              confirmCreate()
            }}>
            <div className="flex flex-col gap-1.5">
              <label htmlFor={nameId} className="text-[13px] font-medium text-[#171717]">
                {t('workbench.composer.code.branch', '新分支')}
              </label>
              <Input
                id={nameId}
                value={createName}
                autoFocus
                autoComplete="off"
                spellCheck={false}
                maxLength={100}
                aria-invalid={createError ? true : undefined}
                onChange={(event) => {
                  setCreateName(event.target.value)
                  setCreateError(null)
                }}
              />
              {createError ? (
                <p role="alert" className="text-[12px] leading-[1.4] text-[#b42318]">
                  {createError}
                </p>
              ) : null}
            </div>
            {projectPath ? (
              <div className="flex flex-col gap-1.5">
                <span className="text-[13px] font-medium text-[#171717]">
                  {t('workbench.composer.code.base', '起点')}
                </span>
                <SearchSelect
                  aria-label={t('workbench.composer.code.base', '起点')}
                  allowClear={false}
                  searchable={baseOptions.length > 8}
                  options={baseOptions}
                  value={createBase}
                  onChange={setCreateBase}
                />
              </div>
            ) : null}
            <DialogFooter className="flex-row justify-end gap-2">
              <Button type="button" variant="outline" size="md" onClick={() => setCreateOpen(false)}>
                {t('common.cancel', '取消')}
              </Button>
              <Button type="submit" variant="emphasis" size="md" onClick={confirmCreate}>
                {t('workbench.composer.code.branch_create_confirm', '创建')}
              </Button>
            </DialogFooter>
          </form>
        </DialogContent>
      </Dialog>
    </>
  )
}

export function HomeComposerModeTrack({ active = true }: { active?: boolean }) {
  const homeActive = use(WorkbenchHomeActiveContext)
  const { modes, selectedId, hostCode } = useHomeComposerModeState()
  const { fade, ref } = useHorizontalFade()
  if (!active && !homeActive) return null
  const chatSelected = selectedId === HOST_CHAT_MODE_ID
  return (
    <div data-ui="workbench.composer.modes" className="mb-10 flex justify-center">
      <div className="relative max-w-full">
        <div
          ref={ref}
          role="radiogroup"
          aria-label="工作模式"
          className="flex w-max max-w-full [scrollbar-width:none] flex-nowrap items-center gap-0.5 overflow-x-auto rounded-full bg-[#f5f5f5] p-1 [&::-webkit-scrollbar]:hidden">
          <button
            type="button"
            role="radio"
            aria-checked={chatSelected}
            className={radioClass(chatSelected)}
            onClick={() => selectHomeComposerMode(HOST_CHAT_MODE_ID)}>
            <ModeGlyph icon={modeIcon(undefined, true)} selected={chatSelected} />
            {HOST_CHAT_MODE_LABEL}
          </button>
          <button
            type="button"
            role="radio"
            aria-checked={hostCode}
            className={radioClass(hostCode)}
            onClick={() => selectHomeComposerMode(HOST_CODE_MODE_ID)}>
            <ModeGlyph icon={Terminal} selected={hostCode} />
            {HOST_CODE_MODE_LABEL}
          </button>
          {modes.map((mode) => (
            <button
              key={mode.key}
              type="button"
              role="radio"
              aria-checked={selectedId === mode.key}
              className={radioClass(selectedId === mode.key)}
              onClick={() => selectHomeComposerMode(mode.key)}>
              <ModeGlyph icon={modeIcon(mode.icon, false)} selected={selectedId === mode.key} />
              {mode.title}
            </button>
          ))}
        </div>
        {fade ? (
          <span
            aria-hidden
            className="pointer-events-none absolute inset-y-0 right-0 w-6 rounded-r-full bg-gradient-to-l from-[#f5f5f5] to-transparent"
          />
        ) : null}
      </div>
    </div>
  )
}

const FIELD_LABEL = 'text-[13px] text-[#3f3f3f]'
const FIELD_CONTROL =
  'h-9 rounded-[8px] bg-[#f5f5f5] text-[13px] text-[#171717] outline-none placeholder:text-[#8a8a8a]'

function stepNumber(control: HomeComposerControl, raw: string, direction: 1 | -1) {
  const min = control.min ?? 1
  const max = control.max ?? 20
  const text = raw.trim()
  const parsed = Number(text)
  const integer = text !== '' && Number.isInteger(parsed)
  if (!integer) return direction > 0 ? String(min) : ''
  if (direction < 0) return parsed <= min ? '' : String(parsed - 1)
  return String(parsed < min ? min : Math.min(max, parsed + 1))
}

function FieldLabel({ children }: { children: string }) {
  return (
    <span data-field-label className={FIELD_LABEL}>
      {children}
    </span>
  )
}

function NumberField({
  control,
  raw,
  language,
  onChange
}: {
  control: HomeComposerControl
  raw: string
  language: string
  onChange: (value: string) => void
}) {
  const min = control.min ?? 1
  const max = control.max ?? 20
  const text = raw.trim()
  const parsed = Number(text)
  const atMax = text !== '' && Number.isInteger(parsed) && parsed >= max
  const autoWord = composerWord('plugins.composer.auto', language)
  return (
    <div data-control-id={control.id} data-value={raw} className="flex flex-col gap-1.5">
      <FieldLabel>{control.label}</FieldLabel>
      <div className="flex items-center gap-1">
        <button
          type="button"
          className={`h-9 shrink-0 rounded-[8px] px-3 text-[13px] text-[#171717] hover:bg-[#f5f5f5] ${
            text === '' ? 'font-medium' : ''
          }`}
          onClick={() => onChange('')}>
          {autoWord}
        </button>
        <button
          type="button"
          aria-label={composerWord('plugins.composer.decrease', language, { label: control.label })}
          className="grid size-9 shrink-0 place-items-center rounded-[8px] text-[#171717] hover:bg-[#f5f5f5] disabled:opacity-40"
          disabled={text === ''}
          onClick={() => onChange(stepNumber(control, raw, -1))}>
          <Minus aria-hidden size={14} strokeWidth={1.8} />
        </button>
        <input
          aria-label={control.label}
          inputMode="numeric"
          value={raw}
          className={`${FIELD_CONTROL} min-w-0 flex-1 px-2 text-center`}
          onChange={(event) => onChange(event.target.value)}
          onKeyDown={(event) => {
            if (event.key !== 'Enter' || event.nativeEvent.isComposing) return
            event.preventDefault()
            void confirmHomeComposerSend()
          }}
        />
        <button
          type="button"
          aria-label={composerWord('plugins.composer.increase', language, { label: control.label })}
          className="grid size-9 shrink-0 place-items-center rounded-[8px] text-[#171717] hover:bg-[#f5f5f5] disabled:opacity-40"
          disabled={atMax}
          onClick={() => onChange(stepNumber(control, raw, 1))}>
          <Plus aria-hidden size={14} strokeWidth={1.8} />
        </button>
      </div>
      <p className="text-[12px] text-[#8a8a8a]">
        {min}–{max}
      </p>
    </div>
  )
}

function SelectField({
  control,
  raw,
  language,
  onChange
}: {
  control: HomeComposerControl
  raw: string
  language: string
  onChange: (value: string) => void
}) {
  const options = selectOptions(control)
  if (segmentedSelect(control)) {
    return (
      <div data-control-id={control.id} data-value={raw} className="flex flex-col gap-1.5">
        <FieldLabel>{control.label}</FieldLabel>
        <SegmentedControl aria-label={control.label} value={raw} options={options} onValueChange={onChange} />
      </div>
    )
  }
  const searchable = options.length > 8
  return (
    <div data-control-id={control.id} data-value={raw} className="flex flex-col gap-1.5">
      <FieldLabel>{control.label}</FieldLabel>
      <SearchSelect
        aria-label={control.label}
        allowClear={false}
        searchable={searchable}
        options={options}
        value={raw}
        placeholder={composerWord('plugins.composer.select', language, { label: control.label })}
        searchPlaceholder={composerWord('plugins.composer.search', language, { label: control.label })}
        emptyText={composerWord('plugins.composer.no_match', language, { label: control.label })}
        onChange={onChange}
      />
    </div>
  )
}

function ToggleField({
  control,
  on,
  language,
  onChange
}: {
  control: HomeComposerControl
  on: boolean
  language: string
  onChange: (value: boolean) => void
}) {
  const word = composerWord(on ? 'plugins.composer.on' : 'plugins.composer.off', language)
  return (
    <div
      data-control-id={control.id}
      data-value={on ? 'true' : 'false'}
      className="flex h-9 items-center justify-between gap-3">
      <FieldLabel>{control.label}</FieldLabel>
      <Switch size="sm" checked={on} aria-label={`${control.label}，${word}`} onCheckedChange={onChange} />
    </div>
  )
}

function TextField({
  control,
  raw,
  onChange
}: {
  control: HomeComposerControl
  raw: string
  onChange: (value: string) => void
}) {
  const max = control.max ?? 4000
  return (
    <div data-control-id={control.id} data-value={raw} className="flex flex-col gap-1.5 sm:col-span-2">
      <FieldLabel>{control.label}</FieldLabel>
      <textarea
        aria-label={control.label}
        rows={3}
        value={raw}
        className={`${FIELD_CONTROL} min-h-[72px] w-full resize-none px-3 py-2`}
        onChange={(event) => onChange(event.target.value)}
      />
      <p className="text-right text-[12px] text-[#8a8a8a]">
        {[...raw].length}/{max}
      </p>
    </div>
  )
}

function HomeComposerSendDialog() {
  const { selected, values, fieldError } = useHomeComposerModeState()
  const pending = useSyncExternalStore(subscribePending, readPending, readPending)
  const confirming = useSyncExternalStore(subscribePending, readConfirmingSend, readConfirmingSend)
  const language = useSyncExternalStore(subscribeComposerLanguage, composerLanguage, composerLanguage)
  const modeKey = selected?.key ?? null
  const previousMode = useRef(modeKey)

  useEffect(() => {
    if (previousMode.current === modeKey) return
    previousMode.current = modeKey
    cancelHomeComposerSend()
  }, [modeKey])

  if (!pending || !selected || selected.controls.length === 0) return null
  const write = (controlId: string, value: string | boolean) => {
    setHomeComposerControl(selected.key, controlId, value)
  }
  return (
    <Dialog
      open
      onOpenChange={(next) => {
        if (!next) cancelHomeComposerSend()
      }}>
      <DialogContent
        size="lg"
        style={{
          maxWidth: 'min(576px, calc(100% - var(--sidebar-width, 60px) - 2rem))',
          left: 'calc(50% + var(--sidebar-width, 60px) / 2)'
        }}
        className="flex max-h-[calc(100dvh-2rem)] flex-col gap-0 p-0"
        showCloseButton={!confirming}
        closeOnOverlayClick={!confirming}
        closeLabel={composerWord('plugins.composer.send_form.close', language)}
        onFocusOutside={keepSelectPanel}
        onInteractOutside={keepSelectPanel}
        onPointerDownOutside={keepSelectPanel}>
        <DialogHeader className="shrink-0 px-6 pt-5 pr-12 pb-4">
          <DialogTitle className="text-[15px] leading-5 font-medium">{selected.title}</DialogTitle>
          <DialogDescription className="text-[13px] text-[#8a8a8a]">
            {composerWord('plugins.composer.send_form.description', language)}
          </DialogDescription>
          <p
            data-presentation-topic=""
            className="mt-2 max-h-20 overflow-y-auto text-[13px] leading-5 whitespace-pre-wrap">
            {pending.input.text}
          </p>
        </DialogHeader>
        <fieldset
          disabled={confirming}
          data-ui="workbench.composer.controls"
          className="m-0 grid min-h-0 min-w-0 grid-cols-1 gap-4 overflow-y-auto border-0 px-6 pb-4 sm:grid-cols-2">
          {selected.controls.map((control) => {
            const value = values[control.id]
            if (control.kind === 'number') {
              return (
                <NumberField
                  key={control.id}
                  control={control}
                  raw={controlRaw(value)}
                  language={language}
                  onChange={(next) => write(control.id, next)}
                />
              )
            }
            if (control.kind === 'select') {
              return (
                <SelectField
                  key={control.id}
                  control={control}
                  raw={controlRaw(value)}
                  language={language}
                  onChange={(next) => write(control.id, next)}
                />
              )
            }
            if (control.kind === 'toggle') {
              return (
                <ToggleField
                  key={control.id}
                  control={control}
                  on={value === true}
                  language={language}
                  onChange={(next) => write(control.id, next)}
                />
              )
            }
            return (
              <TextField
                key={control.id}
                control={control}
                raw={controlRaw(value)}
                onChange={(next) => write(control.id, next)}
              />
            )
          })}
          {fieldError ? (
            <p role="alert" className="text-[13px] text-[#b42318] sm:col-span-2">
              {fieldError}
            </p>
          ) : null}
        </fieldset>
        <DialogFooter className="shrink-0 border-t px-6 py-4">
          <Button type="button" variant="outline" size="md" disabled={confirming} onClick={cancelHomeComposerSend}>
            {composerWord('plugins.composer.send_form.cancel', language)}
          </Button>
          <Button
            type="button"
            variant="emphasis"
            size="md"
            disabled={confirming}
            onClick={() => void confirmHomeComposerSend()}>
            {composerWord('plugins.composer.send_form.confirm', language)}
          </Button>
        </DialogFooter>
      </DialogContent>
    </Dialog>
  )
}

function HomeComposerModeNotice() {
  const { notice, fieldError } = useHomeComposerModeState()
  const pending = useSyncExternalStore(subscribePending, readPending, readPending)
  if (pending || (!notice && !fieldError)) return null
  return (
    <div className="mb-3">
      {fieldError ? (
        <p role="alert" className="text-[12px] text-[#b42318]">
          {fieldError}
        </p>
      ) : null}
      {notice ? (
        <p role="status" className="text-[12px] text-[#171717]">
          {notice}
        </p>
      ) : null}
    </div>
  )
}

export function HomeComposerModeChips({ active = true }: { active?: boolean }) {
  const homeActive = use(WorkbenchHomeActiveContext)
  if (!active && !homeActive) return null
  return (
    <>
      <HomeComposerSendDialog />
      <HomeComposerModeNotice />
    </>
  )
}

export function HomeComposerModeBar({ active = true }: { active?: boolean }) {
  return (
    <>
      <HomeComposerModeTrack active={active} />
      <HomeComposerModeChips active={active} />
    </>
  )
}
