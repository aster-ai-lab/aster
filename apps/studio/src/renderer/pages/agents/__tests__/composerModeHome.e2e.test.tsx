import { readFileSync } from 'node:fs'
import { dirname, join } from 'node:path'
import { fileURLToPath } from 'node:url'
import { MessageChannel as NodeMessageChannel } from 'node:worker_threads'
import { gzipSync, gunzipSync } from 'node:zlib'

import { act, cleanup, fireEvent, render, screen, waitFor, within } from '@testing-library/react'
import i18next from 'i18next'
import { JSDOM } from 'jsdom'
import { useState } from 'react'
import { createPortal } from 'react-dom'
import { afterEach, beforeAll, beforeEach, describe, expect, it, vi } from 'vitest'

import {
  getAgentDraftCacheKey,
  readAgentDraftCache,
  writeAgentDraftCache
} from '@renderer/components/composer/variants/agent/agentDraftCache'
import { QuickPanelProvider } from '@renderer/components/QuickPanel'
import {
  HOME_EVENT,
  listenAppletHomeSeed,
  receiveAppletHomeDocument,
  resetAppletHomeForTests,
  takeAndStageAppletHomeSeed
} from '@renderer/services/plugins/aixAppletHome'
import { publishAixCatalogue } from '@renderer/services/plugins/aixCatalogueStore'
import { AixViewBridge } from '@renderer/services/plugins/aixViewBridge'
import type { PluginActivationContext } from '@renderer/types/workbenchPlugin'
import type { Model } from '@shared/data/types/model'
import { aixWorkbenchCatalogueSchema } from '@shared/types/aixWorkbench'

import { LANGUAGES, TONES, VERBOSITIES } from '../../../../../../use-registry/packages/presenton/agent/deck.mjs'
import AgentComposer, { type AgentComposerSendOptions } from '../AgentComposer'
import { readHomeComposerSendEffect, resetHomeComposerModeForTests, setHomeComposerControl } from '../homeComposerMode'
import { PresentationSessionDeck, resetPresentationDeckForTests } from '../presentationDeck'
import { withPresentonHomepage } from '../presentonHomepage'
import { fillWorkbenchComposer, releaseSessionDraftWrites } from '../workbenchComposerBridge'
import { WorkbenchComposerHostContext, WorkbenchHomeActiveContext } from '../workbenchComposerHost'
import { WorkbenchHome } from '../WorkbenchHome'

// JSDOM elements have client rects. ProseMirror also asks ranges and text nodes.
function installClientRects(scope: typeof globalThis) {
  const box = () =>
    ({
      x: 0,
      y: 0,
      width: 0,
      height: 0,
      top: 0,
      right: 0,
      bottom: 0,
      left: 0,
      toJSON() {
        return {}
      }
    }) as DOMRect
  const protos = [scope.Range?.prototype, scope.Text?.prototype]
  for (const proto of protos) {
    if (!proto) continue
    if (typeof proto.getClientRects !== 'function') {
      Object.defineProperty(proto, 'getClientRects', {
        configurable: true,
        value() {
          return [] as unknown as DOMRectList
        }
      })
    }
    if (typeof proto.getBoundingClientRect !== 'function') {
      Object.defineProperty(proto, 'getBoundingClientRect', {
        configurable: true,
        value() {
          return box()
        }
      })
    }
  }
}

installClientRects(globalThis)

if (typeof Blob !== 'undefined' && typeof Blob.prototype.stream !== 'function') {
  Object.defineProperty(Blob.prototype, 'stream', {
    configurable: true,
    value(this: Blob) {
      const blob = this
      return new ReadableStream<Uint8Array>({
        async start(controller) {
          controller.enqueue(new Uint8Array(await blob.arrayBuffer()))
          controller.close()
        }
      })
    }
  })
}

const SLUG = 'a3s-presenton'
const VIEW_ID = 'workbench'
const DOCUMENT_ID = 'ab'.repeat(32)
const WORKBENCH = join(
  dirname(fileURLToPath(import.meta.url)),
  '../../../../../../use-registry/packages/presenton/ui/workbench'
)
/** The document the applet iframe loads: index.html names this script. */
const PACKAGED_APP = join(WORKBENCH, 'app.js')

interface HomeFact {
  delivery: number
  holdsWork: boolean
  applied: boolean | null
  seed: unknown
  notice: string | null
  modelRequests: number
}

interface ViewTesting {
  setOutlines(lines: string[]): void
  snapshot(): { content: string | null; outlines: string[] | null; seed: unknown; modelRequests: number }
}

function emptyFact(): HomeFact {
  return { delivery: 0, holdsWork: false, applied: null, seed: null, notice: null, modelRequests: 0 }
}

let guestCode = ''
let fact = emptyFact()
let openDelivery: number | null = null
let pendingSeed: { delivery: number; seed: string } | null = null
let nextDelivery = 0
let guestDocument: JSDOM | null = null
let guestWindow: (Window & { __a3sPresentonTesting?: ViewTesting }) | null = null
let booting: Promise<void> | null = null
let bootError: unknown = null
let stopSeedListen: (() => void) | null = null

async function settleGuestWindow(frame: Window) {
  const tick = () =>
    new Promise<void>((resolve) => {
      frame.setTimeout(resolve, 0)
    })
  try {
    await tick()
    await tick()
    await tick()
    await tick()
  } catch {
    // A closed window cannot schedule another turn.
  }
}

async function resetGuestShell() {
  fact = emptyFact()
  pendingSeed = null
  nextDelivery = 0
  booting = null
  bootError = null
  stopSeedListen?.()
  stopSeedListen = null
  const closing = guestDocument
  guestWindow = null
  guestDocument = null
  if (closing) {
    const frame = closing.window as Window & { jve?: () => null }
    // React effects read `location` after paint. Close deletes that binding, so
    // let the queued turn finish, then drop the reader, then close.
    await settleGuestWindow(frame)
    frame.jve = () => null
    frame.close()
  }
  resetAppletHomeForTests()
}

function stageShellSeed(raw: string) {
  const parsed: unknown = JSON.parse(raw)
  if (!parsed || typeof parsed !== 'object' || Array.isArray(parsed) || raw.length > 96 * 1024) {
    throw new Error('AIX mini app seed is invalid.')
  }
  nextDelivery += 1
  pendingSeed = { delivery: nextDelivery, seed: raw }
  fact = { ...fact, delivery: nextDelivery, applied: null, seed: null, notice: null, modelRequests: 0 }
  return nextDelivery
}

function takeShellSeed() {
  if (!pendingSeed) return null
  const taken = pendingSeed
  pendingSeed = null
  return { delivery: taken.delivery, seed: taken.seed }
}

function reportShellDeck(args: Record<string, unknown>) {
  const delivery = args.delivery
  if (typeof delivery !== 'number' || delivery === 0 || delivery !== fact.delivery) return
  const applied = args.applied === true ? true : args.applied === false ? false : null
  fact = {
    ...fact,
    holdsWork: args.holdsWork === true,
    modelRequests: typeof args.modelRequests === 'number' ? args.modelRequests : 0,
    notice: typeof args.notice === 'string' ? args.notice : null,
    applied: args.hasApplied === true && applied !== null ? applied : fact.applied,
    seed:
      args.hasSeed === true && args.seed && typeof args.seed === 'object' && !Array.isArray(args.seed)
        ? args.seed
        : fact.seed
  }
}

function bridgeContext(): PluginActivationContext {
  return {
    scopeId: 'composer-mode-home',
    services: {},
    signal: new AbortController().signal,
    isCurrent: () => true,
    defer() {},
    listen() {},
    spawn() {}
  }
}

function installGuestStreams(frame: Window) {
  const native = globalThis as typeof globalThis & {
    ReadableStream?: typeof ReadableStream
    TransformStream?: typeof TransformStream
    WritableStream?: typeof WritableStream
  }
  if (typeof frame.ReadableStream !== 'function' && native.ReadableStream) frame.ReadableStream = native.ReadableStream
  if (typeof frame.TransformStream !== 'function' && native.TransformStream) {
    frame.TransformStream = native.TransformStream
  }
  if (typeof frame.WritableStream !== 'function' && native.WritableStream) frame.WritableStream = native.WritableStream
  const blob = frame.Blob.prototype as Blob & { stream?: () => ReadableStream<Uint8Array> }
  if (typeof blob.stream !== 'function') {
    blob.stream = function stream(this: Blob) {
      const source = this
      return new frame.ReadableStream<Uint8Array>({
        async start(controller) {
          // JSDOM's Blob has no arrayBuffer(). FileReader is the reader it does implement.
          const bytes = await new Promise<Uint8Array>((resolve, reject) => {
            const reader = new frame.FileReader()
            reader.onload = () => {
              const result = reader.result
              if (result === null || typeof result === 'string') {
                reject(new Error('blob read failed'))
                return
              }
              resolve(new frame.Uint8Array(result))
            }
            reader.onerror = () => reject(reader.error ?? new Error('blob read failed'))
            reader.readAsArrayBuffer(source)
          })
          controller.enqueue(bytes)
          controller.close()
        }
      })
    }
  }
  const encode = (bytes: Uint8Array, compress: boolean) => {
    const out = compress ? gzipSync(bytes) : gunzipSync(bytes)
    const copy = new frame.Uint8Array(out.byteLength)
    copy.set(out)
    return copy
  }
  const codec = (compress: boolean) =>
    class CodecStream {
      readable: ReadableStream<Uint8Array>
      writable: WritableStream<Uint8Array>
      constructor(format: string) {
        if (format !== 'gzip') throw new Error(format)
        const chunks: Uint8Array[] = []
        const stream = new frame.TransformStream<Uint8Array, Uint8Array>({
          transform(chunk) {
            chunks.push(chunk instanceof Uint8Array ? chunk : new frame.Uint8Array(chunk))
          },
          flush(controller) {
            const size = chunks.reduce((sum, chunk) => sum + chunk.byteLength, 0)
            const all = new Uint8Array(size)
            let offset = 0
            for (const chunk of chunks) {
              all.set(chunk, offset)
              offset += chunk.byteLength
            }
            controller.enqueue(encode(all, compress))
          }
        })
        this.readable = stream.readable
        this.writable = stream.writable
      }
    }
  frame.CompressionStream = codec(true) as unknown as typeof CompressionStream
  frame.DecompressionStream = codec(false) as unknown as typeof DecompressionStream
  // The opened view gzips its deck with `new Response(stream)`. JSDOM has no Response.
  if (typeof frame.Response !== 'function' && typeof globalThis.Response === 'function') {
    frame.Response = globalThis.Response
  }
}

function startGuestDocument() {
  // index.html loads app.js as a module. JSDOM does not run module scripts, so
  // the same file bytes are the document script. This is ui/workbench/app.js.
  const page = readFileSync(join(WORKBENCH, 'index.html'), 'utf8').replace(
    /<script type="module" src="app\.js"><\/script>/,
    ''
  )
  const provider = new JSDOM(page, {
    url: `http://127.0.0.1/ui/workbench/index.html#aix-document=${DOCUMENT_ID}`,
    runScripts: 'dangerously',
    pretendToBeVisual: true,
    beforeParse(window) {
      const frame = window as Window & { __A3S_VIEW_TEST__?: boolean }
      frame.__A3S_VIEW_TEST__ = true
      frame.scrollTo = () => undefined
      installClientRects(frame)
      installGuestStreams(frame)
      if (typeof frame.Request !== 'function' && typeof globalThis.Request === 'function') {
        Object.defineProperty(frame, 'Request', { configurable: true, value: globalThis.Request })
      }
      if (typeof frame.Headers !== 'function' && typeof globalThis.Headers === 'function') {
        Object.defineProperty(frame, 'Headers', { configurable: true, value: globalThis.Headers })
      }
      Object.defineProperty(frame, 'MessageChannel', {
        configurable: true,
        value: function MessageChannel() {
          return new NodeMessageChannel()
        }
      })
      Object.defineProperty(frame, 'parent', {
        configurable: true,
        value: {
          postMessage(data: unknown, _origin: string, ports?: MessagePort[]) {
            globalThis.window.dispatchEvent(
              new globalThis.MessageEvent('message', {
                data,
                origin: 'null',
                ports: ports ?? []
              })
            )
          }
        }
      })
    }
  })
  const script = provider.window.document.createElement('script')
  script.textContent = guestCode
  provider.window.document.body.appendChild(script)
  return provider
}

async function bootGuestDocument() {
  const port = await new Promise<MessagePort>((resolve, reject) => {
    const timer = globalThis.setTimeout(() => reject(new Error('view document did not connect')), 8_000)
    const onMessage = (event: MessageEvent) => {
      if (event.data?.type !== 'aix.document.ready') return
      if (event.origin !== 'null' || event.ports.length !== 1) return
      globalThis.clearTimeout(timer)
      globalThis.window.removeEventListener('message', onMessage)
      const connected = event.ports[0]
      if (!connected) {
        reject(new Error('view document port missing'))
        return
      }
      resolve(connected)
    }
    globalThis.window.addEventListener('message', onMessage)
    try {
      guestDocument = startGuestDocument()
    } catch (error) {
      globalThis.clearTimeout(timer)
      globalThis.window.removeEventListener('message', onMessage)
      reject(error instanceof Error ? error : new Error(String(error)))
    }
  })
  await takeAndStageAppletHomeSeed(SLUG, VIEW_ID)
  const bridge = new AixViewBridge(bridgeContext(), VIEW_ID, { read: () => null, write: () => true })
  bridge.connect(port)
  receiveAppletHomeDocument(SLUG, VIEW_ID, port)
  guestWindow = guestDocument.window as Window & { __a3sPresentonTesting?: ViewTesting }
  stopSeedListen = listenAppletHomeSeed(() => {
    void takeAndStageAppletHomeSeed(SLUG, VIEW_ID)
  })
}

function deliverGuestSeed() {
  if (guestWindow) {
    globalThis.window.dispatchEvent(new Event(HOME_EVENT))
    return
  }
  if (!booting) booting = bootGuestDocument()
  void booting.catch((error: unknown) => {
    bootError = error
  })
}

function viewTesting() {
  const hook = guestWindow?.__a3sPresentonTesting
  if (!hook) throw new Error('opened view hook missing')
  return hook
}

const chatSends: { text?: string; options?: AgentComposerSendOptions }[] = []

const liveModel = { id: 'model-1', name: 'Model' } as Model

function LiveHome() {
  const [host, setHost] = useState<HTMLElement | null>(null)
  return (
    <WorkbenchHomeActiveContext value={true}>
      <WorkbenchComposerHostContext value={host}>
        <WorkbenchHome
          sessions={[]}
          activeSessionId={null}
          homeActive
          useLiveComposer
          onComposerHostChange={setHost}
          onNewTask={vi.fn()}
          onOpenSession={vi.fn()}
          onTogglePin={vi.fn()}
          onDeleteSession={vi.fn()}
          onRenameSession={vi.fn()}
        />
        {host
          ? createPortal(
              <QuickPanelProvider>
                <AgentComposer
                  agentId="agent-1"
                  sessionId="session-1"
                  sessionOverride={{ workspaceId: null }}
                  resolvedAgent={undefined}
                  resolvedModel={liveModel}
                  resolvedWorkspaceWarning={null}
                  canChangeModel={false}
                  sendMessage={async (message, options) => {
                    chatSends.push({ text: message?.text, options })
                    return true
                  }}
                  stop={async () => undefined}
                  isStreaming={false}
                />
              </QuickPanelProvider>,
              host
            )
          : null}
      </WorkbenchComposerHostContext>
    </WorkbenchHomeActiveContext>
  )
}

const fallbackCreated: string[] = []
let fallbackAdopted: string | null = null
const fallbackHomeTask = {
  create: async () => {
    const id = `fallback-task-${fallbackCreated.length + 1}`
    fallbackCreated.push(id)
    return { id }
  },
  adopt: (id: string) => {
    fallbackAdopted = id
  }
}

function LeavingTaskHome({ onNewTask, onOpenSession }: { onNewTask: () => void; onOpenSession: (id: string) => void }) {
  const [home, setHome] = useState(false)
  const [host, setHost] = useState<HTMLElement | null>(null)
  return (
    <WorkbenchHomeActiveContext value={home}>
      <WorkbenchComposerHostContext value={host}>
        <WorkbenchHome
          sessions={[{ id: 'session-1', name: '其他任务', updatedAt: '2026-10-01T00:00:00.000Z', pinned: false }]}
          activeSessionId="session-1"
          homeActive={home}
          useLiveComposer
          onComposerHostChange={setHost}
          onNewTask={() => {
            setHome(true)
            onNewTask()
          }}
          onOpenSession={onOpenSession}
          onTogglePin={vi.fn()}
          onDeleteSession={vi.fn()}
          onRenameSession={vi.fn()}
        />
        <QuickPanelProvider>
          <AgentComposer
            agentId="agent-1"
            sessionId="session-1"
            sessionOverride={{ workspaceId: null }}
            resolvedAgent={undefined}
            resolvedModel={liveModel}
            resolvedWorkspaceWarning={null}
            canChangeModel={false}
            sendMessage={async () => true}
            stop={async () => undefined}
            isStreaming={false}
          />
        </QuickPanelProvider>
      </WorkbenchComposerHostContext>
    </WorkbenchHomeActiveContext>
  )
}

function FallbackHome() {
  return (
    <WorkbenchHomeActiveContext value={true}>
      <WorkbenchComposerHostContext value={null}>
        <WorkbenchHome
          sessions={[]}
          activeSessionId={null}
          homeActive
          useLiveComposer={false}
          homeTask={fallbackHomeTask}
          onNewTask={vi.fn()}
          onOpenSession={vi.fn()}
          onTogglePin={vi.fn()}
          onDeleteSession={vi.fn()}
          onRenameSession={vi.fn()}
        />
      </WorkbenchComposerHostContext>
    </WorkbenchHomeActiveContext>
  )
}

vi.mock('@renderer/services/toast', () => ({
  toast: { info: vi.fn(), success: vi.fn(), error: vi.fn(), warning: vi.fn() }
}))

vi.mock('@renderer/data/hooks/usePreference', async () => {
  const actual = await import('../../../../../tests/__mocks__/renderer/usePreference')
  const { mockPreferenceState } = await import('../../../../../tests/__mocks__/renderer/PreferenceService')
  const { getDefaultValue } = await import('@shared/data/preference/preferenceUtils')
  return {
    ...actual.MockUsePreference,
    usePreference: (key: string, options?: { optimistic?: boolean }) => {
      if (!mockPreferenceState.has(key)) {
        try {
          const fallback = getDefaultValue(key as never)
          if (fallback !== undefined) mockPreferenceState.set(key, fallback)
        } catch {
          /* An unknown key keeps the renderer preference mock. */
        }
      }
      return actual.mockUsePreference(key, options)
    }
  }
})

vi.mock('react-i18next', () => {
  const translate = (key: string) => key
  const i18n = {
    language: 'zh-CN',
    resolvedLanguage: 'zh-CN',
    t: translate
  }
  return {
    initReactI18next: { type: '3rdParty', init: vi.fn() },
    useTranslation: () => ({ t: translate, i18n })
  }
})

vi.mock('@tanstack/react-router', () => ({
  useNavigate: () => vi.fn(),
  getRouteApi: () =>
    new Proxy(
      {},
      {
        get: () => () => ({})
      }
    )
}))

vi.mock('../../settings/TasksSettings', () => ({
  default: () => <section data-ui="workbench.automation" />
}))

const KEPT_NOTICE = '当前演示文稿还在，首页草稿未写入'
const SIMPLIFIED_CHINESE = 'Chinese (Simplified - 中文, 汉语)'

const oracles: string[] = []

function pass(name: string) {
  oracles.push(name)
  console.log(`oracle.${name}=pass`)
}

function quotedList(body: string, name: string) {
  const match = body.match(new RegExp(`${name} = \\[([\\s\\S]*?)\\]`))
  if (!match?.[1]) throw new Error(name)
  return [...match[1].matchAll(/"([^"\\]*)"/g)].map((item) => item[1] ?? '')
}

function controlBody(acl: string, id: string) {
  const start = acl.indexOf(`control "${id}" {`)
  if (start < 0) throw new Error(id)
  const next = acl.indexOf('\n    control ', start + 10)
  return acl.slice(start, next < 0 ? acl.length : next)
}

function blockBody(source: string, header: string) {
  const at = source.indexOf(header)
  if (at < 0) throw new Error(header)
  const open = source.indexOf('{', at)
  let depth = 0
  for (let i = open; i < source.length; i += 1) {
    if (source[i] === '{') depth += 1
    else if (source[i] === '}') {
      depth -= 1
      if (depth === 0) return source.slice(open + 1, i)
    }
  }
  throw new Error(header)
}

function assignedLine(body: string, name: string) {
  const match = new RegExp(`(?:^|\\n)\\s*${name}\\s*=\\s*"([^"]*)"`).exec(body)
  if (!match?.[1]) throw new Error(name)
  return match[1]
}

function assignedNumber(body: string, name: string) {
  const match = new RegExp(`(?:^|\\n)\\s*${name}\\s*=\\s*(\\d+)\\b`).exec(body)
  if (!match?.[1]) throw new Error(name)
  return Number(match[1])
}

function presentonCatalogue() {
  const aclPath = join(
    dirname(fileURLToPath(import.meta.url)),
    '../../../../../../use-registry/packages/presenton/a3s-use-extension.acl'
  )
  const acl = readFileSync(aclPath, 'utf8')
  const mode = blockBody(acl, 'composer_mode "slides"')
  const viewId = /ui "([^"]+)"/.exec(acl)?.[1]
  if (!viewId) throw new Error('ui')
  const view = blockBody(acl, `ui "${viewId}"`)
  const editor = assignedLine(mode, 'editor')
  if (editor !== viewId) throw new Error(editor)
  const toneLabels = quotedList(controlBody(acl, 'tone'), 'option_labels')
  const languageLabels = quotedList(controlBody(acl, 'language'), 'option_labels')
  const verbosityLabels = quotedList(controlBody(acl, 'verbosity'), 'option_labels')
  return aixWorkbenchCatalogueSchema.parse({
    protocolVersion: 1,
    revision: '',
    packages: [
      {
        slug: 'a3s-presenton',
        packageId: 'use/a3s/presenton',
        generation: 1,
        revision: `sha256:${'ab'.repeat(32)}`,
        containers: [],
        views: [
          {
            id: viewId,
            title: assignedLine(view, 'title'),
            description: assignedLine(view, 'description'),
            icon: assignedLine(view, 'icon'),
            order: 100,
            container: null,
            point: assignedLine(view, 'point'),
            presentation: assignedLine(view, 'presentation')
          }
        ],
        composerModes: [
          {
            id: 'slides',
            contractVersion: assignedNumber(mode, 'contract_version'),
            title: assignedLine(mode, 'title'),
            description: assignedLine(mode, 'description'),
            icon: assignedLine(mode, 'icon'),
            agent: assignedLine(mode, 'agent'),
            editor,
            keptNotice: assignedLine(mode, 'kept_notice'),
            scope: assignedLine(mode, 'scope'),
            context: ['workspace'],
            order: 20,
            prompt: assignedLine(mode, 'prompt'),
            controls: [
              { id: 'n_slides', kind: 'number', label: '页数', min: 1, max: 20 },
              {
                id: 'language',
                kind: 'select',
                label: '语言',
                initial: 'Chinese (Simplified - 中文, 汉语)',
                options: quotedList(controlBody(acl, 'language'), 'options'),
                optionLabels: languageLabels
              },
              {
                id: 'tone',
                kind: 'select',
                label: '语气',
                initial: 'default',
                options: quotedList(controlBody(acl, 'tone'), 'options'),
                optionLabels: toneLabels
              },
              {
                id: 'verbosity',
                kind: 'select',
                label: '详略',
                initial: 'standard',
                options: quotedList(controlBody(acl, 'verbosity'), 'options'),
                optionLabels: verbosityLabels
              },
              { id: 'include_title_slide', kind: 'toggle', label: '标题页', initial: true },
              { id: 'instructions', kind: 'text', label: '补充说明', max: 4000 }
            ]
          }
        ]
      }
    ]
  })
}

async function renderLiveHome() {
  const view = render(<LiveHome />)
  await waitFor(() => {
    const node = document.querySelector('[data-ui="workbench.composer"]')
    if (!(node instanceof HTMLElement)) throw new Error('live composer missing')
    if (node.parentElement?.getAttribute('data-ui') !== 'workbench.composer.anchor') {
      throw new Error('live composer is not in the home anchor')
    }
    if (!(node.querySelector('textarea') instanceof HTMLTextAreaElement)) throw new Error('live topic missing')
    if (document.querySelector('[data-ui="workbench.composer.fallback"]')) {
      throw new Error('fallback composer is still mounted')
    }
  })
  return view
}

function composer() {
  const node = document.querySelector('[data-ui="workbench.composer"]')
  if (!(node instanceof HTMLElement)) throw new Error('live composer missing')
  if (node.parentElement?.getAttribute('data-ui') !== 'workbench.composer.anchor') {
    throw new Error('live composer is not in the home anchor')
  }
  if (document.querySelector('[data-ui="workbench.composer.fallback"]')) {
    throw new Error('fallback composer is still mounted')
  }
  return node
}

function sendButton() {
  return screen.getByRole('button', { name: 'chat.input.send' })
}

function controlValue(id: string) {
  const node = document.querySelector(`[data-ui="workbench.composer.controls"] [data-control-id="${id}"]`)
  if (!(node instanceof HTMLElement)) throw new Error(id)
  return node.getAttribute('data-value') ?? ''
}

function enteredSeed() {
  const topic = composer().querySelector('textarea')
  if (!(topic instanceof HTMLTextAreaElement)) throw new Error('topic missing')
  const pages = controlValue('n_slides').trim()
  return {
    content: topic.value.trim(),
    n_slides: pages ? Number(pages) : null,
    language: controlValue('language'),
    tone: controlValue('tone'),
    verbosity: controlValue('verbosity'),
    include_title_slide: controlValue('include_title_slide') === 'true',
    instructions: controlValue('instructions')
  }
}

const CHIP_WORD_KEYS = [
  'plugins.composer.auto',
  'plugins.composer.on',
  'plugins.composer.off',
  'plugins.composer.editor_draft_kept',
  'plugins.composer.integer',
  'plugins.composer.max_length',
  'plugins.composer.no_match',
  'plugins.composer.search',
  'plugins.composer.select',
  'workbench.presentation.preparing',
  'workbench.presentation.ready',
  'plugins.composer.send_form.cancel',
  'plugins.composer.send_form.close',
  'plugins.composer.send_form.confirm',
  'plugins.composer.send_form.description'
] as const

function localeCatalog(name: 'zh-cn' | 'en-us') {
  return JSON.parse(
    readFileSync(join(dirname(fileURLToPath(import.meta.url)), `../../../i18n/locales/${name}.json`), 'utf8')
  ) as Record<string, string>
}

function loadComposerWords() {
  const zh = localeCatalog('zh-cn')
  const en = localeCatalog('en-us')
  for (const key of CHIP_WORD_KEYS) {
    const zhWord = zh[key]
    const enWord = en[key]
    if (!zhWord || !enWord) throw new Error(key)
    i18next.addResource('zh-CN', 'translation', key, zhWord)
    i18next.addResource('en-US', 'translation', key, enWord)
  }
}

function shippedKeptNotice() {
  const locale = JSON.parse(
    readFileSync(join(dirname(fileURLToPath(import.meta.url)), '../../../i18n/locales/zh-cn.json'), 'utf8')
  ) as Record<string, string>
  const notice = locale['plugins.composer.draft_kept']
  if (notice !== KEPT_NOTICE) throw new Error(notice ?? 'missing draft notice')
  return notice
}

describe('homepage composer mode', () => {
  const invokes: { command: string; args: Record<string, unknown> }[] = []

  beforeAll(async () => {
    guestCode = readFileSync(PACKAGED_APP, 'utf8')
    if (!guestCode.includes('aix.view.seed') || !guestCode.includes('aix.view.home')) {
      throw new Error('packaged app.js does not apply the homepage seed')
    }
    const notice = shippedKeptNotice()
    if (!i18next.isInitialized) {
      await i18next.init({
        lng: 'zh-CN',
        fallbackLng: 'zh-CN',
        keySeparator: false,
        initImmediate: false,
        interpolation: { escapeValue: false },
        resources: { 'zh-CN': { translation: { 'plugins.composer.draft_kept': notice } } }
      })
    } else {
      i18next.addResource('zh-CN', 'translation', 'plugins.composer.draft_kept', notice)
      await i18next.changeLanguage('zh-CN')
    }
    loadComposerWords()
  })

  beforeEach(async () => {
    oracles.length = 0
    invokes.length = 0
    chatSends.length = 0
    fallbackCreated.length = 0
    fallbackAdopted = null
    releaseSessionDraftWrites()
    openDelivery = null
    if (i18next.language !== 'zh-CN') await i18next.changeLanguage('zh-CN')
    resetHomeComposerModeForTests()
    resetPresentationDeckForTests()
    await resetGuestShell()
    publishAixCatalogue({ protocolVersion: 1, revision: '', packages: [] })
    Object.defineProperty(globalThis, '__TAURI_INTERNALS__', {
      configurable: true,
      value: {
        invoke: async (command: string, args: Record<string, unknown> = {}) => {
          invokes.push({ command, args })
          if (command === 'open_aix_applet_window') {
            if (openDelivery === 0 || typeof args.seed !== 'string') return 0
            const delivery = stageShellSeed(args.seed)
            deliverGuestSeed()
            return delivery
          }
          if (command === 'take_aix_applet_home_seed') return takeShellSeed()
          if (command === 'report_aix_applet_home_deck') {
            reportShellDeck(args)
            return null
          }
          if (command === 'read_aix_applet_home_deck') return fact.delivery > 0 ? { ...fact } : null
          return null
        }
      }
    })
  })

  function opens() {
    return invokes.filter((call) => call.command === 'open_aix_applet_window')
  }

  afterEach(async () => {
    cleanup()
    await resetGuestShell()
  })

  const TOPIC = '边界主题'
  const CONTROL_LABELS = ['页数', '语言', '语气', '详略', '标题页', '补充说明']

  function controlLabels() {
    const controls = document.querySelector('[data-ui="workbench.composer.controls"]')
    if (!(controls instanceof HTMLElement)) throw new Error('controls missing')
    return [...controls.querySelectorAll('[data-field-label]')].map((node) => node.textContent)
  }

  async function openControls(sendName: string) {
    const open = screen.queryByRole('dialog')
    if (open) return open
    fireEvent.click(screen.getByRole('button', { name: sendName }))
    return screen.findByRole('dialog')
  }

  function confirmControls() {
    fireEvent.click(screen.getByRole('button', { name: i18next.t('plugins.composer.send_form.confirm') }))
  }

  function cancelControls() {
    fireEvent.click(screen.getByRole('button', { name: i18next.t('plugins.composer.send_form.cancel') }))
  }

  function topicText(root: ParentNode) {
    const topic = root.querySelector('textarea')
    if (topic instanceof HTMLTextAreaElement) return topic.value
    const editor = root.querySelector('.composer-tiptap, [contenteditable="true"]')
    if (editor instanceof HTMLElement) return (editor.textContent ?? '').replace(/\u200b/g, '').trim()
    throw new Error('topic missing')
  }

  function writeTopic(root: ParentNode, value: string) {
    const topic = root.querySelector('textarea')
    if (topic instanceof HTMLTextAreaElement) {
      fireEvent.change(topic, { target: { value } })
      return
    }
    act(() => {
      fillWorkbenchComposer(value)
    })
  }

  function fallbackCard() {
    const card = document.querySelector('[data-ui="workbench.composer.fallback"]')
    if (!(card instanceof HTMLElement)) throw new Error('fallback composer missing')
    return card
  }

  async function chooseSlides() {
    const title = withPresentonHomepage(presentonCatalogue()).packages[0]?.composerModes?.[0]?.title
    if (!title) throw new Error('slides title')
    await act(async () => {
      publishAixCatalogue(withPresentonHomepage(presentonCatalogue()))
    })
    fireEvent.click(screen.getByRole('radio', { name: title }))
    return title
  }

  function slidesModeKey() {
    const pack = presentonCatalogue().packages[0]
    const id = pack?.composerModes?.[0]?.id
    if (!pack?.packageId || !id) throw new Error('slides mode')
    return `${pack.packageId}/${id}`
  }

  function setPages(value: string) {
    fireEvent.change(screen.getByRole('textbox', { name: '页数' }), { target: { value } })
  }

  function setInstructions(value: string) {
    fireEvent.change(screen.getByRole('textbox', { name: '补充说明' }), { target: { value } })
    const stored = document.querySelector('[data-control-id="instructions"]')?.getAttribute('data-value') ?? ''
    if ([...stored].length !== [...value].length) setHomeComposerControl(slidesModeKey(), 'instructions', value)
  }

  async function waitStored(slides: number, instructions?: string) {
    await waitFor(() => {
      const effect = readHomeComposerSendEffect()
      const seed = effect?.seed
      if (!seed || typeof seed !== 'object') throw new Error('seed missing')
      const record = seed as { n_slides?: unknown; instructions?: unknown; content?: unknown }
      expect(effect?.kind).toBe('chat')
      expect(record.content).toBe(TOPIC)
      expect(record.n_slides).toBe(slides)
      if (instructions !== undefined) expect(record.instructions).toBe(instructions)
    })
    expect(opens()).toEqual([])
    expect(guestWindow).toBeNull()
  }

  async function expectFieldRefusal(alert: string, openCount: number, root: ParentNode) {
    await waitFor(() => {
      expect(screen.getByRole('alert')).toHaveTextContent(alert)
      expect(topicText(root)).toBe(TOPIC)
    })
    expect(opens()).toHaveLength(openCount)
    expect(readHomeComposerSendEffect()).toMatchObject({
      kind: 'refused',
      modeId: 'slides',
      seed: null,
      opened: null
    })
  }

  it('keeps a new task from writing the home draft into the bound session', async () => {
    const key = getAgentDraftCacheKey('session-1')
    const scope = { workspaceKey: '', agentId: 'agent-1' }
    const seed = () =>
      writeAgentDraftCache(key, {
        text: 'kept-in-other-task',
        tokens: [],
        files: [],
        knowledgeBaseIds: [],
        workspaceKey: '',
        agentId: 'agent-1'
      })
    seed()
    await renderLiveHome()
    const homeTopic = document.querySelector('[data-ui="workbench.composer"] textarea')
    if (!(homeTopic instanceof HTMLTextAreaElement)) throw new Error('home composer missing')
    fireEvent.change(homeTopic, { target: { value: 'home draft stays out' } })
    await act(async () => {
      await new Promise((resolve) => {
        setTimeout(resolve, 0)
      })
    })
    expect(readAgentDraftCache(key, scope).text).toBe('kept-in-other-task')
    fireEvent.click(document.querySelector('[data-new-task-trigger="true"]')!)
    await act(async () => {
      await new Promise((resolve) => {
        setTimeout(resolve, 0)
      })
    })
    expect(homeTopic.value).toBe('')
    expect(readAgentDraftCache(key, scope).text).toBe('kept-in-other-task')
    expect(screen.getByRole('radio', { name: '日常办公' })).toHaveAttribute('aria-checked', 'true')

    cleanup()
    releaseSessionDraftWrites()
    seed()
    const onNewTask = vi.fn()
    const onOpenSession = vi.fn()
    render(<LeavingTaskHome onNewTask={onNewTask} onOpenSession={onOpenSession} />)
    const bound = await waitFor(() => {
      const node = document.querySelector('textarea')
      if (!(node instanceof HTMLTextAreaElement)) throw new Error('bound composer missing')
      return node
    })
    fireEvent.click(document.querySelector('[data-new-task-trigger="true"]')!)
    await act(async () => {
      await new Promise((resolve) => {
        setTimeout(resolve, 0)
      })
    })
    expect(onNewTask).toHaveBeenCalledOnce()
    expect(onOpenSession).not.toHaveBeenCalled()
    expect(bound.value).toBe('')
    expect(readAgentDraftCache(key, scope).text).toBe('kept-in-other-task')
  })

  it('drives the home composer through Presenton outline controls without a model call', async () => {
    await renderLiveHome()
    expect(screen.getByRole('radio', { name: '日常办公' })).toHaveAttribute('aria-checked', 'true')
    expect(screen.getByRole('radio', { name: '代码开发' })).toHaveAttribute('aria-checked', 'false')
    expect(composer().querySelector('[data-workbench-chip="model"]')).toBeTruthy()
    expect(composer().querySelector('[data-workbench-chip="authorize"]')).toHaveAttribute('data-posture', 'default')
    expect(sendButton()).toBeTruthy()
    pass('emptyCatalogue')

    const posture = document.querySelector('[data-workbench-chip="authorize"]')?.getAttribute('data-posture')
    const admitted = presentonCatalogue()
    const slides = admitted.packages[0]?.composerModes?.[0]
    expect(slides?.contractVersion).toBe(1)
    const driveRoot = join(dirname(fileURLToPath(import.meta.url)), '../../../../..')
    const bypass = ['with', 'Presenton', 'Homepage'].join('')
    for (const path of [
      'tests/e2e/composer-mode-home.mjs',
      'src/renderer/pages/agents/homeComposerMode.tsx',
      'src/renderer/pages/agents/WorkbenchHome.tsx',
      'src/renderer/pages/agents/AgentComposer.tsx'
    ]) {
      expect(readFileSync(join(driveRoot, path), 'utf8').includes(bypass)).toBe(false)
    }
    expect(slides?.title).toBe('幻灯片')
    expect(slides?.editor).toBe(admitted.packages[0]?.views[0]?.id)
    const published = withPresentonHomepage(admitted)
    const shownTitle = published.packages[0]?.composerModes?.[0]?.title
    expect(shownTitle).toBe('演示文稿')
    expect(published.packages[0]?.composerModes?.[0]).toMatchObject({ task: true, editor: 'workbench' })
    await act(async () => {
      publishAixCatalogue(published)
    })
    const radios = screen.getAllByRole('radio')
    expect(radios.map((radio) => radio.textContent?.replace(/\s+/g, ''))).toEqual(['日常办公', '代码开发', shownTitle])
    expect(radios[0]).toHaveAttribute('aria-checked', 'true')
    expect(radios[1]).toHaveAttribute('aria-checked', 'false')
    expect(radios[2]).toHaveAttribute('aria-checked', 'false')
    expect(composer().querySelector('[data-ui="workbench.composer.controls"]')).toBeNull()
    expect(screen.queryByRole('textbox', { name: '页数' })).toBeNull()
    pass('chatFirstSlidesSecond')

    fireEvent.click(screen.getByRole('radio', { name: shownTitle ?? '' }))
    expect(screen.getByRole('radio', { name: shownTitle ?? '' })).toHaveAttribute('aria-checked', 'true')
    expect(screen.getByRole('radio', { name: '日常办公' })).toHaveAttribute('aria-checked', 'false')
    expect(document.querySelector('[data-ui="workbench.composer.controls"]')).toBeNull()
    expect(screen.queryByRole('dialog')).toBeNull()
    const topicNode = composer().querySelector('textarea')
    const modeTrack = document.querySelector('[data-ui="workbench.composer.modes"]')
    const actions = composer().querySelector('[data-ui~="part:composer-actions"]')
    expect(modeTrack && topicNode && actions).toBeTruthy()
    expect(composer().contains(modeTrack)).toBe(false)
    expect(topicNode!.compareDocumentPosition(actions!) & Node.DOCUMENT_POSITION_FOLLOWING).toBeTruthy()
    const modeGroup = screen.getByRole('radiogroup', { name: '工作模式' })
    expect(modeGroup.className).toContain('flex-nowrap')
    expect(modeGroup.className).not.toMatch(/\bflex-wrap\b/)
    expect(modeGroup.className).not.toContain('mx-auto')
    expect(composer().querySelector('textarea')).toHaveAttribute('placeholder', '输入演示文稿的主题')
    if (!(topicNode instanceof HTMLTextAreaElement)) throw new Error('topic missing')
    fireEvent.change(topicNode, { target: { value: TOPIC } })
    const form = await openControls('chat.input.send')
    expect(composer().contains(form)).toBe(false)
    expect(controlLabels()).toEqual(CONTROL_LABELS)
    expect(within(form).getByRole('textbox', { name: '页数' })).toHaveValue('')
    expect(within(form).getByRole('button', { name: '自动', exact: true })).toBeTruthy()
    const languageTrigger = within(form).getByRole('combobox', { name: '语言' })
    expect(languageTrigger).toHaveTextContent('简体中文')
    expect(languageTrigger).not.toHaveTextContent('萨摩亚语')
    expect(languageTrigger).toHaveAttribute('aria-expanded', 'false')
    expect(within(form).queryByRole('listbox')).toBeNull()
    expect(within(form).queryByRole('option')).toBeNull()
    fireEvent.click(languageTrigger)
    expect(screen.getByRole('option', { name: '简体中文' })).toHaveAttribute('aria-selected', 'true')
    expect(screen.getAllByRole('option')).toHaveLength(LANGUAGES.length)
    fireEvent.click(languageTrigger)
    expect(screen.queryByRole('option')).toBeNull()
    const toneTrigger = within(form).getByRole('combobox', { name: '语气' })
    expect(toneTrigger).toHaveTextContent('默认')
    expect(toneTrigger).not.toHaveTextContent('专业')
    fireEvent.click(toneTrigger)
    expect(screen.getAllByRole('option')).toHaveLength(TONES.length)
    fireEvent.click(toneTrigger)
    const density = form.querySelector('[data-control-id="verbosity"] [data-testid="segmented-control"]') as HTMLElement
    expect(density).toHaveAttribute('aria-label', '详略')
    expect(density).toHaveAttribute('data-value', 'standard')
    expect(
      within(density)
        .getAllByRole('button')
        .map((button) => button.textContent)
    ).toEqual(['简洁', '标准', '详尽'])
    fireEvent.click(within(density).getByRole('button', { name: '详尽' }))
    expect(controlValue('verbosity')).toBe(VERBOSITIES[2])
    fireEvent.click(within(density).getByRole('button', { name: '标准' }))
    expect(controlValue('verbosity')).toBe('standard')
    expect(within(form).getByRole('switch', { name: '标题页，开' })).toHaveAttribute('aria-checked', 'true')
    expect(within(form).getByRole('textbox', { name: '补充说明' }).tagName).toBe('TEXTAREA')
    expect(composer().textContent ?? '').not.toMatch(/general|modern|swift|Chinese \(Simplified/)
    expect(composer().querySelector('[value="general"], [value="modern"], [value="swift"]')).toBeNull()
    expect(document.querySelector('[data-workbench-chip="authorize"]')).toHaveAttribute('data-posture', posture)
    cancelControls()
    expect(screen.queryByRole('dialog')).toBeNull()
    expect(opens()).toEqual([])
    pass('outlineControls')

    const topic = composer().querySelector('textarea')
    if (!(topic instanceof HTMLTextAreaElement)) throw new Error('topic missing')
    fireEvent.change(topic, { target: { value: '   ' } })
    const send = sendButton()
    expect(send).toHaveAttribute('aria-disabled', 'true')
    fireEvent.click(send)
    expect(opens()).toEqual([])
    expect(guestWindow).toBeNull()
    pass('blankTopic')

    const topicText = '季度业务回顾'
    fireEvent.change(topic, { target: { value: topicText } })
    await openControls('chat.input.send')
    const blankPageEntered = enteredSeed()
    expect(blankPageEntered.n_slides).toBeNull()
    expect(blankPageEntered.language).toBe(SIMPLIFIED_CHINESE)
    expect(screen.getByRole('combobox', { name: '语言' })).toHaveTextContent('简体中文')
    confirmControls()
    await waitFor(() => expect(readHomeComposerSendEffect()?.seed).toEqual(blankPageEntered), { timeout: 8_000 })
    expect(chatSends).toEqual([])
    expect(topic.value).toBe('')
    expect(opens()).toEqual([])
    expect(guestWindow).toBeNull()
    expect(fact.modelRequests).toBe(0)
    const deck = render(<PresentationSessionDeck sessionId="session-1" />)
    const work = document.querySelector('[data-ui="workbench.presentation"]')
    expect(work?.textContent).toContain(topicText)
    expect(work?.textContent).toContain('页数：自动')
    expect(work?.textContent).not.toContain('{"step":"outline"')
    expect(work?.textContent).toContain('正在准备演示文稿大纲')
    expect(work?.textContent).not.toContain('让想法变成幻灯片')
    expect(work?.querySelector('[data-presentation-status="outline"]')).toBeTruthy()
    deck.unmount()
    expect(readHomeComposerSendEffect()).toMatchObject({
      kind: 'chat',
      modeId: 'slides',
      permissionMode: posture,
      research: false,
      autoReview: false,
      homeTurnDirectives: null,
      opened: null,
      seed: blankPageEntered
    })
    pass('blankPageSeed')

    fireEvent.change(topic, { target: { value: topicText } })
    await openControls('chat.input.send')
    setPages('8')
    fireEvent.click(screen.getByRole('combobox', { name: '语言' }))
    fireEvent.click(screen.getByRole('option', { name: '英语', exact: true }))
    fireEvent.click(screen.getByRole('combobox', { name: '语气' }))
    fireEvent.click(screen.getByRole('option', { name: '专业' }))
    fireEvent.click(screen.getByRole('switch', { name: '标题页，开' }))
    fireEvent.change(screen.getByRole('textbox', { name: '补充说明' }), { target: { value: '给董事会' } })
    expect(screen.getByRole('combobox', { name: '语言' })).toHaveTextContent('英语')
    const entered = enteredSeed()
    expect(entered.language).toBe('English')
    expect(entered.n_slides).toBe(8)
    expect(entered.include_title_slide).toBe(false)
    expect(entered.instructions).toBe('给董事会')
    confirmControls()
    await waitFor(() => expect(readHomeComposerSendEffect()?.seed).toEqual(entered), { timeout: 8_000 })
    expect(chatSends).toEqual([])
    const enteredPane = render(<PresentationSessionDeck sessionId="session-1" />)
    const enteredWork = document.querySelector('[data-ui="workbench.presentation"]')
    expect(enteredWork?.textContent).toContain('页数：8')
    expect(enteredWork?.textContent).toContain('给董事会')
    expect(enteredWork?.textContent).not.toContain('{"step":"outline"')
    enteredPane.unmount()
    expect(fact.modelRequests).toBe(0)
    expect(entered.tone).toBe('professional')
    expect(TONES).toContain('professional')
    expect(opens()).toEqual([])
    expect(guestWindow).toBeNull()
    expect(readHomeComposerSendEffect()?.seed).toEqual(entered)
    pass('enteredSeed')

    fact = { ...fact, holdsWork: true, delivery: 1, applied: true }
    writeTopic(composer(), '另一份草稿')
    await waitFor(() => expect(sendButton()).not.toHaveAttribute('aria-disabled', 'true'))
    await openControls('chat.input.send')
    confirmControls()
    await waitFor(
      () => {
        const seed = readHomeComposerSendEffect()?.seed
        if (!seed || typeof seed !== 'object') throw new Error('seed missing')
        expect((seed as { content?: string }).content).toBe('另一份草稿')
      },
      { timeout: 8_000 }
    )
    expect(chatSends).toEqual([])
    expect(screen.queryByRole('status')).toBeNull()
    expect(opens()).toEqual([])
    expect(guestWindow).toBeNull()
    expect(fact.modelRequests).toBe(0)
    pass('nextTask')

    await act(async () => {
      publishAixCatalogue({ protocolVersion: 1, revision: '', packages: [] })
    })
    expect(screen.getByRole('radio', { name: '日常办公' })).toHaveAttribute('aria-checked', 'true')
    expect(screen.getByRole('radio', { name: '代码开发' })).toHaveAttribute('aria-checked', 'false')
    expect(screen.queryByRole('textbox', { name: '页数' })).toBeNull()
    expect(composer().querySelector('[data-workbench-chip="model"]')).toBeTruthy()
    writeTopic(composer(), '普通发送')
    fireEvent.click(sendButton())
    await act(async () => {
      await Promise.resolve()
    })
    expect(chatSends).toHaveLength(1)
    expect(chatSends[0]?.text).toContain('普通发送')
    const body = chatSends[0]?.options?.body
    expect(body).toBeTruthy()
    expect(body).not.toHaveProperty('modeId')
    expect(body).not.toHaveProperty('n_slides')
    expect(body).not.toHaveProperty('language')
    expect(body).not.toHaveProperty('tone')
    expect(body).not.toHaveProperty('verbosity')
    expect(body).not.toHaveProperty('include_title_slide')
    expect(body).not.toHaveProperty('instructions')
    expect(body?.turnContext?.research ?? false).toBe(false)
    expect(body?.turnContext?.directives ?? []).toEqual([])
    expect(readHomeComposerSendEffect()).toMatchObject({
      kind: 'chat',
      modeId: null,
      seed: null,
      research: false,
      autoReview: false,
      homeTurnDirectives: null,
      opened: null
    })
    expect(opens()).toEqual([])
    expect(fact.modelRequests).toBe(0)
    expect(guestWindow).toBeNull()
    expect(document.querySelector('[data-workbench-chip="authorize"]')).toHaveAttribute('data-posture', 'default')
    pass('backToChat')

    console.log(`oracle.passCount=${oracles.length}`)
    console.log(`oracle.modelOutlineRequests=${fact.modelRequests}`)
    expect(oracles).toEqual([
      'emptyCatalogue',
      'chatFirstSlidesSecond',
      'outlineControls',
      'blankTopic',
      'blankPageSeed',
      'enteredSeed',
      'nextTask',
      'backToChat'
    ])
  }, 90_000)

  it('stores bounded page counts and instruction length on the live homepage', async () => {
    await renderLiveHome()
    await chooseSlides()
    const root = composer()
    expect(document.querySelector('[data-ui="workbench.composer.controls"]')).toBeNull()
    const topic = root.querySelector('textarea')
    const actions = root.querySelector('[data-ui~="part:composer-actions"]')
    expect(topic && actions).toBeTruthy()
    expect(topic!.compareDocumentPosition(actions!) & Node.DOCUMENT_POSITION_FOLLOWING).toBeTruthy()
    expect(screen.getByRole('radio', { name: '日常办公' })).toHaveAttribute('aria-checked', 'false')

    const integerAlert = i18next.t('plugins.composer.integer', { label: '页数', min: 1, max: 20 })
    const lengthAlert = i18next.t('plugins.composer.max_length', { label: '补充说明', max: 4000 })

    writeTopic(root, TOPIC)
    await openControls('chat.input.send')
    expect(controlLabels()).toEqual(CONTROL_LABELS)
    expect(root.contains(document.querySelector('[data-ui="workbench.composer.controls"]'))).toBe(false)
    setPages('1')
    confirmControls()
    await waitStored(1)

    writeTopic(composer(), TOPIC)
    await openControls('chat.input.send')
    setPages('20')
    confirmControls()
    await waitStored(20)

    for (const value of ['0', '21', '1.5']) {
      writeTopic(composer(), TOPIC)
      await openControls('chat.input.send')
      setPages(value)
      confirmControls()
      await expectFieldRefusal(integerAlert, 0, composer())
    }

    setPages('1')
    setInstructions('b'.repeat(4001))
    confirmControls()
    await expectFieldRefusal(lengthAlert, 0, composer())

    setInstructions('a'.repeat(4000))
    confirmControls()
    await waitStored(1, 'a'.repeat(4000))
  }, 120_000)

  it('collects the same six controls in a send dialog on the fallback card', async () => {
    render(<FallbackHome />)
    await waitFor(() => {
      if (!(document.querySelector('[data-ui="workbench.composer.fallback"]') instanceof HTMLElement)) {
        throw new Error('fallback composer missing')
      }
    })
    await chooseSlides()
    const card = fallbackCard()
    expect(document.querySelector('[data-ui="workbench.composer"]')).toBeNull()
    expect(document.querySelector('[data-ui="workbench.composer.controls"]')).toBeNull()
    const topic = card.querySelector('textarea')
    const composerSend = screen.getByRole('button', { name: 'workbench.composer.send' })
    expect(topic).toBeTruthy()
    expect(card.contains(composerSend)).toBe(true)
    expect(screen.getByRole('radio', { name: '日常办公' })).toBeTruthy()

    const integerAlert = i18next.t('plugins.composer.integer', { label: '页数', min: 1, max: 20 })
    const lengthAlert = i18next.t('plugins.composer.max_length', { label: '补充说明', max: 4000 })

    writeTopic(card, TOPIC)
    const form = await openControls('workbench.composer.send')
    expect(card.contains(form)).toBe(false)
    expect(controlLabels()).toEqual(CONTROL_LABELS)
    for (const value of ['0', '21', '1.5']) {
      setPages(value)
      confirmControls()
      await expectFieldRefusal(integerAlert, 0, card)
    }
    setPages('1')
    setInstructions('c'.repeat(4001))
    confirmControls()
    await expectFieldRefusal(lengthAlert, 0, card)

    setInstructions('')
    setPages('1')
    confirmControls()
    await waitStored(1)
    await waitFor(() => expect(topicText(fallbackCard())).toBe(''))
    expect(fallbackCreated).toEqual(['fallback-task-1'])
    const firstDeck = render(<PresentationSessionDeck sessionId="fallback-task-1" />)
    expect(document.querySelector('[data-ui="workbench.presentation"]')?.textContent).toContain(TOPIC)
    expect(document.querySelector('[data-ui="workbench.presentation"]')?.textContent).toContain('页数：1')
    expect(opens()).toEqual([])
    firstDeck.unmount()

    writeTopic(fallbackCard(), TOPIC)
    await openControls('workbench.composer.send')
    setPages('20')
    confirmControls()
    await waitStored(20)
    await waitFor(() => expect(topicText(fallbackCard())).toBe(''))
    expect(fallbackAdopted).toBe('fallback-task-2')
    const secondDeck = render(<PresentationSessionDeck sessionId="fallback-task-2" />)
    expect(document.querySelector('[data-ui="workbench.presentation"]')?.textContent).toContain('页数：20')
    expect(opens()).toEqual([])
    secondDeck.unmount()
  }, 120_000)

  it('starts a task when the applet window would refuse delivery', async () => {
    await renderLiveHome()
    await chooseSlides()
    writeTopic(composer(), TOPIC)
    openDelivery = 0
    await openControls('chat.input.send')
    confirmControls()
    await waitFor(() =>
      expect(readHomeComposerSendEffect()).toMatchObject({
        kind: 'chat',
        modeId: 'slides',
        opened: null,
        seed: { content: TOPIC, n_slides: null }
      })
    )
    expect(chatSends).toEqual([])
    expect(opens()).toEqual([])
    expect(guestWindow).toBeNull()
    const refused = render(<PresentationSessionDeck sessionId="session-1" />)
    expect(document.querySelector('[data-ui="workbench.presentation"]')?.textContent).toContain(TOPIC)
    refused.unmount()

    cleanup()
    openDelivery = null
    resetHomeComposerModeForTests()
    chatSends.length = 0
    invokes.length = 0
    render(<FallbackHome />)
    await waitFor(() => {
      if (!(document.querySelector('[data-ui="workbench.composer.fallback"]') instanceof HTMLElement)) {
        throw new Error('fallback composer missing')
      }
    })
    await chooseSlides()
    writeTopic(fallbackCard(), TOPIC)
    openDelivery = 0
    await openControls('workbench.composer.send')
    confirmControls()
    await waitFor(() => {
      expect(readHomeComposerSendEffect()).toMatchObject({
        kind: 'chat',
        modeId: 'slides',
        opened: null,
        seed: { content: TOPIC, n_slides: null }
      })
    })
    expect(opens()).toEqual([])
    expect(guestWindow).toBeNull()
    expect(chatSends).toEqual([])
    await waitFor(() => expect(topicText(fallbackCard())).toBe(''))
    expect(fallbackAdopted).toBe('fallback-task-1')
    const fallbackDeck = render(<PresentationSessionDeck sessionId="fallback-task-1" />)
    expect(document.querySelector('[data-ui="workbench.presentation"]')?.textContent).toContain(TOPIC)
    fallbackDeck.unmount()
  }, 30_000)

  function draftKeptCatalogue() {
    const notes = notesCatalogue()
    const pack = notes.packages[0]
    const mode = pack?.composerModes?.[0]
    if (!pack || !mode) throw new Error('notes')
    return {
      ...notes,
      packages: [
        {
          ...pack,
          slug: 'a3s-drafts',
          packageId: 'use/a3s/drafts',
          composerModes: [{ ...mode, id: 'drafts', title: '草稿', keptNotice: 'draft' as const }]
        }
      ]
    }
  }

  function notesCatalogue() {
    return aixWorkbenchCatalogueSchema.parse({
      protocolVersion: 1,
      revision: '',
      packages: [
        {
          slug: 'a3s-notes',
          packageId: 'use/a3s/notes',
          generation: 1,
          revision: `sha256:${'cd'.repeat(32)}`,
          containers: [],
          views: [
            {
              id: 'workbench',
              title: '笔记',
              description: '记录',
              icon: 'files',
              order: 100,
              container: null,
              point: 'studio.applets',
              presentation: 'webview'
            }
          ],
          composerModes: [
            {
              id: 'notes',
              contractVersion: 1,
              title: '笔记',
              description: '记录',
              icon: 'files',
              agent: 'notes',
              editor: 'workbench',
              scope: 'workspace',
              context: ['workspace'],
              order: 30,
              prompt: '输入笔记'
            }
          ]
        }
      ]
    })
  }

  it('clears stored control values when the member returns to 对话', async () => {
    await renderLiveHome()
    const title = await chooseSlides()
    writeTopic(composer(), TOPIC)
    await openControls('chat.input.send')
    setPages('8')
    fireEvent.click(screen.getByRole('switch', { name: '标题页，开' }))
    expect(screen.getByRole('textbox', { name: '页数' })).toHaveValue('8')
    expect(screen.getByRole('switch', { name: '标题页，关' })).toHaveAttribute('aria-checked', 'false')
    cancelControls()
    fireEvent.click(screen.getByRole('radio', { name: '日常办公' }))
    expect(screen.getByRole('radio', { name: '日常办公' })).toHaveAttribute('aria-checked', 'true')
    expect(screen.queryByRole('textbox', { name: '页数' })).toBeNull()
    fireEvent.click(screen.getByRole('radio', { name: title }))
    await openControls('chat.input.send')
    expect(screen.getByRole('textbox', { name: '页数' })).toHaveValue('')
    expect(controlValue('n_slides')).toBe('')
    expect(screen.getByRole('switch', { name: '标题页，开' })).toHaveAttribute('aria-checked', 'true')
    cancelControls()
  })

  it('shows catalog words for the empty page count and the toggle', async () => {
    const zh = localeCatalog('zh-cn')
    const en = localeCatalog('en-us')
    expect(zh['plugins.composer.auto']).toBe('自动')
    expect(zh['plugins.composer.on']).toBe('开')
    expect(zh['plugins.composer.off']).toBe('关')
    expect(en['plugins.composer.auto']).toBe('Auto')
    expect(en['plugins.composer.on']).toBe('On')
    expect(en['plugins.composer.off']).toBe('Off')
    await renderLiveHome()
    await chooseSlides()
    writeTopic(composer(), TOPIC)
    await openControls('chat.input.send')
    expect(screen.getByRole('button', { name: '自动', exact: true })).toBeTruthy()
    expect(screen.getByRole('switch', { name: '标题页，开' })).toHaveAttribute('aria-checked', 'true')
    fireEvent.click(screen.getByRole('switch', { name: '标题页，开' }))
    expect(screen.getByRole('switch', { name: '标题页，关' })).toHaveAttribute('aria-checked', 'false')
    try {
      await act(async () => {
        await i18next.changeLanguage('en-US')
      })
      expect(screen.getByRole('button', { name: 'Auto', exact: true })).toBeTruthy()
      expect(screen.getByRole('button', { name: 'Auto', exact: true }).textContent ?? '').not.toContain('自动')
      expect(screen.getByRole('switch', { name: '标题页，Off' })).toHaveAttribute('aria-checked', 'false')
      fireEvent.click(screen.getByRole('switch', { name: '标题页，Off' }))
      expect(screen.getByRole('switch', { name: '标题页，On' })).toHaveAttribute('aria-checked', 'true')
    } finally {
      await act(async () => {
        await i18next.changeLanguage('zh-CN')
      })
    }
    expect(screen.getByRole('button', { name: '自动', exact: true })).toBeTruthy()
    cancelControls()
  })

  it('chooses both keep sentences without comparing a package id', async () => {
    const source = readFileSync(join(dirname(fileURLToPath(import.meta.url)), '../homeComposerMode.tsx'), 'utf8')
    expect(source.includes('use/a3s/presenton')).toBe(false)
    const draftSentence = localeCatalog('zh-cn')['plugins.composer.draft_kept']
    const editorSentence = localeCatalog('zh-cn')['plugins.composer.editor_draft_kept']
    expect(draftSentence).toBe('当前演示文稿还在，首页草稿未写入')
    expect(editorSentence).toBe('编辑器中已有内容，首页草稿未写入')
    await renderLiveHome()
    expect(presentonCatalogue().packages[0]?.composerModes?.[0]?.keptNotice).toBe('draft')
    fact = { delivery: 1, holdsWork: true, applied: true, seed: null, notice: null, modelRequests: 0 }
    await act(async () => {
      publishAixCatalogue(draftKeptCatalogue())
    })
    fireEvent.click(screen.getByRole('radio', { name: '草稿' }))
    writeTopic(composer(), TOPIC)
    fireEvent.click(screen.getByRole('button', { name: 'chat.input.send' }))
    await waitFor(() => expect(screen.getByRole('status')).toHaveTextContent(draftSentence ?? ''))
    expect(opens()).toEqual([])
    expect(topicText(composer())).toBe(TOPIC)
    await act(async () => {
      publishAixCatalogue(notesCatalogue())
    })
    fireEvent.click(screen.getByRole('radio', { name: '笔记' }))
    writeTopic(composer(), TOPIC)
    fireEvent.click(screen.getByRole('button', { name: 'chat.input.send' }))
    await waitFor(() => expect(screen.getByRole('status')).toHaveTextContent(editorSentence ?? ''))
    expect(opens()).toEqual([])
    expect(topicText(composer())).toBe(TOPIC)
  })

  it('keeps an outlined deck on the packaged create admission', async () => {
    guestDocument = startGuestDocument()
    guestWindow = guestDocument.window as Window & { __a3sPresentonTesting?: ViewTesting }
    const frame = guestWindow
    const hook = viewTesting()
    const create = (content: string) =>
      frame.fetch('/api/v1/ppt/presentation/create', {
        method: 'POST',
        body: JSON.stringify({
          content,
          n_slides: 4,
          language: SIMPLIFIED_CHINESE,
          tone: 'default',
          verbosity: 'standard',
          include_title_slide: true
        })
      })
    const first = await create('空稿主题')
    expect(first.status).toBe(200)
    const created = (await first.json()) as { id?: string; content?: string }
    expect(created.content).toBe('空稿主题')
    expect(typeof created.id).toBe('string')
    expect(hook.snapshot()).toMatchObject({ content: '空稿主题', outlines: null, modelRequests: 0 })
    const again = await create('另一主题')
    expect(again.status).toBe(200)
    const replaced = (await again.json()) as { id?: string; content?: string }
    expect(replaced.content).toBe('另一主题')
    expect(replaced.id).not.toBe(created.id)
    hook.setOutlines(['# Kept'])
    const refused = await create('替换主题')
    expect(refused.status).toBe(409)
    expect(((await refused.json()) as { detail?: string }).detail).toBe('当前演示文稿还在，首页草稿未写入')
    expect(hook.snapshot()).toMatchObject({ content: '另一主题', outlines: ['# Kept'], modelRequests: 0 })
    const kept = await frame.fetch(`/api/v1/ppt/presentation/${replaced.id}`)
    expect(kept.status).toBe(200)
    const body = (await kept.json()) as { id?: string; content?: string }
    expect(body.id).toBe(replaced.id)
    expect(body.content).toBe('另一主题')
  }, 30_000)
})
