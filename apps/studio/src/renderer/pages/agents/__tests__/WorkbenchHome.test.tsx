import { act, fireEvent, render, screen, waitFor, within } from '@testing-library/react'
import { PDFDocument, StandardFonts } from 'pdf-lib'
import { type ComponentProps, useState } from 'react'
import { createPortal } from 'react-dom'
import { beforeEach, describe, expect, it, vi } from 'vitest'

import {
  getAgentDraftCacheKey,
  readAgentDraftCache,
  writeAgentDraftCache
} from '@renderer/components/composer/variants/agent/agentDraftCache'
import { subscribeWorkbenchDroppedFiles } from '@renderer/components/composer/workbenchDroppedFiles'
import { PERSONAL_CENTER } from '@renderer/data/memberOrgs'
import { driveApi, resetDriveByteBaseForTests } from '@renderer/drive/driveTransport'
import enUs from '@renderer/i18n/locales/en-us.json'
import zhCn from '@renderer/i18n/locales/zh-cn.json'
import { dataWorkbenchCatalogue } from '@renderer/pages/data/dataPackageCatalogue'
import { OPEN_MAIN_ROUTE_EVENT } from '@renderer/services/mainWindowNavigation'
import { publishAixCatalogue } from '@renderer/services/plugins/aixCatalogueStore'
import { toast } from '@renderer/services/toast'
import type { AixWorkbenchCatalogue } from '@shared/types/aixWorkbench'

import { resetPluginInstallOverlayForTests } from '../../capabilities/pluginInstallSync'
import { APPLET_PERMISSION_GRANT_KEY } from '../appletPermissionGrant'
import { HOME_APPLET_PINS_KEY } from '../homeApplets'
import { readCodeSession } from '../codeMode'
import { resetHomeComposerModeForTests } from '../homeComposerMode'
import { writeHomePluginPins } from '../homePins'
import { appletPackageCacheKey, sha256Hex } from '../loadAppletPackage'
import { APPLET_PROFILE_GRANT_KEY } from '../loadAppletProfile'
import { isOfficialWritingHandoff, OFFICIAL_WRITING_EVENT } from '../officialWriting'
import { readOfficialWritingSession, resetOfficialWritingSessionForTests } from '../officialWritingAgentEdit'
import {
  openOfficialWritingTask,
  readOfficialWritingFile,
  resetOfficialWritingFileForTests
} from '../officialWritingWorkspaceFile'
import { openWorkbenchAppletWindow } from '../openWorkbenchAppletWindow'
import { PDF_TOOL_KIND_KEY } from '../pdfToolsJob'
import { encodePng, solid } from '../pdfToolsPng'
import { setReviewPluginForTests } from '../reviewPlugin'
import { WorkbenchApplets } from '../WorkbenchApplets'
import { WorkbenchAppletWindow } from '../WorkbenchAppletWindow'
import { runAppletWindowAction } from '../workbenchAppletWindowBridge'
import { registerWorkbenchComposer } from '../workbenchComposerBridge'
import { peekPendingHomeSubmit } from '../workbenchComposerHost'
import { WorkbenchHome, highlightQuery, matchesQuery, type WorkbenchSession } from '../WorkbenchHome'

const capabilityApi = vi.hoisted(() => ({
  listCapabilities: vi.fn(),
  installCapability: vi.fn()
}))

vi.mock('../../capabilities/capabilitiesData', async (importOriginal) => {
  const actual = await importOriginal<typeof import('../../capabilities/capabilitiesData')>()
  return { ...actual, ...capabilityApi }
})

setReviewPluginForTests({ skill: true, ui: true, title: '审查' })

vi.mock('@renderer/services/toast', () => ({
  toast: { info: vi.fn(), success: vi.fn(), error: vi.fn(), warning: vi.fn() }
}))

vi.mock('../openWorkbenchAppletWindow', () => ({
  openWorkbenchAppletWindow: vi.fn(() => Promise.resolve(true))
}))

vi.mock('@a3s-lab/office/react', async () => {
  const { useEffect, useState } = await import('react')
  function readBlob(blob: Blob): Promise<ArrayBuffer> {
    if (typeof blob.arrayBuffer === 'function') return blob.arrayBuffer()
    return new Promise((resolve, reject) => {
      const reader = new FileReader()
      reader.onload = () => resolve(reader.result as ArrayBuffer)
      reader.onerror = () => reject(reader.error ?? new Error('unreadable'))
      reader.readAsArrayBuffer(blob)
    })
  }

  return {
    PdfViewer: ({ fileName, loadSource }: { fileName?: string; loadSource: () => Promise<Blob> }) => {
      const [mark, setMark] = useState('')
      useEffect(() => {
        let cancelled = false
        void loadSource().then(async (blob) => {
          const bytes = new Uint8Array(await readBlob(blob))
          if (cancelled) return
          const head = String.fromCharCode(...bytes.slice(0, 5))
          setMark(`${bytes.byteLength}:${head}`)
        })
        return () => {
          cancelled = true
        }
      }, [loadSource])
      return (
        <div data-ui="office.pdf-viewer" data-file={fileName}>
          {mark}
        </div>
      )
    },
    DocumentEditor: ({
      content,
      preview,
      onEditorReady
    }: {
      content: { html: string }
      preview?: boolean
      onEditorReady?: (editor: { commands: { setContent: (html: string) => void } }) => void
    }) => {
      const [html, setHtml] = useState(content.html)
      const [seen, setSeen] = useState(content.html)
      if (content.html !== seen) {
        setSeen(content.html)
        setHtml(content.html)
      }
      useEffect(() => {
        onEditorReady?.({
          commands: {
            setContent(next: string) {
              setHtml(next)
            }
          }
        })
      }, [onEditorReady])
      return <div data-office-preview={preview ? 'true' : 'false'}>{html}</div>
    }
  }
})

vi.mock('@a3s-lab/office/styles.css', () => ({}))

vi.mock('@a3s-lab/office/core', () => ({
  createArtifact: () => ({
    id: 'artifact-1',
    kind: 'document',
    title: '无标题文字',
    content: { type: 'document', html: '<p></p>', pageSize: 'a4' }
  }),
  createArtifactBlob: async () => ({
    arrayBuffer: async () => Uint8Array.from([0x50, 0x4b, 3, 4]).buffer
  })
}))

vi.mock('@renderer/pages/documents/useLocalDocuments', () => ({
  createLocalEntry: vi.fn(async (_root: string, _parent: string, name: string) => ({
    name,
    path: `/tmp/workspace/${name}`,
    kind: 'file',
    sizeBytes: 1,
    modifiedMs: 0
  })),
  writeLocalDocumentBytes: vi.fn(async () => undefined),
  removeLocalEntry: vi.fn(async () => undefined),
  localEntryStatus: (error: unknown) =>
    error && typeof error === 'object' && 'status' in error ? Number((error as { status: number }).status) : 0
}))

vi.mock('../workbenchAppletWindowBridge', () => ({
  runAppletWindowAction: vi.fn(() => Promise.resolve(true))
}))

vi.mock('../../settings/TasksSettings', () => ({
  default: ({ onCreateWithAgent }: { onCreateWithAgent?: () => void }) => (
    <section data-ui="workbench.automation">
      <button type="button" onClick={onCreateWithAgent}>
        settings.scheduledTasks.agentCreate
      </button>
    </section>
  )
}))

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

vi.mock('react-i18next', () => ({
  initReactI18next: { type: '3rdParty', init: vi.fn() },
  useTranslation: () => ({
    t: (key: string) => key
  })
}))

const APPLET_RUNTIME = {
  apiBaseUrl: 'http://127.0.0.1:9',
  appletTicket: 'ab'.repeat(32),
  appId: 'leave',
  tenant: 'toyou'
}

const CACHE_KEY = appletPackageCacheKey('toyou')

function installAppletRuntime(appId = 'leave', permissions?: string[]) {
  localStorage.removeItem(APPLET_PROFILE_GRANT_KEY)
  localStorage.removeItem(APPLET_PERMISSION_GRANT_KEY)
  const declared = permissions ?? (appId === 'leave' ? ['user'] : undefined)
  Object.defineProperty(globalThis, '__A3S_APPLET_RUNTIME__', {
    value: { ...APPLET_RUNTIME, appId, ...(declared ? { permissions: declared } : {}) },
    configurable: true
  })
}

function removeAppletRuntime() {
  Reflect.deleteProperty(globalThis, '__A3S_APPLET_RUNTIME__')
  localStorage.removeItem(CACHE_KEY)
  localStorage.removeItem(APPLET_PROFILE_GRANT_KEY)
  localStorage.removeItem(APPLET_PERMISSION_GRANT_KEY)
}

const LEAVE_AVATAR =
  'data:image/png;base64,iVBORw0KGgoAAAANSUhEUgAAAAEAAAABCAYAAAAfFcSJAAAADUlEQVR42mP8z8BQDwAEhQGAhKmMIQAAAABJRU5ErkJggg=='

const PACKAGE_BODY = JSON.stringify({
  appId: 'leave',
  version: '1.1.0',
  entry: 'pages/index/index',
  pages: ['pages/index/index'],
  window: { navigationBarTitleText: '请假' },
  fields: [
    { name: 'title', kind: 'text', label: '标题', required: true, max: 80 },
    { name: 'detail', kind: 'textarea', label: '说明', required: true, max: 2000 }
  ]
})

function appletFetch(
  digest: string,
  options: {
    signedIn?: boolean
    hang?: boolean
    submitFails?: boolean
    body?: string
    centerId?: string
    approvers?: { id: string; name: string }[]
    preview?: (body: string) => { status?: number; payload: unknown }
  } = {}
) {
  const packageBody = options.body ?? PACKAGE_BODY
  return vi.fn((url: string, init?: RequestInit) => {
    if (options.hang) return new Promise<Response>(() => {})
    const target = String(url)
    if (target.endsWith('/session')) {
      return Promise.resolve(
        new Response(
          JSON.stringify({ signedIn: options.signedIn !== false, displayName: '林', phone: '13800138000' }),
          { status: 200 }
        )
      )
    }
    if (target.endsWith('/profile')) {
      return Promise.resolve(
        new Response(JSON.stringify({ displayName: '林', avatar: LEAVE_AVATAR, phone: '13800138000' }), { status: 200 })
      )
    }
    if (options.centerId && target.endsWith('/center')) {
      return Promise.resolve(new Response(JSON.stringify({ id: options.centerId }), { status: 200 }))
    }
    if (target.endsWith('/package/release')) return Promise.resolve(new Response(packageBody, { status: 200 }))
    if (target.endsWith('/work-items/approvers')) {
      return Promise.resolve(
        new Response(JSON.stringify(options.approvers ?? [{ id: 'a1', name: '周' }]), { status: 200 })
      )
    }
    if (target.endsWith('/work-items/travel-links')) {
      return Promise.resolve(new Response(JSON.stringify([]), { status: 200 }))
    }
    if (init?.method === 'POST' && target.endsWith('/work-items/process-preview')) {
      const raw = String(init.body ?? '')
      const custom = options.preview?.(raw)
      const processKey = (JSON.parse(raw) as { process_key?: string }).process_key
      const fallback =
        processKey === 'clock'
          ? { version: 0, nodes: [] }
          : { version: 1, nodes: [{ key: 'manager', name: '直属主管', assignees: [] }] }
      return Promise.resolve(
        new Response(JSON.stringify(custom?.payload ?? fallback), { status: custom?.status ?? 200 })
      )
    }
    if (init?.method === 'POST' && target.endsWith('/work-items/process')) {
      return Promise.resolve(
        new Response(JSON.stringify({ id: 'wi-1', title: 'item', status: 'pending', kind: 'approval' }), {
          status: 200
        })
      )
    }
    if (init?.method === 'POST' && target.endsWith('/work-items')) {
      return Promise.resolve(
        new Response(JSON.stringify({ id: 'wi-1', title: 'item', status: 'pending', kind: 'approval' }), {
          status: 200
        })
      )
    }
    if (init?.method === 'POST') {
      return Promise.resolve(
        new Response(options.submitFails ? 'no' : JSON.stringify({ id: '1' }), {
          status: options.submitFails ? 400 : 200
        })
      )
    }
    return Promise.resolve(
      new Response(JSON.stringify({ release: { version: '1.1.0', digest }, trial: null }), { status: 200 })
    )
  })
}

function session(id: string, updatedAt: string, pinned = false, name = id): WorkbenchSession {
  return { id, name, updatedAt, pinned }
}

function pointAt(node: Element) {
  const original = Document.prototype.elementFromPoint
  Document.prototype.elementFromPoint = () => node
  return () => {
    Document.prototype.elementFromPoint = original
  }
}

function sendPointer(target: EventTarget, type: string, x: number, y: number) {
  const event = new MouseEvent(type, { bubbles: true, cancelable: true, button: 0, clientX: x, clientY: y })
  Object.defineProperty(event, 'pointerId', { value: 1 })
  act(() => {
    target.dispatchEvent(event)
  })
}

async function chooseMarkedDay(marker: string, iso: string) {
  const day = await waitFor(() => {
    const button = document.querySelector(`[data-ui="${marker}"] [data-day="${iso}"] button`)
    if (!(button instanceof HTMLButtonElement)) throw new Error(`missing ${iso}`)
    return button
  })
  fireEvent.click(day)
}

async function chooseLeaveDay(iso: string) {
  await chooseMarkedDay('workbench.applet.leave.range', iso)
}

async function chooseOption(label: string, option: string) {
  const combo = screen.getByRole('combobox', { name: label })
  fireEvent.pointerDown(combo)
  fireEvent.click(combo)
  fireEvent.click(await screen.findByRole('option', { name: option }))
}

function renderHome(overrides: Partial<ComponentProps<typeof WorkbenchHome>> = {}) {
  const props = {
    sessions: [] as WorkbenchSession[],
    activeSessionId: null,
    homeActive: true,
    onNewTask: vi.fn(),
    onOpenSession: vi.fn(),
    onTogglePin: vi.fn(),
    onDeleteSession: vi.fn(),
    onRenameSession: vi.fn(),
    ...overrides
  }
  render(<WorkbenchHome {...props} />)
  return props
}

function renderStrip() {
  return render(<WorkbenchApplets />)
}

function finishLaunchpadClose() {
  const stage = document.querySelector('[data-ui="workbench.applet.launcher.stage"]')
  if (!(stage instanceof HTMLElement)) throw new Error('Expected the launchpad to be closing')
  const event = new Event('animationend', { bubbles: true })
  Object.defineProperty(event, 'animationName', { value: 'workbench-launchpad-out' })
  act(() => {
    stage.dispatchEvent(event)
  })
}

function fileBytes(blob: Blob): Promise<Uint8Array> {
  if (typeof blob.arrayBuffer === 'function') return blob.arrayBuffer().then((buffer) => new Uint8Array(buffer))
  return new Promise((resolve, reject) => {
    const reader = new FileReader()
    reader.onload = () => resolve(new Uint8Array(reader.result as ArrayBuffer))
    reader.onerror = () => reject(reader.error ?? new Error('unreadable'))
    reader.readAsArrayBuffer(blob)
  })
}

const REVISION = `sha256:${'ab'.repeat(32)}`

function osHomeCatalogue(): AixWorkbenchCatalogue {
  const pack = (slug: string, home: AixWorkbenchCatalogue['packages'][number]['home']) => ({
    slug,
    packageId: `use/a3s/${slug}`,
    generation: 1,
    revision: REVISION,
    containers: [],
    views: [],
    home
  })
  return {
    protocolVersion: 1,
    revision: '',
    packages: [
      pack('home-writing', [
        {
          id: 'notice',
          shelf: 'write',
          title: 'workbench.writing.card.notice_meeting.title',
          description: '发布、传达要求周知或执行的事项',
          order: 10,
          action: 'writing',
          writingType: 'notice',
          seed: 'workbench.feature.write.blessing',
          tag: '通知'
        },
        {
          id: 'report',
          shelf: 'write',
          title: 'workbench.writing.card.summary_quarter.title',
          description: '向上级汇报工作、反映情况',
          order: 20,
          action: 'writing',
          writingType: 'report',
          seed: 'workbench.feature.write.career',
          tag: '报告'
        },
        {
          id: 'request',
          shelf: 'write',
          title: 'workbench.writing.card.request_funding.title',
          description: '向上级请求指示或批准',
          order: 30,
          action: 'writing',
          writingType: 'request',
          seed: 'workbench.feature.write.plan',
          tag: '请示'
        },
        {
          id: 'minutes',
          shelf: 'write',
          title: 'workbench.writing.card.minutes_director.title',
          description: '记载会议主要情况和议定事项',
          order: 40,
          action: 'writing',
          writingType: 'minutes',
          seed: 'workbench.feature.write.review',
          tag: '纪要'
        }
      ]),
      pack('home-create', [
        {
          id: 'ppt',
          shelf: 'create',
          title: 'workbench.feature.create.ppt',
          description: '',
          order: 10,
          action: 'unavailable',
          cover: 'ppt'
        },
        {
          id: 'site',
          shelf: 'create',
          title: 'workbench.feature.create.site',
          description: '',
          order: 20,
          action: 'unavailable',
          cover: 'site'
        },
        {
          id: 'video',
          shelf: 'create',
          title: 'workbench.feature.create.video',
          description: '',
          order: 30,
          action: 'unavailable',
          cover: 'video'
        },
        {
          id: 'image',
          shelf: 'create',
          title: 'workbench.feature.create.image',
          description: '',
          order: 40,
          action: 'applet',
          applet: 'plan',
          cover: 'image'
        }
      ]),
      pack('home-process', [
        {
          id: 'pdf-to-word',
          shelf: 'process',
          title: 'workbench.feature.pdf.edit.title',
          description: 'workbench.feature.pdf.edit.body',
          order: 10,
          action: 'file',
          fileKind: 'pdf-to-word',
          format: 'pdf'
        },
        {
          id: 'word-to-pdf',
          shelf: 'process',
          title: 'workbench.feature.pdf.translate.title',
          description: 'workbench.feature.pdf.translate.body',
          order: 20,
          action: 'file',
          fileKind: 'word-to-pdf',
          format: 'word'
        }
      ])
    ]
  }
}

describe('WorkbenchHome', () => {
  beforeEach(() => {
    publishAixCatalogue(osHomeCatalogue())
    if (!HTMLElement.prototype.hasPointerCapture) HTMLElement.prototype.hasPointerCapture = () => false
    if (!HTMLElement.prototype.releasePointerCapture) HTMLElement.prototype.releasePointerCapture = () => {}
    if (!HTMLElement.prototype.setPointerCapture) HTMLElement.prototype.setPointerCapture = () => {}
    document.body.style.cursor = ''
    localStorage.removeItem(HOME_APPLET_PINS_KEY)
    localStorage.removeItem(PDF_TOOL_KIND_KEY)
    localStorage.removeItem(APPLET_PERMISSION_GRANT_KEY)
    localStorage.removeItem('a3s_org_member_token')
    localStorage.removeItem('a3s.org.center')
    writeHomePluginPins(null, ['home-writing', 'home-create', 'home-process'])
    resetDriveByteBaseForTests()
    resetPluginInstallOverlayForTests()
    capabilityApi.listCapabilities.mockReset()
    capabilityApi.listCapabilities.mockResolvedValue({
      items: [],
      total: 0,
      page: 1,
      pageSize: 100,
      categories: []
    })
    capabilityApi.installCapability.mockReset()
    capabilityApi.installCapability.mockResolvedValue(undefined)
  })

  it('keeps the home plugin grid when the analysis work mode is selected', () => {
    const home = osHomeCatalogue()
    publishAixCatalogue({
      ...home,
      packages: [...home.packages, ...dataWorkbenchCatalogue().packages]
    })
    renderHome()
    fireEvent.click(screen.getByRole('radio', { name: '数据分析' }))
    expect(document.querySelector('[data-ui="data.analysis"]')).toBeNull()
    expect(document.querySelector('[data-ui="data.browser"]')).toBeNull()
    expect(document.querySelectorAll('[data-ui="workbench.feature"]')).toHaveLength(3)
    const tasks = document.querySelector('[data-ui="workbench.composer.tasks"]')
    const fallback = document.querySelector('[data-ui="workbench.composer.fallback"]')
    expect(tasks).toBeInstanceOf(HTMLElement)
    expect(fallback?.contains(tasks)).toBe(false)
    expect(within(tasks as HTMLElement).getByRole('button', { name: '查询数据源' })).toBeInTheDocument()
    const canvas = document.querySelector('[data-ui="workbench.canvas"]')
    const features = canvas?.querySelector('[data-ui="workbench.features"]')
    expect(features).toBeInstanceOf(HTMLElement)
    expect(canvas?.querySelector('[data-ui="data.analysis"]')).toBeNull()
  })

  it('leaves the home shelves out until the OS packages are installed', () => {
    publishAixCatalogue({ protocolVersion: 1, revision: '', packages: [] })
    renderHome()
    expect(document.querySelector('[data-ui="workbench.features"]')).toBeNull()
    expect(document.querySelector('[data-ui="workbench.applets"]')).toBeNull()
    expect(document.querySelectorAll('[data-ui="workbench.feature"]')).toHaveLength(0)
  })

  it('matches the workbench landing without a mascot', () => {
    document.documentElement.dataset.a3sProduct = 'ToWork'
    renderHome()
    const wordmark = screen.getByRole('heading', { name: 'ToWork' })
    expect(wordmark).toHaveClass('workbench-wordmark', 'text-center', 'mb-5')
    expect(wordmark.querySelector('[data-ui="workbench.wordmark.mark"]')).toHaveClass('h-12')
    expect(document.querySelector('[data-ui="workbench.experts"]')).toBeNull()
    delete document.documentElement.dataset.a3sProduct

    const home = document.querySelector('[data-ui="workbench.home"]')
    expect(home).toBeTruthy()
    expect(home?.querySelector('img:not([data-ui="workbench.feature-thumb"])')).toBeNull()
    expect(document.querySelector('[data-ui="workbench.canvas"] > div')).toHaveClass('min-h-full', 'shrink-0')
    expect(document.querySelectorAll('[data-ui="workbench.feature"]')).toHaveLength(3)
    expect(document.querySelector('[data-ui="workbench.feature.write.grid"]')).toHaveClass('grid-cols-2')
    expect(document.querySelector('[data-home="minutes"] span')).not.toHaveClass('whitespace-nowrap')
    expect(document.querySelector('[data-feature="create"]')).toBeTruthy()
    expect(document.querySelector('[data-ui="workbench.feature.create.grid"]')).toHaveAttribute(
      'data-layout',
      'collage'
    )
    expect(document.querySelectorAll('[data-feature="create"] [data-cover]')).toHaveLength(4)
    expect(document.querySelector('[data-cover="site"] [data-art="app"]')).toBeTruthy()
    expect(document.querySelector('[data-cover="site"] circle')).toBeNull()
    expect(document.querySelector('[data-cover="video"] [data-art="interface"]')).toBeTruthy()
    expect(document.querySelector('[data-cover="image"] [data-art="plan"]')).toBeTruthy()
    expect(document.querySelector('[data-cover="video"]')).toHaveTextContent('workbench.feature.create.video')
    expect(document.querySelector('[data-cover="image"]')).toHaveTextContent('workbench.feature.create.image')
    expect(document.querySelector('[data-ui="workbench.projects"]')).toBeTruthy()
    expect(document.querySelector('[data-ui="workbench.tasks"]')).toBeTruthy()
    expect(document.querySelector('[data-ui="workbench.sessions.empty"]')).toHaveTextContent('workbench.projects.empty')
    expect(document.querySelector('[data-ui="workbench.tasks.empty"]')).toHaveTextContent('workbench.tasks.empty')
    expect(screen.queryByRole('button', { name: 'workbench.nav.plugins' })).not.toBeInTheDocument()
    expect(screen.queryByRole('button', { name: 'workbench.nav.extensions' })).not.toBeInTheDocument()
    expect(screen.queryByText('workbench.nav.shortcut')).not.toBeInTheDocument()
    expect(screen.queryByRole('searchbox', { name: 'workbench.search' })).not.toBeInTheDocument()
    expect(screen.getByRole('button', { name: 'workbench.search' })).toHaveAttribute('aria-expanded', 'false')
    expect(screen.queryByRole('button', { name: 'workbench.nav.experts' })).toBeNull()
    const connectors = screen.getByRole('button', { name: 'workbench.nav.connectors' })
    const skills = screen.getByRole('button', { name: 'workbench.nav.skills' })
    const newTask = document.querySelector('[data-new-task-trigger="true"]')!
    expect(newTask.compareDocumentPosition(connectors) & Node.DOCUMENT_POSITION_FOLLOWING).toBeTruthy()
    expect(connectors.compareDocumentPosition(skills) & Node.DOCUMENT_POSITION_FOLLOWING).toBeTruthy()
    const automation = screen.getByRole('button', { name: 'workbench.nav.scheduled' })
    expect(skills.compareDocumentPosition(automation) & Node.DOCUMENT_POSITION_FOLLOWING).toBeTruthy()
    expect(automation.querySelector('svg')).toHaveClass('lucide-clock')
    expect(screen.queryByRole('button', { name: 'workbench.nav.encyclopedia' })).not.toBeInTheDocument()
    expect(connectors).toHaveClass('h-9')
    expect(skills).toHaveClass('h-9')
    expect(document.querySelector('[data-ui="workbench.nav"]')).toHaveClass('mt-3')
    expect(document.querySelector('[data-new-task-trigger="true"]')).toHaveClass(
      'h-9',
      'bg-[#f5f5f5]',
      'text-[#171717]'
    )
    expect(screen.getByRole('button', { name: 'workbench.search' })).toHaveClass('bg-[#f5f5f5]', 'hover:bg-[#ececec]')
    expect(screen.queryByRole('separator', { name: 'common.resize_panel' })).not.toBeInTheDocument()
    expect(screen.queryByText('workbench.composer.docs')).not.toBeInTheDocument()
    expect(screen.queryByRole('button', { name: 'settings.scheduledTasks.title' })).not.toBeInTheDocument()
    expect(screen.queryByRole('button', { name: 'settings.nav.archivedTasks' })).not.toBeInTheDocument()
    expect(document.querySelectorAll('[data-project-action="create"] path')).toHaveLength(2)
    expect(document.querySelector('[data-ui="workbench.tasks.more"]')).toHaveTextContent('workbench.tasks.more')
    expect(screen.getByRole('textbox')).toBeInTheDocument()
    expect(screen.getByRole('textbox').parentElement).toHaveClass('shadow-[0_12px_28px_rgba(32,42,68,0.08)]')
    expect(document.querySelector('[data-ui="workbench.applets"]')).toBeNull()
    const features = document.querySelector('[data-ui="workbench.features"]')
    expect(features).toBeTruthy()
    expect(
      screen.getByRole('textbox').compareDocumentPosition(features!) & Node.DOCUMENT_POSITION_FOLLOWING
    ).toBeTruthy()
    expect(features).toHaveClass('mt-20')
    expect(document.querySelector('[data-new-task-trigger="true"]')).toBeTruthy()
  })

  it('installs an office applet from the strip and opens its window', async () => {
    vi.mocked(openWorkbenchAppletWindow).mockClear()
    vi.mocked(toast.info).mockClear()
    renderStrip()

    fireEvent.click(screen.getByRole('button', { name: 'workbench.applet.approval' }))

    await waitFor(() =>
      expect(capabilityApi.installCapability).toHaveBeenCalledWith(
        'a3s-approval',
        expect.any(Function),
        expect.any(Function)
      )
    )
    await waitFor(() => expect(openWorkbenchAppletWindow).toHaveBeenCalledWith('approval', 'workbench.applet.approval'))
    expect(toast.info).not.toHaveBeenCalled()
    await waitFor(() =>
      expect(screen.getByRole('button', { name: 'workbench.applet.approval' })).toHaveAttribute(
        'data-installed',
        'true'
      )
    )
  })

  it('opens a pinned applet from a short press that is not a drag', async () => {
    vi.mocked(openWorkbenchAppletWindow).mockClear()
    renderStrip()
    const button = screen.getByRole('button', { name: 'workbench.applet.approval' })
    const host = button.parentElement as HTMLElement
    const previousPointer = window.PointerEvent
    if (typeof window.PointerEvent !== 'function') {
      window.PointerEvent = class extends MouseEvent {
        pointerId: number
        constructor(type: string, init: MouseEventInit & { pointerId?: number } = {}) {
          super(type, init)
          this.pointerId = init.pointerId ?? 1
        }
      } as unknown as typeof PointerEvent
    }
    try {
      fireEvent.pointerDown(host, { clientX: 4, clientY: 4, button: 0, pointerId: 1 })
      fireEvent.pointerMove(host, { clientX: 6, clientY: 5, pointerId: 1 })
      expect(document.querySelector('[data-ui="workbench.applet.drag"]')).toBeNull()
      fireEvent.pointerUp(host, { clientX: 6, clientY: 5, button: 0, pointerId: 1 })
      fireEvent.click(button)
    } finally {
      if (previousPointer) window.PointerEvent = previousPointer
      else Reflect.deleteProperty(window, 'PointerEvent')
    }
    await waitFor(() => expect(openWorkbenchAppletWindow).toHaveBeenCalledTimes(1))
    expect(openWorkbenchAppletWindow).toHaveBeenCalledWith('approval', 'workbench.applet.approval')
  })

  it('opens a pinned applet with the window and permissions declared by its package', async () => {
    capabilityApi.listCapabilities.mockResolvedValue({
      items: [
        {
          slug: 'a3s-leave',
          kind: 'plugin',
          category: 'productivity',
          name: '请假',
          description: '',
          color: '#3B78F0',
          featured: false,
          installed: true,
          surfaces: ['ui'],
          config: [],
          applet: {
            window: { defaultWidth: 480, defaultHeight: 720, maxWidth: 840, maxHeight: 960 },
            permissions: ['user']
          }
        }
      ],
      total: 1,
      page: 1,
      pageSize: 100,
      categories: []
    })
    vi.mocked(openWorkbenchAppletWindow).mockClear()
    renderStrip()
    fireEvent.click(screen.getByRole('button', { name: 'workbench.applet.leave' }))
    await waitFor(() => expect(openWorkbenchAppletWindow).toHaveBeenCalledTimes(1))
    expect(openWorkbenchAppletWindow).toHaveBeenCalledWith('leave', 'workbench.applet.leave', {
      window: { defaultWidth: 480, defaultHeight: 720, maxWidth: 840, maxHeight: 960 },
      permissions: ['user']
    })
  })

  it('reorders the home strip by dragging one applet onto another', () => {
    vi.mocked(openWorkbenchAppletWindow).mockClear()
    renderStrip()
    const strip = document.querySelector('[data-ui="workbench.applets"]') as HTMLElement
    const order = () => [...strip.querySelectorAll('[data-applet]')].map((node) => node.getAttribute('data-applet'))
    expect(order()).toEqual(['approval', 'leave', 'travel', 'expense', 'meeting', 'seal'])
    const seal = strip.querySelector('[data-applet="seal"]') as HTMLElement
    const review = strip.querySelector('[data-applet="approval"]') as HTMLElement
    const sealButton = within(seal).getByRole('button', { name: 'workbench.applet.seal' })
    expect(sealButton).toHaveClass('cursor-pointer')
    expect(sealButton.className).not.toContain('cursor-grab')
    const previousPoint = document.elementFromPoint?.bind(document)
    document.elementFromPoint = () => review
    const previousPointer = window.PointerEvent
    if (typeof window.PointerEvent !== 'function') {
      window.PointerEvent = class extends MouseEvent {
        pointerId: number
        constructor(type: string, init: MouseEventInit & { pointerId?: number } = {}) {
          super(type, init)
          this.pointerId = init.pointerId ?? 1
        }
      } as unknown as typeof PointerEvent
    }
    document.elementFromPoint = () => review
    try {
      fireEvent.pointerDown(seal, { clientX: 0, clientY: 0, button: 0, pointerId: 1 })
      expect(sealButton).toHaveClass('cursor-pointer')
      fireEvent.pointerMove(seal, { clientX: 24, clientY: 0, pointerId: 1 })
      expect(sealButton).toHaveClass('cursor-grabbing')
      expect(within(review).getByRole('button', { name: 'workbench.applet.approval' })).toHaveClass('cursor-pointer')
      const ghost = document.querySelector('[data-ui="workbench.applet.drag"]')
      expect(ghost).toHaveAttribute('data-applet', 'seal')
      expect(ghost).toHaveClass('pointer-events-none')
      expect(seal).toHaveAttribute('data-dragging', 'true')
      expect(document.body.style.cursor).toBe('grabbing')
      expect(document.documentElement.dataset.appletDrag).toBe('true')
      fireEvent.pointerUp(seal, { clientX: 24, clientY: 0, pointerId: 1 })
      expect(sealButton).toHaveClass('cursor-pointer')
      expect(document.querySelector('[data-ui="workbench.applet.drag"]')).toBeNull()
      expect(document.body.style.cursor).toBe('')
      expect(document.documentElement.dataset.appletDrag).toBeUndefined()
      fireEvent.click(within(seal).getByRole('button', { name: 'workbench.applet.seal' }))
    } finally {
      if (previousPoint) document.elementFromPoint = previousPoint
      else Reflect.deleteProperty(document, 'elementFromPoint')
      if (previousPointer) window.PointerEvent = previousPointer
      else Reflect.deleteProperty(window, 'PointerEvent')
    }
    expect(order()).toEqual(['seal', 'approval', 'leave', 'travel', 'expense', 'meeting'])
    expect(openWorkbenchAppletWindow).not.toHaveBeenCalled()
    expect(localStorage.getItem(HOME_APPLET_PINS_KEY)).toContain('"seal"')
  })

  it('slides the home strip toward the pointer and drops into that gap', () => {
    vi.mocked(openWorkbenchAppletWindow).mockClear()
    renderStrip()
    const strip = document.querySelector('[data-ui="workbench.applets"]') as HTMLElement
    const slots = [...strip.children].filter(
      (node): node is HTMLElement => node instanceof HTMLElement && node.hasAttribute('data-applet')
    )
    slots.forEach((slot, index) => {
      const left = index * 88
      vi.spyOn(slot, 'getBoundingClientRect').mockReturnValue({
        x: left,
        y: 10,
        left,
        top: 10,
        right: left + 56,
        bottom: 82,
        width: 56,
        height: 72,
        toJSON() {
          return {}
        }
      })
    })
    const seal = slots[5]
    const review = slots[0]
    const previousPoint = document.elementFromPoint?.bind(document)
    document.elementFromPoint = () => null
    const previousPointer = window.PointerEvent
    if (typeof window.PointerEvent !== 'function') {
      window.PointerEvent = class extends MouseEvent {
        pointerId: number
        constructor(type: string, init: MouseEventInit & { pointerId?: number } = {}) {
          super(type, init)
          this.pointerId = init.pointerId ?? 1
        }
      } as unknown as typeof PointerEvent
    }
    try {
      fireEvent.pointerDown(seal, { clientX: 468, clientY: 46, button: 0, pointerId: 1 })
      fireEvent.pointerMove(seal, { clientX: 28, clientY: 46, pointerId: 1 })
      expect(seal.style.transform).toBe('translate3d(-440px, 0px, 0)')
      expect(review.style.transform).toBe('translate3d(88px, 0px, 0)')
      expect(document.querySelector('[data-ui="workbench.applet.drag"]')?.getAttribute('style')).toContain(
        'scale(1.06)'
      )
      fireEvent.pointerUp(seal, { clientX: 28, clientY: 46, pointerId: 1 })
    } finally {
      if (previousPoint) document.elementFromPoint = previousPoint
      else Reflect.deleteProperty(document, 'elementFromPoint')
      if (previousPointer) window.PointerEvent = previousPointer
      else Reflect.deleteProperty(window, 'PointerEvent')
    }
    const order = [...strip.querySelectorAll('[data-applet]')].map((node) => node.getAttribute('data-applet'))
    expect(order).toEqual(['seal', 'approval', 'leave', 'travel', 'expense', 'meeting'])
    expect(seal.style.transform).toBe('')
    expect(review.style.transform).toBe('')
    expect(openWorkbenchAppletWindow).not.toHaveBeenCalled()
  })

  it('puts the home strip back when Escape cancels a drag', () => {
    vi.mocked(openWorkbenchAppletWindow).mockClear()
    renderStrip()
    const strip = document.querySelector('[data-ui="workbench.applets"]') as HTMLElement
    const order = () => [...strip.querySelectorAll('[data-applet]')].map((node) => node.getAttribute('data-applet'))
    const slots = [...strip.children].filter(
      (node): node is HTMLElement => node instanceof HTMLElement && node.hasAttribute('data-applet')
    )
    slots.forEach((slot, index) => {
      const left = index * 88
      vi.spyOn(slot, 'getBoundingClientRect').mockReturnValue({
        x: left,
        y: 10,
        left,
        top: 10,
        right: left + 56,
        bottom: 82,
        width: 56,
        height: 72,
        toJSON() {
          return {}
        }
      })
    })
    const seal = slots[5]
    const previousPointer = window.PointerEvent
    if (typeof window.PointerEvent !== 'function') {
      window.PointerEvent = class extends MouseEvent {
        pointerId: number
        constructor(type: string, init: MouseEventInit & { pointerId?: number } = {}) {
          super(type, init)
          this.pointerId = init.pointerId ?? 1
        }
      } as unknown as typeof PointerEvent
    }
    try {
      fireEvent.pointerDown(seal, { clientX: 468, clientY: 46, button: 0, pointerId: 1 })
      fireEvent.pointerMove(seal, { clientX: 28, clientY: 46, pointerId: 1 })
      expect(seal.style.transform).toBe('translate3d(-440px, 0px, 0)')
      fireEvent.keyDown(window, { key: 'Escape' })
      expect(order()).toEqual(['approval', 'leave', 'travel', 'expense', 'meeting', 'seal'])
      expect(seal.style.transform).toBe('')
      expect(document.querySelector('[data-ui="workbench.applet.drag"]')).toBeNull()
      fireEvent.pointerUp(seal, { clientX: 28, clientY: 46, pointerId: 1 })
    } finally {
      if (previousPointer) window.PointerEvent = previousPointer
      else Reflect.deleteProperty(window, 'PointerEvent')
    }
    expect(order()).toEqual(['approval', 'leave', 'travel', 'expense', 'meeting', 'seal'])
    expect(openWorkbenchAppletWindow).not.toHaveBeenCalled()
    expect(document.body.style.cursor).toBe('')
  })

  it('opens the mini app launcher and can unpin one applet', () => {
    vi.mocked(toast.info).mockClear()
    renderStrip()

    fireEvent.click(screen.getByRole('button', { name: 'workbench.applet.add' }))
    const panel = document.querySelector('[data-ui="workbench.applet.launcher"]')
    expect(panel).toBeTruthy()
    expect(panel).toHaveClass('fixed', 'inset-0')
    expect(panel).not.toHaveClass('items-center')
    expect(panel!.querySelector('[data-ui="workbench.applet.launcher.grid"]')).toHaveClass('items-start')
    expect(panel!.parentElement).toBe(document.body)
    expect(panel!.querySelector('label')).toHaveClass('rounded-full', 'bg-black/40')
    expect(panel!.querySelector('[data-ui="workbench.applet.launcher.pages"]')).toBeTruthy()
    const review = panel!.querySelector('[data-applet="approval"]') as HTMLElement
    expect(within(review).queryByRole('button', { name: 'workbench.applet.move_up' })).not.toBeInTheDocument()
    const unpin = within(review).getByRole('button', { name: 'workbench.applet.unpin' })
    expect(unpin).toHaveClass('size-6', 'bg-[#171717]')
    expect(unpin.querySelector('svg')).toHaveAttribute('width', '14')
    fireEvent.click(unpin)

    const strip = document.querySelector('[data-ui="workbench.applets"]') as HTMLElement
    expect(within(strip).queryByRole('button', { name: 'workbench.applet.approval' })).not.toBeInTheDocument()
    expect(within(strip).getByRole('button', { name: 'workbench.applet.leave' })).toBeInTheDocument()
    expect(toast.info).not.toHaveBeenCalled()
  })

  it('shows only mini-program plugins in the launcher', async () => {
    capabilityApi.listCapabilities.mockResolvedValue({
      items: [
        { slug: 'a3s-review', surfaces: ['skill', 'agent'] },
        { slug: 'a3s-approval', surfaces: ['tool'] },
        { slug: 'quiet-tool', surfaces: ['tool'] }
      ],
      total: 3,
      page: 1,
      pageSize: 100,
      categories: []
    })
    renderStrip()

    fireEvent.click(screen.getByRole('button', { name: 'workbench.applet.add' }))
    const panel = document.querySelector('[data-ui="workbench.applet.launcher"]') as HTMLElement
    await waitFor(() => {
      expect(panel.querySelector('[data-applet="approval"]')).toBeNull()
    })
    expect(panel.querySelector('[data-applet="review"]')).toBeNull()
    expect(panel.querySelector('[data-applet="leave"]')).toBeTruthy()
    expect(panel.querySelector('[data-applet="quiet-tool"]')).toBeNull()
    expect(panel.querySelector('[data-ui="workbench.applet.launcher.grid"]')).toHaveClass('items-start')
    expect(document.querySelector('[data-ui="workbench.applets"] [data-applet="approval"]')).toBeTruthy()
  })

  it('opens a mini app from the launcher without waiting for install', async () => {
    capabilityApi.installCapability.mockReturnValue(new Promise(() => {}))
    vi.mocked(openWorkbenchAppletWindow).mockClear()
    renderStrip()

    fireEvent.click(screen.getByRole('button', { name: 'workbench.applet.add' }))
    const panel = document.querySelector('[data-ui="workbench.applet.launcher"]') as HTMLElement
    fireEvent.click(
      within(panel.querySelector('[data-applet="leave"]') as HTMLElement).getByRole('button', {
        name: 'workbench.applet.leave'
      })
    )

    await waitFor(() => expect(openWorkbenchAppletWindow).toHaveBeenCalledWith('leave', 'workbench.applet.leave'))
    expect(document.querySelector('[data-ui="workbench.applet.launcher"]')).toHaveAttribute('data-state', 'closing')
    finishLaunchpadClose()
    expect(document.querySelector('[data-ui="workbench.applet.launcher"]')).toBeNull()
    expect(screen.getByRole('button', { name: 'workbench.applet.approval' })).toBeEnabled()
  })

  it('closes the launcher from the empty sheet and from Escape', () => {
    renderStrip()

    fireEvent.click(screen.getByRole('button', { name: 'workbench.applet.add' }))
    const panel = document.querySelector('[data-ui="workbench.applet.launcher"]') as HTMLElement
    fireEvent.mouseDown(panel.querySelector('input')!)
    expect(document.querySelector('[data-ui="workbench.applet.launcher"]')).toBeTruthy()

    fireEvent.mouseDown(panel)
    expect(panel).toHaveAttribute('data-state', 'closing')
    finishLaunchpadClose()
    expect(document.querySelector('[data-ui="workbench.applet.launcher"]')).toBeNull()

    fireEvent.click(screen.getByRole('button', { name: 'workbench.applet.add' }))
    fireEvent.keyDown(window, { key: 'Escape' })
    expect(document.querySelector('[data-ui="workbench.applet.launcher"]')).toHaveAttribute('data-state', 'closing')
    finishLaunchpadClose()
    expect(document.querySelector('[data-ui="workbench.applet.launcher"]')).toBeNull()
  })

  it('grows the launchpad from the add button and keeps it through the exit', () => {
    renderStrip()
    const add = screen.getByRole('button', { name: 'workbench.applet.add' })
    add.getBoundingClientRect = () =>
      ({
        x: 100,
        y: 200,
        left: 100,
        top: 200,
        right: 156,
        bottom: 248,
        width: 56,
        height: 48,
        toJSON() {
          return {}
        }
      }) as DOMRect

    fireEvent.click(add)
    const panel = document.querySelector('[data-ui="workbench.applet.launcher"]') as HTMLElement
    const stage = panel.querySelector('[data-ui="workbench.applet.launcher.stage"]') as HTMLElement
    expect(panel).toHaveClass('workbench-launchpad')
    expect(panel).toHaveAttribute('data-state', 'open')
    expect(add).toHaveAttribute('aria-expanded', 'true')
    expect(stage).toHaveClass('workbench-launchpad-stage')
    expect(stage.style.transformOrigin).toBe('128px 224px')

    fireEvent.keyDown(window, { key: 'Escape' })
    expect(panel).toHaveAttribute('data-state', 'closing')
    expect(add).toHaveAttribute('aria-expanded', 'true')
    const ignored = new Event('animationend', { bubbles: true })
    Object.defineProperty(ignored, 'animationName', { value: 'workbench-launchpad-in' })
    act(() => {
      stage.dispatchEvent(ignored)
    })
    expect(document.querySelector('[data-ui="workbench.applet.launcher"]')).toBe(panel)

    fireEvent.click(add)
    expect(panel).toHaveAttribute('data-state', 'open')
    const stale = new Event('animationend', { bubbles: true })
    Object.defineProperty(stale, 'animationName', { value: 'workbench-launchpad-out' })
    act(() => {
      stage.dispatchEvent(stale)
    })
    expect(document.querySelector('[data-ui="workbench.applet.launcher"]')).toBe(panel)

    fireEvent.keyDown(window, { key: 'Escape' })
    finishLaunchpadClose()
    expect(document.querySelector('[data-ui="workbench.applet.launcher"]')).toBeNull()
    expect(add).toHaveFocus()
  })

  it('closes the launchpad immediately when motion is reduced', () => {
    vi.stubGlobal('matchMedia', (query: string) => ({
      matches: query === '(prefers-reduced-motion: reduce)',
      media: query,
      addEventListener: () => undefined,
      removeEventListener: () => undefined
    }))
    try {
      renderStrip()
      fireEvent.click(screen.getByRole('button', { name: 'workbench.applet.add' }))
      expect(document.querySelector('[data-ui="workbench.applet.launcher"]')).toHaveAttribute('data-state', 'open')
      fireEvent.keyDown(window, { key: 'Escape' })
      expect(document.querySelector('[data-ui="workbench.applet.launcher"]')).toBeNull()
    } finally {
      vi.unstubAllGlobals()
    }
  })

  it('opens the window when install fails', async () => {
    capabilityApi.installCapability.mockRejectedValue(new Error('denied'))
    vi.mocked(openWorkbenchAppletWindow).mockClear()
    vi.mocked(toast.error).mockClear()
    renderStrip()

    fireEvent.click(screen.getByRole('button', { name: 'workbench.applet.seal' }))

    await waitFor(() => expect(openWorkbenchAppletWindow).toHaveBeenCalledWith('seal', 'workbench.applet.seal'))
    await waitFor(() => expect(toast.error).toHaveBeenCalledWith('workbench.applet.install_failed'))
  })

  it('opens official writing from the card and commits the draft', async () => {
    vi.mocked(toast.info).mockClear()
    renderHome()

    const noticeTile = document.querySelector('[data-home="notice"]')!
    expect(noticeTile).toHaveAttribute('data-writing-type', 'notice')
    expect(noticeTile).toHaveTextContent('workbench.writing.card.notice_meeting.title')
    expect(document.querySelector('[data-home="report"]')).toHaveAttribute('data-writing-type', 'report')
    expect(document.querySelector('[data-home="request"]')).toHaveAttribute('data-writing-type', 'request')
    expect(document.querySelector('[data-home="minutes"]')).toHaveAttribute('data-writing-type', 'minutes')
    expect(zhCn['workbench.writing.card.notice_meeting.title']).toBe('通知')
    expect(zhCn['workbench.writing.card.summary_quarter.title']).toBe('报告')
    expect(zhCn['workbench.writing.card.request_funding.title']).toBe('请示')
    expect(zhCn['workbench.writing.card.minutes_director.title']).toBe('纪要')
    expect(zhCn['workbench.feature.write.career_tag']).toBe('报告')
    expect(zhCn['workbench.feature.write.blessing']).toBe('发布需要周知或执行的事项。')
    expect(zhCn['workbench.feature.write.career']).toBe('向上级汇报工作进展和需要说明的情况。')
    expect(zhCn['workbench.feature.write.plan']).toBe('就下列事项请求指示或批准。')
    expect(zhCn['workbench.feature.write.review']).toBe('记载会议的主要情况和议定事项。')
    expect(enUs['workbench.writing.card.minutes_director.title']).toBe('Minutes')
    expect(enUs['workbench.writing.card.summary_quarter.title']).toBe('Report')
    expect(enUs['workbench.feature.write.career_tag']).toBe('Report')

    fireEvent.click(screen.getByRole('button', { name: 'workbench.feature.write.go' }))

    expect(toast.info).not.toHaveBeenCalled()
    expect(document.querySelector('[data-ui="writing.center"]')).toBeNull()
    expect(screen.getByPlaceholderText('workbench.composer.placeholder')).toHaveValue('')
    const picker = document.querySelector('[data-ui="workbench.writing.picker"]') as HTMLElement
    expect(picker).toBeTruthy()
    expect(picker.querySelector('.grid')).toHaveClass('grid-cols-4')
    expect(picker).toHaveTextContent('workbench.writing.hub.title')
    expect(picker.querySelectorAll('[data-official-type]')).toHaveLength(11)
    expect(picker.querySelectorAll('[data-ui="workbench.writing.preview"]')).toHaveLength(11)
    expect(picker.querySelector('[data-official-type="notice"]')).toHaveAttribute('data-writing-banner', 'true')
    expect(picker.querySelector('[data-official-type="report"]')).toHaveAttribute('data-writing-banner', 'false')
    expect(screen.queryByRole('textbox', { name: 'workbench.writing.hub.brief' })).not.toBeInTheDocument()

    const handoffs: Array<{
      text?: string
      typeId?: string
      brief?: string
      choices?: { banner?: boolean }
      fields?: { org?: string }
    }> = []
    const onWriting = (event: Event) => {
      const detail = (
        event as CustomEvent<{
          text?: string
          typeId?: string
          brief?: string
          choices?: { banner?: boolean }
          fields?: { org?: string }
        }>
      ).detail
      if (detail) handoffs.push(detail)
    }
    window.addEventListener(OFFICIAL_WRITING_EVENT, onWriting)
    fireEvent.click(document.querySelector('[data-official-type="notice"]')!)
    const brief = screen.getByRole('textbox', { name: 'workbench.writing.hub.brief' })
    expect(brief).toHaveValue('')
    expect(screen.queryByRole('button', { name: 'workbench.writing.hub.make' })).not.toBeInTheDocument()
    expect(screen.getByRole('button', { name: 'workbench.writing.hub.commit' })).toBeDisabled()
    fireEvent.click(screen.getByRole('button', { name: 'workbench.writing.hub.commit' }))
    expect(handoffs).toHaveLength(0)
    const dialog = document.querySelector('[data-writing-stage="brief"]') as HTMLElement
    expect(dialog).toHaveAttribute('data-writing-stage', 'brief')
    expect(screen.queryByRole('button', { name: 'workbench.writing.hub.back' })).not.toBeInTheDocument()
    expect(document.querySelector('[data-ui="workbench.writing.draft"]')).toBeNull()
    const banner = screen.getByRole('switch', { name: 'workbench.writing.option.banner' })
    expect(banner).toHaveAttribute('aria-checked', 'true')
    expect(dialog.style.maxWidth).toContain('1080px')
    expect(dialog.style.height).toContain('860px')
    expect(document.querySelector('[data-ui="workbench.writing.layout"]')).toBeTruthy()
    expect(document.querySelectorAll('[data-ui="workbench.writing.part"]')).toHaveLength(3)
    const live = document.querySelector('[data-ui="workbench.writing.live"]')
    expect(live?.querySelector('[data-office-preview="true"]')).toBeTruthy()
    expect(live).toHaveTextContent('data-writing-slot="issuer"')
    expect(live).toHaveTextContent('border: 1px dashed #595959')
    expect(live).toHaveTextContent('待填写：事由与依据')
    expect(live).not.toHaveTextContent('【')
    fireEvent.change(screen.getByRole('textbox', { name: 'workbench.writing.field.org' }), {
      target: { value: '市教育局' }
    })
    expect(live).toHaveTextContent('市教育局')
    expect(live).not.toHaveTextContent('data-writing-slot="issuer"')
    expect(live).toHaveTextContent('data-writing-slot="subject"')
    fireEvent.click(banner)
    expect(banner).toHaveAttribute('aria-checked', 'false')
    expect(live).not.toHaveTextContent('48pt')
    expect(live).not.toHaveTextContent('#FF0000')
    fireEvent.change(brief, { target: { value: '请教育局通知各校周五开会' } })
    expect(screen.getByRole('button', { name: 'workbench.writing.hub.commit' })).toBeEnabled()
    expect(document.querySelector('[data-ui="workbench.writing.draft"]')).toBeNull()
    expect(screen.queryByRole('button', { name: 'workbench.writing.hub.back' })).not.toBeInTheDocument()
    expect(live?.querySelector('[data-office-preview="true"]')).toBeTruthy()
    expect(live).toHaveTextContent('请教育局通知各校周五开会')
    expect(live).toHaveTextContent('特此通知')
    expect(live).toHaveTextContent('发文字号')
    expect(live).not.toHaveTextContent('发文机关文件')
    expect(live).not.toHaveTextContent('文号：待填写')
    expect(live?.querySelector('[data-ui="workbench.writing.seal"]')).toBeNull()
    fireEvent.click(screen.getByRole('button', { name: 'workbench.writing.hub.commit' }))

    const composer = screen.getByPlaceholderText('workbench.composer.placeholder') as HTMLTextAreaElement
    expect(handoffs).toHaveLength(1)
    const handoff = handoffs[0]
    const instruction = handoff?.text ?? ''
    expect(instruction).toContain('这份通知')
    expect(instruction).toContain('请教育局通知各校周五开会')
    expect(instruction).toContain('不使用红头')
    expect(instruction).toContain('市教育局')
    expect(handoff?.typeId).toBe('notice')
    expect(handoff?.brief).toBe('请教育局通知各校周五开会')
    expect(handoff?.choices?.banner).toBe(false)
    expect(handoff?.fields?.org).toBe('市教育局')
    expect(composer.value).toBe(instruction)
    expect(readOfficialWritingSession()).toBeNull()
    expect(isOfficialWritingHandoff(handoff)).toBe(true)
    if (!isOfficialWritingHandoff(handoff)) throw new Error('writing handoff')
    const opened = await openOfficialWritingTask({
      handoff,
      create: async () => ({ id: 'writing-task' }),
      workspacePath: () => '/tmp/workspace',
      adopt: vi.fn()
    })
    expect(opened).toEqual({
      sessionId: 'writing-task',
      file: { path: '/tmp/workspace/通知.docx', name: '通知.docx' }
    })
    expect(readOfficialWritingFile()).toEqual({
      sessionId: 'writing-task',
      path: '/tmp/workspace/通知.docx',
      name: '通知.docx'
    })
    window.removeEventListener(OFFICIAL_WRITING_EVENT, onWriting)
    resetOfficialWritingSessionForTests()
    resetOfficialWritingFileForTests()
    await waitFor(() => expect(document.querySelector('[data-writing-stage]')).toBeNull())
    expect(document.querySelector('[data-ui="writing.center"]')).toBeNull()
  })

  it('keeps the creation card on 暂未开放 and asks for a file on each PDF row', () => {
    vi.mocked(toast.info).mockClear()
    vi.mocked(openWorkbenchAppletWindow).mockClear()
    renderHome()

    const create = document.querySelector('[data-feature="create"]')!
    expect(create).toHaveTextContent('workbench.feature.create')
    expect(create).toHaveTextContent('workbench.feature.create.center')
    fireEvent.click(create)
    fireEvent.click(create.querySelector('button')!)
    for (const id of ['ppt', 'site', 'video', 'image']) {
      fireEvent.click(create.querySelector(`[data-create="${id}"]`)!)
    }
    expect(document.querySelector('[data-ui="workbench.create.board"]')).toBeNull()
    expect(toast.info).toHaveBeenCalledTimes(5)
    expect(toast.info).toHaveBeenCalledWith({ title: 'workbench.composer.not_open', appearance: 'pill' })
    expect(openWorkbenchAppletWindow).toHaveBeenCalledTimes(1)
    expect(openWorkbenchAppletWindow).toHaveBeenCalledWith('plan', 'workbench.feature.create.image')
    vi.mocked(openWorkbenchAppletWindow).mockClear()
    vi.mocked(toast.info).mockClear()

    const click = vi.spyOn(HTMLInputElement.prototype, 'click').mockImplementation(() => {})
    fireEvent.click(
      within(document.querySelector('[data-feature="pdf"]') as HTMLElement).getByRole('button', {
        name: /workbench\.feature\.pdf\.edit\.title/
      })
    )
    const input = document.querySelector('[data-ui="workbench.feature.pdf.input"]') as HTMLInputElement
    expect(click).toHaveBeenCalledTimes(1)
    expect(input.accept).toBe('.pdf')
    expect(input.multiple).toBe(true)
    fireEvent.click(
      within(document.querySelector('[data-feature="pdf"]') as HTMLElement).getByRole('button', {
        name: /workbench\.feature\.pdf\.translate\.title/
      })
    )
    expect(click).toHaveBeenCalledTimes(2)
    expect(input.accept).toBe('.doc,.docx')
    expect(input.multiple).toBe(true)
    fireEvent.change(input, { target: { files: [] } })
    click.mockRestore()
    expect(openWorkbenchAppletWindow).not.toHaveBeenCalled()
    expect(localStorage.getItem(PDF_TOOL_KIND_KEY)).toBeNull()
    expect(toast.info).not.toHaveBeenCalled()
    expect(document.querySelector('[data-ui="workbench.pdf-tools"]')).toBeNull()
    expect(document.querySelector('[data-ui="workbench.applet.window"]')).toBeNull()
    expect(document.querySelector('[data-writing-stage]')).toBeNull()

    fireEvent.click(document.querySelector('[data-home="notice"]')!)
    expect(toast.info).not.toHaveBeenCalled()
    expect(document.querySelector('[data-writing-stage]')).toBeTruthy()
  })

  it('opens the homepage PDF job in its own window and leaves the homepage in place', async () => {
    const click = vi.spyOn(HTMLInputElement.prototype, 'click').mockImplementation(() => {})
    const invoke = vi.fn(async (command: string) => {
      if (command === 'stage_pdf_tool_handoff' || command === 'drop_pdf_tool_handoff') return null
      throw new Error(command)
    })
    Object.assign(globalThis, { __TAURI_INTERNALS__: { invoke } })
    vi.mocked(openWorkbenchAppletWindow).mockClear()
    vi.mocked(toast.info).mockClear()
    try {
      renderHome()

      fireEvent.click(
        within(document.querySelector('[data-feature="pdf"]') as HTMLElement).getByRole('button', {
          name: /workbench\.feature\.pdf\.edit\.title/
        })
      )
      const input = document.querySelector('[data-ui="workbench.feature.pdf.input"]') as HTMLInputElement
      fireEvent.change(input, { target: { files: [new File(['%PDF'], 'notes.pdf', { type: 'application/pdf' })] } })
      await waitFor(() =>
        expect(openWorkbenchAppletWindow).toHaveBeenCalledWith(
          'pdf',
          'workbench.feature.pdf.tools.title',
          undefined,
          'pdf-to-word'
        )
      )
      expect(invoke).toHaveBeenCalledWith('stage_pdf_tool_handoff', {
        kind: 'pdf-to-word',
        files: [{ name: 'notes.pdf', mediaType: 'application/pdf', bytesBase64: btoa('%PDF') }]
      })
      expect(click).toHaveBeenCalledTimes(1)
      expect(document.querySelector('[data-feature="pdf"]')).toBeTruthy()
      expect(document.querySelector('[data-ui="workbench.pdf-tools"]')).toBeNull()
      expect(document.querySelector('[data-ui="workbench.applet.window"]')).toBeNull()
      expect(toast.info).not.toHaveBeenCalled()
    } finally {
      click.mockRestore()
      Reflect.deleteProperty(globalThis, '__TAURI_INTERNALS__')
    }
  })

  it('drops the homepage PDF job when the window does not open', async () => {
    const invoke = vi.fn(async (command: string) => {
      if (command === 'stage_pdf_tool_handoff' || command === 'drop_pdf_tool_handoff') return null
      throw new Error(command)
    })
    Object.assign(globalThis, { __TAURI_INTERNALS__: { invoke } })
    vi.mocked(openWorkbenchAppletWindow).mockReset()
    vi.mocked(openWorkbenchAppletWindow).mockResolvedValue(false)
    vi.mocked(toast.info).mockClear()
    try {
      renderHome()

      fireEvent.click(
        within(document.querySelector('[data-feature="pdf"]') as HTMLElement).getByRole('button', {
          name: /workbench\.feature\.pdf\.translate\.title/
        })
      )
      const input = document.querySelector('[data-ui="workbench.feature.pdf.input"]') as HTMLInputElement
      fireEvent.change(input, { target: { files: [new File(['PK'], 'notes.docx', { type: '' })] } })
      await waitFor(() => expect(invoke).toHaveBeenCalledWith('drop_pdf_tool_handoff'))
      expect(openWorkbenchAppletWindow).toHaveBeenCalledWith(
        'pdf',
        'workbench.feature.pdf.tools.title',
        undefined,
        'word-to-pdf'
      )
      expect(toast.info).toHaveBeenCalledWith({ title: 'workbench.composer.not_open', appearance: 'pill' })
      expect(document.querySelector('[data-feature="pdf"]')).toBeTruthy()
      expect(document.querySelector('[data-ui="workbench.pdf-tools"]')).toBeNull()
    } finally {
      vi.mocked(openWorkbenchAppletWindow).mockReset()
      vi.mocked(openWorkbenchAppletWindow).mockResolvedValue(true)
      Reflect.deleteProperty(globalThis, '__TAURI_INTERNALS__')
    }
  })

  it('opens 通知, 报告, 请示, and 纪要 with that type layout switches', () => {
    renderHome()

    fireEvent.click(document.querySelector('[data-home="notice"]')!)
    expect(screen.getByRole('switch', { name: 'workbench.writing.option.banner' })).toHaveAttribute(
      'aria-checked',
      'true'
    )
    expect(screen.queryByRole('switch', { name: 'workbench.writing.option.amount' })).not.toBeInTheDocument()

    fireEvent.click(document.querySelector('[data-home="report"]')!)
    expect(screen.getByRole('switch', { name: 'workbench.writing.option.banner' })).toHaveAttribute(
      'aria-checked',
      'false'
    )
    expect(screen.getByRole('textbox', { name: 'workbench.writing.hub.brief' })).toHaveValue(
      'workbench.feature.write.career'
    )

    fireEvent.click(document.querySelector('[data-home="request"]')!)
    expect(screen.getByRole('switch', { name: 'workbench.writing.option.signer' })).toHaveAttribute(
      'aria-checked',
      'true'
    )
    expect(screen.getByRole('switch', { name: 'workbench.writing.option.amount' })).toHaveAttribute(
      'aria-checked',
      'false'
    )

    fireEvent.click(document.querySelector('[data-home="minutes"]')!)
    expect(screen.getByRole('switch', { name: 'workbench.writing.option.attendance' })).toHaveAttribute(
      'aria-checked',
      'true'
    )
    expect(screen.queryByRole('switch', { name: 'workbench.writing.option.amount' })).not.toBeInTheDocument()
    expect(screen.getByRole('textbox', { name: 'workbench.writing.hub.brief' })).toHaveValue(
      'workbench.feature.write.review'
    )
  })

  it('keeps the contract-review agent out of the home mini-app strip', () => {
    renderStrip()
    expect(document.querySelector('[data-applet="review"]')).toBeNull()
    expect(screen.getByRole('button', { name: 'workbench.applet.approval' })).toBeInTheDocument()
  })

  it('opens the staged PDF handler picker', async () => {
    localStorage.removeItem(APPLET_PERMISSION_GRANT_KEY)
    localStorage.setItem(PDF_TOOL_KIND_KEY, 'word-to-pdf')
    const click = vi.spyOn(HTMLInputElement.prototype, 'click').mockImplementation(() => {})
    render(<WorkbenchAppletWindow appletId="pdf" />)
    fireEvent.click(screen.getByRole('button', { name: 'workbench.applet.permission.allow' }))
    const input = await waitFor(() => {
      const node = document.querySelector('[data-ui="workbench.pdf-tools.input"]') as HTMLInputElement | null
      expect(node).toBeTruthy()
      return node!
    })
    expect(click).toHaveBeenCalled()
    expect(input.accept).toContain('.docx')
    expect(input.multiple).toBe(true)
    expect(localStorage.getItem(PDF_TOOL_KIND_KEY)).toBeNull()
    click.mockRestore()
  })

  it('opens the PDF handler named on the applet URL', async () => {
    localStorage.removeItem(APPLET_PERMISSION_GRANT_KEY)
    const click = vi.spyOn(HTMLInputElement.prototype, 'click').mockImplementation(() => {})
    render(<WorkbenchAppletWindow appletId="pdf" tool="pdf-to-word" />)
    fireEvent.click(screen.getByRole('button', { name: 'workbench.applet.permission.allow' }))
    const input = await waitFor(() => {
      const node = document.querySelector('[data-ui="workbench.pdf-tools.input"]') as HTMLInputElement | null
      expect(node).toBeTruthy()
      return node!
    })
    expect(click).toHaveBeenCalled()
    expect(input.accept).toContain('.pdf')
    expect(input.accept).not.toContain('.docx')
    expect(input.multiple).toBe(true)
    click.mockRestore()
  })

  it('opens the format tools applet window from more', async () => {
    vi.mocked(openWorkbenchAppletWindow).mockClear()
    vi.mocked(toast.info).mockClear()
    renderHome()

    fireEvent.click(screen.getByRole('button', { name: /workbench\.feature\.pdf\.open/ }))

    expect(openWorkbenchAppletWindow).toHaveBeenCalledWith('pdf', 'workbench.feature.pdf.tools.title')
    expect(toast.info).not.toHaveBeenCalled()
    expect(document.querySelector('[data-ui="workbench.pdf-tools"]')).toBeNull()
  })

  it('opens the plan editor from the create collage', async () => {
    vi.mocked(openWorkbenchAppletWindow).mockClear()
    vi.mocked(toast.info).mockClear()
    renderHome()

    fireEvent.click(document.querySelector('[data-cover="image"]')!)

    await waitFor(() =>
      expect(openWorkbenchAppletWindow).toHaveBeenCalledWith('plan', 'workbench.feature.create.image')
    )
    expect(toast.info).not.toHaveBeenCalled()
  })

  it('leaves the other create tiles closed', () => {
    vi.mocked(openWorkbenchAppletWindow).mockClear()
    vi.mocked(toast.info).mockClear()
    renderHome()

    fireEvent.click(document.querySelector('[data-cover="video"]')!)

    expect(openWorkbenchAppletWindow).not.toHaveBeenCalled()
    expect(toast.info).toHaveBeenCalled()
  })

  it('starts a format-conversion job and names a real failure', async () => {
    render(<WorkbenchAppletWindow appletId="pdf" />)

    const panel = document.querySelector('[data-ui="workbench.applet.window"]')
    expect(panel).toHaveAttribute('data-applet', 'pdf')
    expect(panel).toHaveAttribute('data-presentation', 'window')
    expect(screen.getByRole('heading', { level: 1, name: 'workbench.feature.pdf.tools.title' })).toBeInTheDocument()
    expect(screen.getByRole('button', { name: 'common.close' })).toBeInTheDocument()
    expect(screen.queryByRole('button', { name: 'workbench.feature.pdf.tools.item.pdf_to_word' })).toBeNull()
    expect(document.querySelector('[data-ui="workbench.applet.permission"]')).toBeTruthy()
    fireEvent.click(screen.getByRole('button', { name: 'workbench.applet.permission.allow' }))
    await waitFor(() => {
      expect(document.querySelector('[data-ui="workbench.pdf-tools"]')?.className).toContain('overflow-y-auto')
    })
    expect(screen.getByRole('button', { name: 'workbench.feature.pdf.tools.item.pdf_to_word' })).toBeInTheDocument()
    expect(panel).not.toHaveTextContent('夸克')
    expect(document.querySelector('[data-ui="workbench.applet.loading"]')).toBeNull()
    expect(document.querySelector('[data-ui="workbench.pdf-tools.job"]')).toBeNull()
    expect(screen.getByRole('button', { name: 'workbench.feature.pdf.tools.crop' })).toHaveAttribute(
      'aria-pressed',
      'true'
    )
    expect(document.querySelectorAll('[data-kind="merge"]')).toHaveLength(2)
    expect(zhCn['workbench.feature.pdf.tools.edit.body']).not.toContain('完整还原')

    const input = document.querySelector('[data-ui="workbench.pdf-tools.input"]') as HTMLInputElement
    for (const button of document.querySelectorAll('[data-kind="merge"]')) {
      fireEvent.click(button)
      expect(input.accept).toContain('.png')
      expect(input.multiple).toBe(true)
    }

    const png = new File([await encodePng(solid(4, 2, [10, 20, 30, 255]))], 'a.png', { type: 'image/png' })
    const other = new File([await encodePng(solid(4, 2, [30, 20, 10, 255]))], 'b.png', { type: 'image/png' })
    fireEvent.drop(document.querySelector('[data-ui="workbench.pdf-tools.drop"]')!, {
      dataTransfer: { files: [png, other] }
    })
    expect(document.querySelector('[data-ui="workbench.pdf-tools.job"]')).toHaveAttribute('data-state', 'choose')
    expect(document.querySelector('[data-ui="workbench.pdf-tools.job"]')).toHaveTextContent('a.png')
    expect(document.querySelector('[data-ui="workbench.pdf-tools.job"]')).toHaveTextContent('b.png')
    expect(document.querySelector('[data-ui="workbench.pdf-tools"]')).toHaveAttribute('data-stage', 'result')
    expect(document.querySelector('[data-ui="workbench.pdf-tools.drop"]')).toBeNull()
    expect(document.querySelector('a[download]')).toBeNull()

    fireEvent.click(screen.getByRole('button', { name: 'workbench.applet.nav.back' }))
    expect(document.querySelector('[data-ui="workbench.pdf-tools.job"]')).toBeNull()
    expect(document.querySelector('[data-ui="workbench.pdf-tools"]')).toHaveAttribute('data-stage', 'catalog')

    vi.mocked(toast.info).mockClear()
    fireEvent.click(screen.getByRole('button', { name: 'workbench.feature.pdf.tools.item.pdf_to_word' }))
    fireEvent.change(input, { target: { files: [new File(['PK'], 'notes.docx', { type: '' })] } })
    expect(document.querySelector('[data-ui="document-tools.review.reason"]')).toHaveTextContent(
      'workbench.feature.pdf.tools.workspace.refusal.type'
    )
    expect(document.querySelector('[data-ui="document-tools.workspace"]')).toHaveTextContent('notes.docx')
    expect(document.querySelector('a[download]')).toBeNull()

    fireEvent.change(input, {
      target: { files: [new File(['%PDF-1.4\n/Encrypt'], 'secret.pdf', { type: 'application/pdf' })] }
    })
    await waitFor(() =>
      expect(document.querySelector('[data-ui="document-tools.review.reason"]')).toHaveTextContent(
        'workbench.feature.pdf.tools.job.encrypted'
      )
    )

    fireEvent.click(screen.getByRole('button', { name: 'workbench.applet.nav.back' }))
    fireEvent.click(screen.getByRole('button', { name: 'workbench.feature.pdf.tools.chip.erase' }))
    fireEvent.change(input, { target: { files: [png] } })
    await waitFor(() =>
      expect(document.querySelector('[data-ui="document-tools.review.reason"]')).toHaveTextContent(
        'workbench.feature.pdf.tools.job.gap.inpaint'
      )
    )
    expect(toast.info).not.toHaveBeenCalled()
    expect(document.body.textContent).not.toContain('暂未开放')
    expect(document.body.textContent).not.toContain('完整还原')
  })

  it('opens the existing PDF viewer with the edited bytes', async () => {
    localStorage.removeItem(APPLET_PERMISSION_GRANT_KEY)
    const pdf = await PDFDocument.create()
    const font = await pdf.embedFont(StandardFonts.Helvetica)
    const page = pdf.addPage([200, 120])
    page.drawText('Hello layer', { x: 20, y: 60, size: 16, font })
    const bytes = new Uint8Array(await pdf.save())
    render(<WorkbenchAppletWindow appletId="pdf" />)
    fireEvent.click(screen.getByRole('button', { name: 'workbench.applet.permission.allow' }))
    const input = await waitFor(() => {
      const node = document.querySelector('[data-ui="workbench.pdf-tools.input"]') as HTMLInputElement | null
      expect(node).toBeTruthy()
      return node!
    })
    fireEvent.click(screen.getByRole('button', { name: /workbench\.feature\.pdf\.tools\.edit\.title/ }))
    fireEvent.change(input, { target: { files: [new File([bytes], 'layer.pdf', { type: 'application/pdf' })] } })
    const viewer = await waitFor(() => {
      const node = document.querySelector('[data-ui="office.pdf-viewer"]')
      expect(node).toBeTruthy()
      expect(node).toHaveTextContent(`%PDF`)
      return node
    })
    expect(viewer).toHaveAttribute('data-file', 'layer.pdf')
    expect(viewer).toHaveTextContent(`${bytes.byteLength}:%PDF`)
    expect(document.querySelector('[data-ui="workbench.pdf-tools.job"]')).not.toHaveAttribute('data-editor')
    expect(document.body.textContent).not.toContain('完整还原')
  }, 20000)

  it('writes a scan to the org drive only for a member session', async () => {
    localStorage.removeItem(APPLET_PERMISSION_GRANT_KEY)
    const upload = vi.spyOn(driveApi, 'upload')
    const fetchMock = vi.spyOn(globalThis, 'fetch').mockImplementation(async (input, init) => {
      const url = String(input)
      if (url === 'http://drive.test/api/bootstrap') {
        return new Response(JSON.stringify({ data: { orgOrigin: 'http://org.test' } }), {
          status: 200,
          headers: { 'Content-Type': 'application/json' }
        })
      }
      if (url === 'http://org.test/v1/api/org/drive/org-42/files') {
        expect(new Headers(init?.headers).get('Authorization')).toBe('Bearer member-session')
        return new Response(null, { status: 200 })
      }
      return new Response('missing', { status: 404 })
    })
    Object.defineProperty(globalThis, '__A3S_STUDIO_RUNTIME__', {
      configurable: true,
      value: { apiBaseUrl: 'http://drive.test' }
    })
    try {
      const png = new File([await encodePng(solid(8, 6, [20, 20, 20, 255]))], 'page.png', { type: 'image/png' })
      render(<WorkbenchAppletWindow appletId="pdf" />)
      fireEvent.click(screen.getByRole('button', { name: 'workbench.applet.permission.allow' }))
      const input = await waitFor(() => {
        const node = document.querySelector('[data-ui="workbench.pdf-tools.input"]') as HTMLInputElement | null
        expect(node).toBeTruthy()
        return node!
      })
      const scan = () => {
        const back = screen.getByRole('button', { name: 'workbench.applet.nav.back' })
        if (!(back as HTMLButtonElement).disabled) fireEvent.click(back)
        fireEvent.click(screen.getByRole('button', { name: /workbench\.feature\.pdf\.tools\.scan\.title/ }))
        fireEvent.change(input, { target: { files: [png] } })
      }
      scan()
      await waitFor(() =>
        expect(document.querySelector('[data-ui="workbench.pdf-tools.job"]')).toHaveAttribute('data-state', 'done')
      )
      expect(document.querySelector('[data-ui="workbench.pdf-tools.job"]')).toHaveTextContent('scan.pdf')
      expect(document.querySelector('a[download="scan.pdf"]')).toHaveAttribute('download', 'scan.pdf')
      expect(document.querySelector('[data-ui="workbench.pdf-tools.job.copy"]')).toHaveTextContent(
        '结果只在这台电脑上。'
      )
      expect(upload).not.toHaveBeenCalled()

      localStorage.setItem('a3s_org_member_token', 'member-session')
      localStorage.setItem('a3s.org.center', PERSONAL_CENTER)
      scan()
      await waitFor(() =>
        expect(document.querySelector('[data-ui="workbench.pdf-tools.job.copy"]')).toHaveTextContent(
          '结果只在这台电脑上。'
        )
      )
      expect(upload).not.toHaveBeenCalled()

      localStorage.setItem('a3s.org.center', 'org-42')
      scan()
      await waitFor(() => expect(upload).toHaveBeenCalledOnce())
      expect(document.querySelector('[data-ui="workbench.pdf-tools.job.copy"]')).toBeNull()
      expect(document.body.textContent).not.toContain('结果只在这台电脑上。')
      const sent = upload.mock.calls[0]?.[0]
      expect(sent).toBeInstanceOf(File)
      expect(sent?.name).toBe('scan.pdf')
      expect(sent?.type).toBe('application/pdf')
      const header = new TextDecoder().decode((await fileBytes(sent!)).slice(0, 4))
      expect(header).toBe('%PDF')
      expect(fetchMock).toHaveBeenCalledWith('http://org.test/v1/api/org/drive/org-42/files', expect.anything())
    } finally {
      upload.mockRestore()
      fetchMock.mockRestore()
      Reflect.deleteProperty(globalThis, '__A3S_STUDIO_RUNTIME__')
      resetDriveByteBaseForTests()
    }
  })

  it('shows a streamed transcript while audio is still running', async () => {
    localStorage.removeItem(APPLET_PERMISSION_GRANT_KEY)
    const previousRuntime = (globalThis as typeof globalThis & { __A3S_STUDIO_RUNTIME__?: unknown })
      .__A3S_STUDIO_RUNTIME__
    const nativeToken = 'a'.repeat(64)
    Object.assign(globalThis, { __A3S_STUDIO_RUNTIME__: { apiBaseUrl: 'http://127.0.0.1:4199', token: nativeToken } })
    const encoder = new TextEncoder()
    let release: () => void = () => {}
    const gate = new Promise<void>((resolve) => {
      release = resolve
    })
    let sent = false
    const fetchMock = vi.spyOn(globalThis, 'fetch').mockImplementation(async (input, options) => {
      if (String(input) === 'http://127.0.0.1:4199/api/os-gateway/credential') {
        expect(new Headers(options?.headers).get('authorization')).toBe(`Bearer ${nativeToken}`)
        return new Response(JSON.stringify({ origin: 'http://127.0.0.1:8080', token: 'audio-fixture-token' }))
      }
      expect(String(input)).toBe('http://127.0.0.1:8080/v1/audio/transcriptions')
      expect(new Headers(options?.headers).get('authorization')).toBe('Bearer audio-fixture-token')
      return new Response(
        new ReadableStream<Uint8Array>({
          async pull(controller) {
            if (!sent) {
              sent = true
              controller.enqueue(encoder.encode('data: {"delta":"会议开始"}\n\n'))
              return
            }
            await gate
            controller.enqueue(encoder.encode('data: {"delta":"通过"}\n\n'))
            controller.close()
          }
        }),
        { status: 200, headers: { 'Content-Type': 'text/event-stream' } }
      )
    })
    try {
      render(<WorkbenchAppletWindow appletId="pdf" />)
      fireEvent.click(screen.getByRole('button', { name: 'workbench.applet.permission.allow' }))
      const input = await waitFor(() => {
        const node = document.querySelector('[data-ui="workbench.pdf-tools.input"]') as HTMLInputElement | null
        expect(node).toBeTruthy()
        return node!
      })
      fireEvent.click(screen.getByRole('button', { name: /workbench\.feature\.pdf\.tools\.audio\.title/ }))
      fireEvent.change(input, {
        target: { files: [new File([Uint8Array.of(1, 2, 3)], 'memo.wav', { type: 'audio/wav' })] }
      })
      await waitFor(() =>
        expect(document.querySelector('[data-ui="workbench.pdf-tools.job.partial"]')).toHaveTextContent('会议开始')
      )
      expect(document.querySelector('[data-ui="workbench.pdf-tools.job"]')).toHaveAttribute('data-state', 'running')
      expect(document.querySelector('[data-ui="workbench.pdf-tools.job"]')).toHaveTextContent('memo.wav')
      expect(document.querySelector('[role="progressbar"]')).not.toHaveAttribute('aria-valuenow')
      expect(document.querySelector('[data-ui="workbench.pdf-tools"]')).toHaveAttribute('data-stage', 'work')
      expect(document.querySelector('[data-ui="workbench.pdf-tools.drop"]')).toBeNull()
      expect(document.querySelector('a[download]')).toBeNull()
      release()
      await waitFor(() =>
        expect(document.querySelector('[data-ui="workbench.pdf-tools.job"]')).toHaveAttribute('data-state', 'done')
      )
      expect(document.querySelector('[data-ui="workbench.pdf-tools.job"]')).toHaveTextContent('minutes.docx')
      expect(document.querySelector('a[download="minutes.docx"]')).toHaveAttribute('download', 'minutes.docx')
      expect(document.querySelectorAll('a[download]')).toHaveLength(1)
      expect(document.querySelector('[data-ui="workbench.pdf-tools"]')).toHaveAttribute('data-stage', 'result')
    } finally {
      release()
      fetchMock.mockRestore()
      if (previousRuntime === undefined) Reflect.deleteProperty(globalThis, '__A3S_STUDIO_RUNTIME__')
      else Object.assign(globalThis, { __A3S_STUDIO_RUNTIME__: previousRuntime })
    }
  })

  it('opens 连接器, 技能, and 自动化', () => {
    const onOpenCatalog = vi.fn()
    const onOpenScheduledTasks = vi.fn()
    renderHome({ onOpenCatalog, onOpenScheduledTasks })

    fireEvent.click(screen.getByRole('button', { name: 'workbench.nav.connectors' }))
    fireEvent.click(screen.getByRole('button', { name: 'workbench.nav.skills' }))

    expect(onOpenCatalog).toHaveBeenNthCalledWith(1, 'connector')
    expect(onOpenCatalog).toHaveBeenNthCalledWith(2, 'skill')

    fireEvent.click(screen.getByRole('button', { name: 'workbench.nav.scheduled' }))
    expect(onOpenScheduledTasks).toHaveBeenCalledOnce()
    expect(screen.getByRole('button', { name: 'workbench.nav.scheduled' })).toHaveAttribute('aria-current', 'page')
    expect(document.querySelector('[data-ui="workbench.automation"]')).toBeTruthy()
    expect(document.querySelector('[data-ui="workbench.composer"]')).toBeNull()
  })

  it('returns to the composer when a scheduled task is created with the agent', () => {
    const onCreateScheduledWithAgent = vi.fn()
    renderHome({ onCreateScheduledWithAgent })

    fireEvent.click(screen.getByRole('button', { name: 'workbench.nav.scheduled' }))
    fireEvent.click(screen.getByRole('button', { name: 'settings.scheduledTasks.agentCreate' }))

    expect(onCreateScheduledWithAgent).toHaveBeenCalledOnce()
    expect(document.querySelector('[data-ui="workbench.automation"]')).toBeNull()
    expect(document.querySelector('[data-ui="workbench.home"]')).toBeTruthy()
  })

  it('leaves automation for the work home when the work route is requested', () => {
    renderHome()

    fireEvent.click(screen.getByRole('button', { name: 'workbench.nav.scheduled' }))
    expect(document.querySelector('[data-ui="workbench.automation"]')).toBeTruthy()

    act(() => {
      window.dispatchEvent(new CustomEvent(OPEN_MAIN_ROUTE_EVENT, { detail: { path: '/app/agents' } }))
    })

    expect(document.querySelector('[data-ui="workbench.automation"]')).toBeNull()
    expect(document.querySelector('[data-ui="workbench.canvas"]')).toBeTruthy()
    expect(screen.getByRole('button', { name: 'workbench.nav.scheduled' })).not.toHaveAttribute('aria-current', 'page')
    expect(screen.queryByRole('button', { name: 'workbench.nav.experts' })).toBeNull()
  })

  it('keeps automation open when a different route is requested', () => {
    renderHome()

    fireEvent.click(screen.getByRole('button', { name: 'workbench.nav.scheduled' }))
    act(() => {
      window.dispatchEvent(new CustomEvent(OPEN_MAIN_ROUTE_EVENT, { detail: { path: '/app/knowledge' } }))
    })

    expect(document.querySelector('[data-ui="workbench.automation"]')).toBeTruthy()
    expect(screen.getByRole('button', { name: 'workbench.nav.scheduled' })).toHaveAttribute('aria-current', 'page')
  })

  it('offers connectors and skills without the removed agent catalog', () => {
    renderHome()

    expect(screen.queryByRole('button', { name: 'workbench.nav.experts' })).toBeNull()
    expect(screen.getByRole('button', { name: 'workbench.nav.connectors' })).toBeVisible()
    expect(screen.getByRole('button', { name: 'workbench.nav.skills' })).toBeVisible()
    expect(document.querySelector('[data-ui="workbench.canvas"]')).toBeTruthy()
  })

  it('opens a catalog in the work canvas and keeps the sidebar', () => {
    renderHome({ catalog: 'connector' })

    const connectors = screen.getByRole('button', { name: 'workbench.nav.connectors' })
    expect(connectors).toHaveAttribute('aria-current', 'page')
    expect(connectors).toHaveClass('bg-[#f2f3f5]')
    expect(screen.getByRole('button', { name: 'workbench.nav.skills' })).not.toHaveAttribute('aria-current')
    expect(document.querySelector('[data-ui="capabilities.page"]')).toBeTruthy()
    expect(document.querySelector('[data-ui="workbench.composer"]')).toBeNull()
    expect(screen.getByRole('button', { name: 'workbench.nav.new_task' })).toBeTruthy()
  })

  it('keeps the composer untouched when a feature prompt is clicked', () => {
    vi.mocked(toast.info).mockClear()
    renderHome()

    fireEvent.click(document.querySelector('[data-home="notice"]')!)

    expect(toast.info).not.toHaveBeenCalled()
    expect(screen.getByPlaceholderText('workbench.composer.placeholder')).toHaveValue('')
    expect(screen.getByRole('textbox', { name: 'workbench.writing.hub.brief' })).toHaveValue(
      'workbench.feature.write.blessing'
    )
    expect(screen.getByRole('switch', { name: 'workbench.writing.option.banner' })).toHaveAttribute(
      'aria-checked',
      'true'
    )
    expect(document.querySelector('[data-feature="create"]')).toBeTruthy()
    expect(document.querySelector('[data-feature="pdf"]')).toBeTruthy()
  })

  it('keeps the sidebar beside a conversation and hides the composer', () => {
    renderHome({
      homeActive: false,
      activeSessionId: 'live',
      sessions: [session('live', '2026-10-01T00:00:00.000Z')]
    })

    expect(screen.queryByRole('textbox')).not.toBeInTheDocument()
    expect(screen.queryByRole('searchbox', { name: 'workbench.search' })).not.toBeInTheDocument()
    expect(screen.getByRole('button', { name: 'workbench.search' })).toBeInTheDocument()
    expect(document.querySelector('[data-session-place="project"][data-session-id="live"]')).toBeNull()
    expect(
      document.querySelector('[data-session-place="recent"][data-session-id="live"] [aria-current="page"]')
    ).toBeTruthy()
    expect(document.querySelector('[data-ui="workbench.home"]')).toBeTruthy()
  })

  it('returns home and clears the draft without leaving the workbench', () => {
    const props = renderHome()
    const composer = screen.getByRole('textbox')
    fireEvent.change(composer, { target: { value: 'draft' } })
    fireEvent.click(document.querySelector('[data-new-task-trigger="true"]')!)

    expect(props.onNewTask).toHaveBeenCalledOnce()
    expect(props.onOpenSession).not.toHaveBeenCalled()
    expect(composer).toHaveValue('')
    expect(document.querySelector('[data-ui="workbench.projects"]')).toBeTruthy()
  })

  it('does not write the fallback draft into another task when a new task starts', () => {
    const key = getAgentDraftCacheKey('other-task')
    writeAgentDraftCache(key, {
      text: 'kept-in-other-task',
      tokens: [],
      files: [],
      knowledgeBaseIds: [],
      workspaceKey: '',
      agentId: ''
    })
    const props = renderHome()
    fireEvent.change(screen.getByRole('textbox'), { target: { value: 'home only' } })
    fireEvent.click(document.querySelector('[data-new-task-trigger="true"]')!)

    expect(props.onNewTask).toHaveBeenCalledOnce()
    expect(props.onOpenSession).not.toHaveBeenCalled()
    expect(screen.getByRole('textbox')).toHaveValue('')
    expect(readAgentDraftCache(key, { workspaceKey: '', agentId: '' }).text).toBe('kept-in-other-task')
  })

  it('moves a fallback code send onto a new task', async () => {
    resetHomeComposerModeForTests()
    localStorage.removeItem('workbench.code-session.v1')
    const adopt = vi.fn()
    const create = vi.fn(async () => ({ id: 'task-1' }))
    renderHome({ homeTask: { create, adopt } })
    fireEvent.click(screen.getByRole('radio', { name: '代码开发' }))
    const composer = screen.getByPlaceholderText('workbench.composer.placeholder')
    fireEvent.change(composer, { target: { value: 'Fix login' } })
    fireEvent.click(screen.getByRole('button', { name: 'workbench.composer.send' }))

    await waitFor(() => expect(adopt).toHaveBeenCalledWith('task-1'))
    expect(create).toHaveBeenCalledOnce()
    expect(readAgentDraftCache(getAgentDraftCacheKey('task-1'), { workspaceKey: '', agentId: '' }).text).toBe(
      'Fix login'
    )
    expect(readCodeSession('task-1')).toMatchObject({ placement: 'checkout', topic: 'Fix login', branch: null })
    expect(peekPendingHomeSubmit('task-1')).toBe(true)
    expect(composer).toHaveValue('')
    resetHomeComposerModeForTests()
  })

  it('moves a plain fallback send onto a new task', async () => {
    resetHomeComposerModeForTests()
    const adopt = vi.fn()
    const create = vi.fn(async () => ({ id: 'task-2' }))
    renderHome({ homeTask: { create, adopt } })
    const composer = screen.getByPlaceholderText('workbench.composer.placeholder')
    fireEvent.change(composer, { target: { value: 'hello' } })
    fireEvent.click(screen.getByRole('button', { name: 'workbench.composer.send' }))

    await waitFor(() => expect(adopt).toHaveBeenCalledWith('task-2'))
    expect(readAgentDraftCache(getAgentDraftCacheKey('task-2'), { workspaceKey: '', agentId: '' }).text).toBe('hello')
    expect(composer).toHaveValue('')
    resetHomeComposerModeForTests()
  })

  it('keeps the fallback draft when no task can be created', async () => {
    resetHomeComposerModeForTests()
    vi.mocked(toast.error).mockClear()
    renderHome()
    const composer = screen.getByPlaceholderText('workbench.composer.placeholder')
    fireEvent.change(composer, { target: { value: 'hello' } })
    fireEvent.click(screen.getByRole('button', { name: 'workbench.composer.send' }))

    await waitFor(() => expect(toast.error).toHaveBeenCalledWith('workbench.default_agent.preparing'))
    expect(composer).toHaveValue('hello')
    resetHomeComposerModeForTests()
  })

  it('keeps the fallback draft when a worktree has no project folder', async () => {
    resetHomeComposerModeForTests()
    vi.mocked(toast.error).mockClear()
    const create = vi.fn(async () => ({ id: 'task-3' }))
    renderHome({ homeTask: { create, adopt: vi.fn() } })
    fireEvent.click(screen.getByRole('radio', { name: '代码开发' }))
    const placement = document.querySelector('[data-ui="workbench.composer.code.placement"]') as HTMLElement
    fireEvent.click(within(placement).getByRole('radio', { name: 'workbench.composer.code.worktree' }))
    const composer = screen.getByPlaceholderText('workbench.composer.placeholder')
    fireEvent.change(composer, { target: { value: 'Fix login' } })
    fireEvent.click(screen.getByRole('button', { name: 'workbench.composer.send' }))

    await waitFor(() => expect(toast.error).toHaveBeenCalledWith('先选择一个项目文件夹，再创建工作树'))
    expect(create).not.toHaveBeenCalled()
    expect(composer).toHaveValue('Fix login')
    resetHomeComposerModeForTests()
  })

  it('shows the code workspace choice on the work page', () => {
    renderHome()
    expect(screen.getByRole('radio', { name: '日常办公' })).toHaveAttribute('aria-checked', 'true')
    expect(document.querySelector('[data-ui="workbench.composer.code.placement"]')).toBeNull()

    fireEvent.click(screen.getByRole('radio', { name: '代码开发' }))

    const placement = document.querySelector('[data-ui="workbench.composer.code.placement"]') as HTMLElement
    expect(placement).toBeTruthy()
    expect(within(placement).getByRole('radio', { name: 'workbench.composer.code.checkout' })).toHaveAttribute(
      'aria-checked',
      'true'
    )
    fireEvent.click(within(placement).getByRole('radio', { name: 'workbench.composer.code.worktree' }))
    expect(within(placement).getByRole('combobox', { name: 'workbench.composer.code.branch_pick' })).toBeTruthy()
    resetHomeComposerModeForTests()
  })

  it('opens each pinned office applet in its own window', async () => {
    vi.mocked(openWorkbenchAppletWindow).mockClear()
    renderStrip()
    const strip = document.querySelector('[data-ui="workbench.applets"]') as HTMLElement
    const ids = [...strip.querySelectorAll('[data-applet]')].map((node) => node.getAttribute('data-applet'))
    expect(ids).toEqual(['approval', 'leave', 'travel', 'expense', 'meeting', 'seal'])
    expect(ids).not.toEqual(expect.arrayContaining(['clock', 'probation', 'ai-resource']))

    for (const id of ids) {
      fireEvent.click(within(strip).getByRole('button', { name: `workbench.applet.${id}` }))
    }

    await waitFor(() => expect(openWorkbenchAppletWindow).toHaveBeenCalledTimes(6))
    for (const id of ids) {
      expect(openWorkbenchAppletWindow).toHaveBeenCalledWith(id, `workbench.applet.${id}`)
    }
  })

  it('groups sessions under their project and collapses that project', () => {
    renderHome({
      sessions: [
        { ...session('a', '2026-10-01T00:00:00.000Z', false, 'Traffic lights'), projectId: 'a3s', projectName: 'A3S' },
        { ...session('b', '2026-09-01T00:00:00.000Z', false, 'Other task'), projectId: 'other', projectName: 'Other' }
      ]
    })

    const a3s = document.querySelector('[data-project-id="a3s"]')!
    expect(a3s.querySelector('[data-project-action="toggle"]')).toHaveAttribute('aria-expanded', 'true')
    expect(a3s.querySelector('[data-session-id="a"]')).toBeTruthy()
    expect(document.querySelector('[data-project-id="other"] [data-session-id="b"]')).toBeNull()

    fireEvent.click(a3s.querySelector('[data-project-action="toggle"]')!)
    expect(a3s.querySelector('[data-project-action="toggle"]')).toHaveAttribute('aria-expanded', 'false')
    expect(a3s.querySelector('[data-session-id="a"]')).toBeNull()
    expect(document.querySelector('[data-session-place="recent"][data-session-id="a"]')).toBeNull()
    expect(document.querySelector('[data-session-place="recent"][data-session-id="b"]')).toBeNull()
  })

  it('opens a session from the sidebar and keeps the row identity', () => {
    const props = renderHome({
      sessions: [
        session('a', '2026-10-01T00:00:00.000Z', false, 'Same title'),
        session('b', '2026-09-01T00:00:00.000Z', false, 'Same title')
      ]
    })

    fireEvent.click(document.querySelector('[data-session-place="recent"][data-session-id="b"] button')!)

    expect(props.onOpenSession).toHaveBeenCalledWith('b')
    expect(document.querySelectorAll('[data-session-place="project"]')).toHaveLength(0)
    expect(document.querySelectorAll('[data-session-place="recent"]')).toHaveLength(2)
  })

  it('nests tasks under a project, expands the rest, and still pins or archives', () => {
    const now = Date.now()
    const props = renderHome({
      sessions: [
        { ...session('fresh', new Date(now).toISOString(), false, 'Fresh'), projectId: 'a3s', projectName: 'A3S' },
        ...Array.from({ length: 5 }, (_, index) => ({
          ...session(`older-${index}`, new Date(now - (index + 1) * 60_000).toISOString(), false, `Older ${index}`),
          projectId: 'a3s',
          projectName: 'A3S'
        }))
      ]
    })

    const projectTasks = document.querySelectorAll('[data-session-place="project"]')
    expect(projectTasks).toHaveLength(4)
    expect(document.querySelector('[data-session-place="project"][data-session-id="older-3"]')).toBeNull()
    expect(document.querySelector('[data-session-place="project"][data-session-id="older-4"]')).toBeNull()
    expect(document.querySelector('[data-ui="workbench.projects.expand"]')).toHaveTextContent(
      'workbench.projects.expand'
    )

    fireEvent.click(document.querySelector('[data-ui="workbench.projects.expand"]')!)
    expect(document.querySelector('[data-session-place="project"][data-session-id="older-4"]')).toBeTruthy()
    expect(document.querySelector('[data-session-place="recent"][data-session-id="older-4"]')).toBeNull()

    const fresh = document.querySelector('[data-session-place="project"][data-session-id="fresh"]')!
    fireEvent.click(fresh.querySelector('[data-session-action="pin"]')!)
    expect(props.onTogglePin).toHaveBeenCalledWith('fresh')

    expect(fresh.querySelector('[data-session-action="delete"]')).toBeNull()
    expect(fresh.querySelector('[data-session-action="archive"]')).toHaveClass('hover:bg-black/[0.08]')
    fireEvent.click(fresh.querySelector('[data-session-action="archive"]')!)
    const archiveDialog = screen.getByRole('dialog', { name: 'workbench.session.archive_confirm' })
    fireEvent.click(within(archiveDialog).getByRole('button', { name: 'common.cancel' }))
    expect(props.onDeleteSession).not.toHaveBeenCalled()
  })

  it('places a pinned project task ahead of a newer task in the same project', () => {
    renderHome({
      sessions: [
        {
          ...session('newer', '2026-10-04T00:00:00.000Z', false, 'Newer'),
          projectId: 'a3s',
          projectName: 'A3S'
        },
        {
          ...session('pinned', '2026-10-01T00:00:00.000Z', true, 'Pinned'),
          projectId: 'a3s',
          projectName: 'A3S'
        }
      ]
    })

    const rows = [...document.querySelectorAll('[data-session-place="project"]')].map((row) =>
      row.getAttribute('data-session-id')
    )
    expect(rows).toEqual(['pinned', 'newer'])
    expect(document.querySelector('[data-session-id="pinned"]')).toHaveAttribute('data-pinned', 'true')
    expect(document.querySelector('[data-session-id="pinned"] [data-session-action="pin"]')).toHaveAttribute(
      'aria-label',
      'workbench.session.unpin'
    )
    expect(document.querySelector('[data-session-id="pinned"] [data-session-action="pin"] svg')).toHaveClass(
      'lucide-pin-off'
    )
    expect(document.querySelector('[data-session-id="newer"] [data-session-action="pin"]')).toHaveAttribute(
      'aria-label',
      'workbench.session.pin'
    )
    expect(document.querySelector('[data-session-id="newer"] [data-session-action="pin"] svg')).toHaveClass(
      'lucide-pin'
    )
  })

  it('renames from the row action and cancels with escape', () => {
    const props = renderHome({
      sessions: [session('a', '2026-10-01T00:00:00.000Z', false, 'Old name')]
    })
    const row = document.querySelector('[data-session-place="recent"][data-session-id="a"]')!
    expect(row.querySelector('[data-session-action="rename"]')!.closest('[data-session-actions]')).toHaveClass(
      'absolute'
    )
    expect(row.querySelector('time')).toHaveClass('group-hover:hidden')
    expect(row.querySelector('button')).toHaveClass('group-hover:pr-28')

    fireEvent.click(row.querySelector('[data-session-action="rename"]')!)
    const input = screen.getByRole('textbox', { name: 'workbench.session.rename' })
    fireEvent.change(input, { target: { value: 'New name' } })
    fireEvent.keyDown(input, { key: 'Escape' })
    expect(props.onRenameSession).not.toHaveBeenCalled()

    fireEvent.click(row.querySelector('[data-session-action="rename"]')!)
    const next = screen.getByRole('textbox', { name: 'workbench.session.rename' })
    fireEvent.change(next, { target: { value: 'New name' } })
    fireEvent.keyDown(next, { key: 'Enter' })
    expect(props.onRenameSession).toHaveBeenCalledWith('a', 'New name')
  })

  it('hosts the Cherry composer and clears it through the bridge', async () => {
    const fill = vi.fn()
    const focus = vi.fn()
    const unregister = registerWorkbenchComposer({ fill, focus })
    const onComposerHostChange = vi.fn()
    renderHome({ useLiveComposer: true, onComposerHostChange })

    expect(screen.queryByRole('textbox')).not.toBeInTheDocument()
    expect(document.querySelector('[data-ui="workbench.composer"]')).toBeTruthy()
    expect(document.querySelector('[aria-label="workbench.composer.mic"]')).toBeNull()
    expect(onComposerHostChange).toHaveBeenCalledWith(expect.any(HTMLElement))

    vi.mocked(toast.info).mockClear()
    fireEvent.click(document.querySelector('[data-home="notice"]')!)
    expect(fill).not.toHaveBeenCalled()
    expect(toast.info).not.toHaveBeenCalled()
    expect(document.querySelector('[data-writing-stage]')).toBeTruthy()

    fireEvent.click(document.querySelector('[data-new-task-trigger="true"]')!)
    expect(fill).toHaveBeenCalledWith('')
    unregister()
  })

  it('puts the input back when the live composer has not arrived', async () => {
    renderHome({ useLiveComposer: true, onComposerHostChange: vi.fn() })

    expect(screen.queryByRole('textbox')).not.toBeInTheDocument()

    await act(async () => {
      await new Promise((resolve) => {
        requestAnimationFrame(() => requestAnimationFrame(() => resolve(undefined)))
      })
    })

    expect(screen.getByRole('textbox')).toHaveAttribute('placeholder', 'workbench.composer.placeholder')
    expect(screen.getByRole('button', { name: 'workbench.composer.capture' })).toBeInTheDocument()
    expect(screen.getAllByRole('textbox')).toHaveLength(1)
    expect(screen.getAllByText('workbench.composer.model')).toHaveLength(1)
    fireEvent.click(screen.getByText('workbench.composer.model'))
    expect(screen.getAllByRole('textbox')).toHaveLength(1)
    expect(screen.getAllByText('workbench.composer.model')).toHaveLength(1)
    expect(document.querySelector('[data-ui="workbench.composer"]')).toBeTruthy()
  })

  it('stages files dropped on the homepage placeholder', async () => {
    renderHome({ useLiveComposer: true, onComposerHostChange: vi.fn() })
    await act(async () => {
      await new Promise((resolve) => {
        requestAnimationFrame(() => requestAnimationFrame(() => resolve(undefined)))
      })
    })
    const shell = document.querySelector('[data-ui="workbench.composer.fallback"]') as HTMLElement
    const received: File[][] = []
    const stop = subscribeWorkbenchDroppedFiles((files) => {
      received.push(files)
    })
    const report = new File(['abc'], 'report.pdf', { type: 'application/pdf' })
    const files = { types: ['Files'], files: [report], dropEffect: 'none' }

    fireEvent.dragEnter(shell, { dataTransfer: { types: ['text/plain'], files: [] } })
    expect(shell).not.toHaveAttribute('data-drag-over')

    fireEvent.dragEnter(shell, { dataTransfer: files })
    fireEvent.dragEnter(shell, { dataTransfer: files })
    expect(shell).toHaveAttribute('data-drag-over', 'true')
    expect(shell.querySelector('[data-ui="composer.file-drop"]')).toHaveTextContent('chat.input.drop_files')

    fireEvent.dragLeave(shell, { dataTransfer: files })
    expect(shell).toHaveAttribute('data-drag-over', 'true')

    fireEvent.drop(shell, { dataTransfer: files })
    expect(received).toHaveLength(1)
    expect(received[0]?.map((file) => file.name)).toEqual(['report.pdf'])
    expect(shell).not.toHaveAttribute('data-drag-over')
    stop()
  })

  it('leaves the portaled composer in place once it arrives', async () => {
    const onComposerHostChange = vi.fn()
    renderHome({ useLiveComposer: true, onComposerHostChange })
    const host = onComposerHostChange.mock.calls.at(-1)?.[0] as HTMLElement
    const input = document.createElement('textarea')
    input.setAttribute('data-composer-inputbar', '')

    await act(async () => {
      host.append(input)
      await new Promise((resolve) => {
        requestAnimationFrame(() => requestAnimationFrame(() => resolve(undefined)))
      })
    })

    expect(host.querySelector('textarea')).toBe(input)
    expect(screen.queryByPlaceholderText('workbench.composer.placeholder')).not.toBeInTheDocument()
  })

  it('keeps a react portal in the slot when the homepage renders again', async () => {
    function Harness() {
      const [host, setHost] = useState<HTMLElement | null>(null)
      const [tick, setTick] = useState(0)
      return (
        <>
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
          {host ? createPortal(<textarea data-testid="live-composer" />, host) : null}
          <button type="button" onClick={() => setTick((value) => value + 1)}>
            tick {tick}
          </button>
        </>
      )
    }

    render(<Harness />)
    await act(async () => {
      await Promise.resolve()
    })
    const live = screen.getByTestId('live-composer')
    expect(live.parentElement).toHaveAttribute('data-ui', 'workbench.composer')

    fireEvent.click(screen.getByRole('button', { name: /tick/ }))

    expect(screen.getByTestId('live-composer')).toBe(live)
    expect(live.isConnected).toBe(true)
    expect(screen.queryByPlaceholderText('workbench.composer.placeholder')).not.toBeInTheDocument()
  })

  it('submits each office form as a field object', async () => {
    const cases = [
      {
        id: 'travel' as const,
        fill: async () => {
          fireEvent.change(screen.getByRole('textbox', { name: 'workbench.applet.travel.destination' }), {
            target: { value: '上海' }
          })
          fireEvent.click(document.querySelector('[data-ui="workbench.applet.travel.start"]')!)
          await chooseMarkedDay('workbench.applet.travel.range', '2026-10-06')
          await chooseMarkedDay('workbench.applet.travel.range', '2026-10-08')
          fireEvent.change(document.querySelector('textarea[name="reason"]')!, { target: { value: '客户拜访' } })
        },
        values: {
          destination: '上海',
          start_date: '2026-10-06',
          end_date: '2026-10-08',
          reason: '客户拜访'
        }
      },
      {
        id: 'expense' as const,
        fill: async () => {
          const amount = screen.getByRole('textbox', { name: 'workbench.applet.expense.amount' })
          fireEvent.change(amount, { target: { value: '0' } })
          expect(screen.getByRole('button', { name: 'workbench.applet.submit' })).toBeDisabled()
          fireEvent.change(amount, { target: { value: '128.50' } })
          await chooseOption('workbench.applet.expense.category', 'workbench.applet.expense.category.transport')
          fireEvent.click(document.querySelector('[data-ui="workbench.applet.expense.date"]')!)
          await chooseMarkedDay('workbench.applet.expense.calendar', '2026-10-07')
          fireEvent.change(document.querySelector('textarea[name="reason"]')!, { target: { value: '高铁' } })
        },
        values: {
          amount: '128.50',
          category: '交通',
          spent_on: '2026-10-07',
          detail: '高铁'
        }
      },
      {
        id: 'meeting' as const,
        fill: async () => {
          fireEvent.change(screen.getByRole('textbox', { name: 'workbench.applet.meeting.subject' }), {
            target: { value: '周例会' }
          })
          fireEvent.click(document.querySelector('[data-ui="workbench.applet.meeting.day"]')!)
          await chooseMarkedDay('workbench.applet.meeting.calendar', '2026-10-07')
          await chooseOption('workbench.applet.meeting.start', '09:00')
          await chooseOption('workbench.applet.meeting.end', '10:00')
          fireEvent.change(screen.getByRole('textbox', { name: 'workbench.applet.meeting.place' }), {
            target: { value: '三楼' }
          })
          fireEvent.change(document.querySelector('textarea[name="agenda"]')!, { target: { value: '同步进度' } })
        },
        values: {
          title: '周例会',
          day: '2026-10-07',
          start_time: '09:00',
          end_time: '10:00',
          place: '三楼',
          agenda: '同步进度'
        }
      },
      {
        id: 'seal' as const,
        fill: async () => {
          fireEvent.change(screen.getByRole('textbox', { name: 'workbench.applet.seal.document' }), {
            target: { value: '采购合同' }
          })
          await chooseOption('workbench.applet.seal.kind', 'workbench.applet.seal.kind.official')
          const copies = screen.getByRole('textbox', { name: 'workbench.applet.seal.copies' })
          fireEvent.change(copies, { target: { value: '0' } })
          expect(screen.getByRole('button', { name: 'workbench.applet.submit' })).toBeDisabled()
          fireEvent.change(copies, { target: { value: '2' } })
          fireEvent.change(document.querySelector('textarea[name="reason"]')!, { target: { value: '签约' } })
        },
        values: {
          document: '采购合同',
          seal_type: '公章',
          copies: '2',
          purpose: '签约'
        }
      },
      {
        id: 'notice' as const,
        fill: async () => {
          fireEvent.change(screen.getByRole('textbox', { name: 'workbench.applet.notice.title' }), {
            target: { value: '放假安排' }
          })
          fireEvent.change(document.querySelector('textarea[name="body"]')!, { target: { value: '周五下午放假' } })
        },
        values: { title: '放假安排', detail: '周五下午放假' }
      }
    ]
    try {
      for (const item of cases) {
        const body = PACKAGE_BODY.replace('"appId":"leave"', `"appId":"${item.id}"`)
        const digest = await sha256Hex(new TextEncoder().encode(body))
        installAppletRuntime(item.id)
        const fetchImpl = appletFetch(digest, { body })
        vi.stubGlobal('fetch', fetchImpl)
        const view = render(<WorkbenchAppletWindow appletId={item.id} />)
        expect(document.querySelector('[data-ui="workbench.applet.window"]')).toHaveAttribute('data-applet', item.id)
        expect(document.querySelector('[data-ui="workbench.applet.loading"]')).toBeTruthy()
        await waitFor(() => {
          expect(document.querySelector('[data-ui="workbench.applet.permission"]')).toBeTruthy()
        })
        expect(document.querySelector('[data-ui="workbench.applet.form"]')).toBeNull()
        fireEvent.click(screen.getByRole('button', { name: 'workbench.applet.permission.allow' }))
        await waitFor(() => {
          expect(document.querySelector('[data-ui="workbench.applet.form"]')).toBeTruthy()
        })
        expect(document.querySelector('[data-ui="workbench.applet.body"]')?.className).toContain('overflow-y-auto')
        expect(document.querySelector('[data-ui="workbench.applet.building"]')).toBeNull()
        expect(document.querySelector('[data-lottie="construction-crew"]')).toBeNull()
        expect(document.querySelector('select:not([aria-hidden="true"])')).toBeNull()
        expect(document.querySelector('input[type="date"], input[type="time"]')).toBeNull()
        expect(screen.getByText(`workbench.applet.${item.id}.lead`)).toHaveClass('text-[15px]', 'text-[#8a8a8a]')
        const submit = screen.getByRole('button', { name: 'workbench.applet.submit' })
        expect(submit).toBeDisabled()
        await item.fill()
        await waitFor(() => expect(submit).toBeEnabled())
        fireEvent.click(submit)
        await waitFor(() => {
          expect(screen.getByText('workbench.applet.submitted')).toBeInTheDocument()
        })
        expect(screen.queryByText('workbench.applet.submitted.team')).not.toBeInTheDocument()
        expect(document.querySelector('[data-ui="workbench.applet.submitted.item"]')).toBeNull()
        const posts = fetchImpl.mock.calls.filter((call) => call[1]?.method === 'POST')
        expect(posts.map((call) => String(call[0]))).toEqual([
          `http://127.0.0.1:9/api/applet-runtime/${item.id}/submissions`
        ])
        expect(JSON.parse(String(posts[0]?.[1]?.body)).values).toEqual(item.values)
        expect(document.body.textContent).not.toContain('13800138000')
        view.unmount()
        vi.unstubAllGlobals()
        removeAppletRuntime()
      }
    } finally {
      vi.unstubAllGlobals()
      removeAppletRuntime()
    }
  })

  it('routes a team office submission into the approval center', async () => {
    const body = PACKAGE_BODY.replace('"appId":"leave"', '"appId":"travel"')
    const digest = await sha256Hex(new TextEncoder().encode(body))
    installAppletRuntime('travel')
    const fetchImpl = appletFetch(digest, { body, centerId: 'org-1' })
    vi.stubGlobal('fetch', fetchImpl)
    try {
      render(<WorkbenchAppletWindow appletId="travel" />)
      await waitFor(() => {
        expect(document.querySelector('[data-ui="workbench.applet.permission"]')).toBeTruthy()
      })
      fireEvent.click(screen.getByRole('button', { name: 'workbench.applet.permission.allow' }))
      await waitFor(() => {
        expect(document.querySelector('[data-ui="workbench.applet.form"]')).toBeTruthy()
      })
      fireEvent.change(screen.getByRole('textbox', { name: 'workbench.applet.travel.destination' }), {
        target: { value: '上海' }
      })
      fireEvent.click(document.querySelector('[data-ui="workbench.applet.travel.start"]')!)
      await chooseMarkedDay('workbench.applet.travel.range', '2026-10-06')
      await chooseMarkedDay('workbench.applet.travel.range', '2026-10-08')
      fireEvent.change(document.querySelector('textarea[name="reason"]')!, { target: { value: '客户拜访' } })
      await waitFor(() => {
        expect(document.querySelector('[data-ui="workbench.applet.route"]')).toHaveTextContent('直属主管')
        expect(screen.getByRole('button', { name: 'workbench.applet.submit' })).toBeEnabled()
      })
      expect(document.querySelector('[data-ui="workbench.applet.approvers"]')).toBeNull()
      fireEvent.click(screen.getByRole('button', { name: 'workbench.applet.submit' }))
      await waitFor(() => {
        expect(screen.getByText('workbench.applet.submitted.team')).toBeInTheDocument()
      })
      expect(document.querySelector('[data-ui="workbench.applet.submitted.item"]')).toHaveTextContent('wi-1')
      const posts = fetchImpl.mock.calls.filter((call) => call[1]?.method === 'POST')
      expect(posts.map((call) => String(call[0]))).toEqual([
        'http://127.0.0.1:9/api/applet-runtime/travel/work-items/process-preview',
        'http://127.0.0.1:9/api/applet-runtime/travel/work-items/process'
      ])
      expect(posts.map((call) => String(call[0])).some((url) => url.endsWith('/work-items'))).toBe(false)
      const posted = JSON.parse(String(posts[1]?.[1]?.body)) as {
        org_id: string
        process_key: string
        idempotency_key: string
        form: Record<string, string>
      }
      expect(posted.idempotency_key).toMatch(/^[0-9a-f-]{36}$/)
      expect(posted).toMatchObject({
        org_id: 'org-1',
        process_key: 'travel',
        form: {
          destination: '上海',
          start_date: '2026-10-06',
          end_date: '2026-10-08',
          reason: '客户拜访'
        }
      })
    } finally {
      vi.unstubAllGlobals()
      removeAppletRuntime()
    }
  })

  it('submits leave and expense through preview and start', async () => {
    const cases = [
      {
        id: 'leave',
        processKey: 'leave',
        fill: async () => {
          await chooseOption('workbench.applet.leave.type', 'workbench.applet.leave.type.personal')
          fireEvent.click(document.querySelector('[data-ui="workbench.applet.leave.start"]')!)
          await chooseLeaveDay('2026-10-05')
          fireEvent.click(document.querySelector('[data-ui="workbench.applet.leave.end"]')!)
          await chooseLeaveDay('2026-10-08')
          fireEvent.change(document.querySelector('textarea[name="reason"]')!, { target: { value: '家事' } })
        },
        form: {
          leave_type: '事假',
          start_date: '2026-10-05',
          end_date: '2026-10-08',
          reason: '家事'
        }
      },
      {
        id: 'expense',
        processKey: 'expense',
        fill: async () => {
          fireEvent.change(screen.getByRole('textbox', { name: 'workbench.applet.expense.amount' }), {
            target: { value: '128.50' }
          })
          await chooseOption('workbench.applet.expense.category', 'workbench.applet.expense.category.transport')
          fireEvent.click(document.querySelector('[data-ui="workbench.applet.expense.date"]')!)
          await chooseMarkedDay('workbench.applet.expense.calendar', '2026-10-07')
          fireEvent.change(document.querySelector('textarea[name="reason"]')!, { target: { value: '高铁' } })
        },
        form: {
          amount: '128.50',
          category: '交通',
          spent_on: '2026-10-07',
          detail: '高铁'
        }
      }
    ] as const
    try {
      for (const item of cases) {
        const body = PACKAGE_BODY.replace('"appId":"leave"', `"appId":"${item.id}"`)
        const digest = await sha256Hex(new TextEncoder().encode(body))
        installAppletRuntime(item.id)
        const fetchImpl = appletFetch(digest, { body, centerId: 'org-1' })
        vi.stubGlobal('fetch', fetchImpl)
        const view = render(<WorkbenchAppletWindow appletId={item.id} />)
        await waitFor(() => {
          expect(document.querySelector('[data-ui="workbench.applet.permission"]')).toBeTruthy()
        })
        fireEvent.click(screen.getByRole('button', { name: 'workbench.applet.permission.allow' }))
        await waitFor(() => {
          expect(document.querySelector('[data-ui="workbench.applet.form"]')).toBeTruthy()
        })
        expect(document.querySelector('select, datalist')).toBeNull()
        await item.fill()
        await waitFor(() => {
          expect(screen.getByRole('button', { name: 'workbench.applet.submit' })).toBeEnabled()
          expect(document.querySelector('[data-ui="workbench.applet.route"]')).toHaveTextContent('直属主管')
        })
        expect(document.querySelector('[data-ui="workbench.applet.approvers"]')).toBeNull()
        fireEvent.click(screen.getByRole('button', { name: 'workbench.applet.submit' }))
        await waitFor(() => {
          expect(screen.getByText('workbench.applet.submitted.team')).toBeInTheDocument()
        })
        const posts = fetchImpl.mock.calls.filter((call) => call[1]?.method === 'POST')
        expect(posts.map((call) => String(call[0]))).toEqual([
          `http://127.0.0.1:9/api/applet-runtime/${item.id}/work-items/process-preview`,
          `http://127.0.0.1:9/api/applet-runtime/${item.id}/work-items/process`
        ])
        const started = JSON.parse(String(posts[1]?.[1]?.body)) as { process_key: string; form: Record<string, string> }
        expect(started.process_key).toBe(item.processKey)
        expect(started.form).toEqual(item.form)
        view.unmount()
        vi.unstubAllGlobals()
        removeAppletRuntime()
      }
    } finally {
      vi.unstubAllGlobals()
      removeAppletRuntime()
    }
  })

  it('submits a clock punch, a correction, and an ai member id through the process entry', async () => {
    const ownerId = '00000000-0000-0000-0000-000000000006'
    const cases = [
      {
        id: 'clock' as const,
        processKey: 'clock',
        approvers: [{ id: 'a1', name: '周' }],
        fill: async () => {
          fireEvent.click(screen.getByRole('radio', { name: 'workbench.applet.clock.type.in' }))
        },
        form: { punch_type: '上班' }
      },
      {
        id: 'correction' as const,
        processKey: 'attendance_correction',
        approvers: [{ id: 'a1', name: '周' }],
        fill: async () => {
          fireEvent.click(document.querySelector('[data-ui="workbench.applet.clock.date"]')!)
          await chooseMarkedDay('workbench.applet.clock.calendar', '2026-10-14')
          await chooseOption('workbench.applet.clock.type', 'workbench.applet.clock.type.out')
          fireEvent.change(screen.getByRole('textbox', { name: 'workbench.applet.clock.time' }), {
            target: { value: '18:30' }
          })
          fireEvent.change(document.querySelector('textarea[name="reason"]')!, { target: { value: '漏打卡' } })
        },
        form: {
          punch_date: '2026-10-14',
          punch_type: '下班',
          corrected_time: '18:30',
          reason: '漏打卡'
        }
      },
      {
        id: 'ai-resource' as const,
        processKey: 'ai_demo_resource',
        approvers: [{ id: ownerId, name: '研发负责人' }],
        fill: async () => {
          fireEvent.change(screen.getByRole('textbox', { name: 'workbench.applet.ai.project' }), {
            target: { value: '演示项目' }
          })
          fireEvent.change(document.querySelector('textarea[name="reason"]')!, { target: { value: '评测' } })
          fireEvent.change(screen.getByRole('textbox', { name: 'workbench.applet.ai.owner' }), {
            target: { value: '林知夏' }
          })
          await chooseOption('workbench.applet.ai.rd_owner', '研发负责人')
          fireEvent.change(screen.getByRole('textbox', { name: 'workbench.applet.ai.days' }), {
            target: { value: '3' }
          })
          fireEvent.click(document.querySelector('[data-ui="workbench.applet.ai.start"]')!)
          await chooseMarkedDay('workbench.applet.ai.range', '2026-10-20')
          fireEvent.click(document.querySelector('[data-ui="workbench.applet.ai.end"]')!)
          await chooseMarkedDay('workbench.applet.ai.range', '2026-10-27')
          await chooseOption('workbench.applet.ai.type', 'workbench.applet.ai.type.compute')
          fireEvent.change(document.querySelector('textarea[name="spec"]')!, { target: { value: '一台' } })
          fireEvent.change(document.querySelector('textarea[name="output"]')!, { target: { value: '报告' } })
        },
        form: {
          project_name: '演示项目',
          test_reason: '评测',
          owner: '林知夏',
          rd_owner: ownerId,
          days: '3',
          start_date: '2026-10-20',
          end_date: '2026-10-27',
          resource_type: '算力',
          resource_note: '一台',
          expected_output: '报告'
        }
      }
    ]
    try {
      for (const item of cases) {
        const body = PACKAGE_BODY.replace('"appId":"leave"', `"appId":"${item.id}"`)
        const digest = await sha256Hex(new TextEncoder().encode(body))
        installAppletRuntime(item.id)
        const fetchImpl = appletFetch(digest, { body, centerId: 'org-1', approvers: item.approvers })
        vi.stubGlobal('fetch', fetchImpl)
        const view = render(<WorkbenchAppletWindow appletId={item.id} />)
        await waitFor(() => {
          expect(document.querySelector('[data-ui="workbench.applet.permission"]')).toBeTruthy()
        })
        fireEvent.click(screen.getByRole('button', { name: 'workbench.applet.permission.allow' }))
        await waitFor(() => {
          expect(document.querySelector('[data-ui="workbench.applet.form"]')).toBeTruthy()
        })
        expect(document.querySelector('select, datalist')).toBeNull()
        expect(document.querySelector('input[name="rd_owner"]')).toBeNull()
        if (item.id === 'clock') {
          expect(screen.queryByRole('combobox', { name: 'workbench.applet.clock.type' })).toBeNull()
          expect(screen.getByRole('radio', { name: 'workbench.applet.clock.type.in' })).toHaveAttribute(
            'aria-checked',
            'false'
          )
        }
        await item.fill()
        await waitFor(() => {
          expect(screen.getByRole('button', { name: 'workbench.applet.submit' })).toBeEnabled()
          expect(document.querySelector('[data-ui="workbench.applet.route"]')).toHaveTextContent(
            item.id === 'clock' ? 'workbench.applet.clock.recorded' : '直属主管'
          )
        })
        expect(document.querySelector('[data-ui="workbench.applet.approvers"]')).toBeNull()
        fireEvent.click(screen.getByRole('button', { name: 'workbench.applet.submit' }))
        await waitFor(() => {
          expect(screen.getByText('workbench.applet.submitted.team')).toBeInTheDocument()
        })
        const posts = fetchImpl.mock.calls.filter((call) => call[1]?.method === 'POST')
        expect(posts.map((call) => String(call[0]))).toEqual([
          `http://127.0.0.1:9/api/applet-runtime/${item.id}/work-items/process-preview`,
          `http://127.0.0.1:9/api/applet-runtime/${item.id}/work-items/process`
        ])
        const started = JSON.parse(String(posts[1]?.[1]?.body)) as { process_key: string; form: Record<string, string> }
        expect(started.process_key).toBe(item.processKey)
        expect(started.form).toEqual(item.form)
        view.unmount()
        vi.unstubAllGlobals()
        removeAppletRuntime()
      }
    } finally {
      vi.unstubAllGlobals()
      removeAppletRuntime()
    }
  })

  it('replaces the approval route when a later preview returns a different one', async () => {
    const body = PACKAGE_BODY.replace('"appId":"leave"', '"appId":"expense"')
    const digest = await sha256Hex(new TextEncoder().encode(body))
    installAppletRuntime('expense')
    const fetchImpl = appletFetch(digest, {
      body,
      centerId: 'org-1',
      preview: (raw) => {
        const amount = (JSON.parse(raw) as { form?: { amount?: string } }).form?.amount
        const nodes = [
          { key: 'manager', name: '直属主管', assignees: [] as string[] },
          { key: 'finance', name: '财务', assignees: [] as string[] }
        ]
        if (amount === '6000') nodes.push({ key: 'finance_lead', name: '财务负责人', assignees: [] })
        return { payload: { version: 1, nodes } }
      }
    })
    vi.stubGlobal('fetch', fetchImpl)
    try {
      render(<WorkbenchAppletWindow appletId="expense" />)
      await waitFor(() => {
        expect(document.querySelector('[data-ui="workbench.applet.permission"]')).toBeTruthy()
      })
      fireEvent.click(screen.getByRole('button', { name: 'workbench.applet.permission.allow' }))
      await waitFor(() => {
        expect(document.querySelector('[data-ui="workbench.applet.form"]')).toBeTruthy()
      })
      fireEvent.change(screen.getByRole('textbox', { name: 'workbench.applet.expense.amount' }), {
        target: { value: '128.50' }
      })
      await chooseOption('workbench.applet.expense.category', 'workbench.applet.expense.category.transport')
      fireEvent.click(document.querySelector('[data-ui="workbench.applet.expense.date"]')!)
      await chooseMarkedDay('workbench.applet.expense.calendar', '2026-10-07')
      fireEvent.change(document.querySelector('textarea[name="reason"]')!, { target: { value: '高铁' } })
      await waitFor(() => {
        const route = document.querySelector('[data-ui="workbench.applet.route"]')
        expect(route).toHaveTextContent('财务')
        expect(route).not.toHaveTextContent('财务负责人')
      })
      fireEvent.change(screen.getByRole('textbox', { name: 'workbench.applet.expense.amount' }), {
        target: { value: '6000' }
      })
      await waitFor(() => {
        expect(document.querySelector('[data-ui="workbench.applet.route"]')).toHaveTextContent('财务负责人')
        expect(screen.getByRole('button', { name: 'workbench.applet.submit' })).toBeEnabled()
      })
      const posts = fetchImpl.mock.calls.filter((call) => call[1]?.method === 'POST')
      expect(posts.map((call) => String(call[0]))).toEqual([
        'http://127.0.0.1:9/api/applet-runtime/expense/work-items/process-preview',
        'http://127.0.0.1:9/api/applet-runtime/expense/work-items/process-preview'
      ])
    } finally {
      vi.unstubAllGlobals()
      removeAppletRuntime()
    }
  })

  it('keeps submit off and shows the preview refusal until a later preview accepts', async () => {
    const body = PACKAGE_BODY.replace('"appId":"leave"', '"appId":"expense"')
    const digest = await sha256Hex(new TextEncoder().encode(body))
    installAppletRuntime('expense')
    const fetchImpl = appletFetch(digest, {
      body,
      centerId: 'org-1',
      preview: (raw) => {
        const amount = (JSON.parse(raw) as { form?: { amount?: string } }).form?.amount
        if (amount === '1') return { status: 400, payload: { error: '直属主管未设置' } }
        return { payload: { version: 1, nodes: [{ key: 'manager', name: '直属主管', assignees: [] }] } }
      }
    })
    vi.stubGlobal('fetch', fetchImpl)
    try {
      render(<WorkbenchAppletWindow appletId="expense" />)
      await waitFor(() => {
        expect(document.querySelector('[data-ui="workbench.applet.permission"]')).toBeTruthy()
      })
      fireEvent.click(screen.getByRole('button', { name: 'workbench.applet.permission.allow' }))
      await waitFor(() => {
        expect(document.querySelector('[data-ui="workbench.applet.form"]')).toBeTruthy()
      })
      fireEvent.change(screen.getByRole('textbox', { name: 'workbench.applet.expense.amount' }), {
        target: { value: '1' }
      })
      await chooseOption('workbench.applet.expense.category', 'workbench.applet.expense.category.transport')
      fireEvent.click(document.querySelector('[data-ui="workbench.applet.expense.date"]')!)
      await chooseMarkedDay('workbench.applet.expense.calendar', '2026-10-07')
      fireEvent.change(document.querySelector('textarea[name="reason"]')!, { target: { value: '高铁' } })
      await waitFor(() => {
        expect(document.querySelector('[data-ui="workbench.applet.route"]')).toHaveTextContent('直属主管未设置')
      })
      expect(screen.getByRole('button', { name: 'workbench.applet.submit' })).toBeDisabled()
      expect(screen.queryByText('workbench.applet.submit_failed')).not.toBeInTheDocument()
      fireEvent.change(screen.getByRole('textbox', { name: 'workbench.applet.expense.amount' }), {
        target: { value: '128.50' }
      })
      await waitFor(() => {
        expect(document.querySelector('[data-ui="workbench.applet.route"]')).toHaveTextContent('直属主管')
        expect(screen.getByRole('button', { name: 'workbench.applet.submit' })).toBeEnabled()
      })
      expect(document.body.textContent).not.toContain('直属主管未设置')
      const posts = fetchImpl.mock.calls.filter((call) => call[1]?.method === 'POST')
      expect(posts.every((call) => String(call[0]).endsWith('/work-items/process-preview'))).toBe(true)
      expect(posts).toHaveLength(2)
    } finally {
      vi.unstubAllGlobals()
      removeAppletRuntime()
    }
  })

  it('keeps submit off when the team has no other approver', async () => {
    const body = PACKAGE_BODY.replace('"appId":"leave"', '"appId":"approval"')
    const digest = await sha256Hex(new TextEncoder().encode(body))
    installAppletRuntime('approval')
    const fetchImpl = appletFetch(digest, { body, centerId: 'org-1', approvers: [] })
    vi.stubGlobal('fetch', fetchImpl)
    try {
      render(<WorkbenchAppletWindow appletId="approval" />)
      await waitFor(() => {
        expect(document.querySelector('[data-ui="workbench.applet.permission"]')).toBeTruthy()
      })
      fireEvent.click(screen.getByRole('button', { name: 'workbench.applet.permission.allow' }))
      await waitFor(() => {
        expect(document.querySelector('[data-ui="workbench.applet.form"]')).toBeTruthy()
      })
      fireEvent.change(screen.getByRole('textbox', { name: 'workbench.applet.field.title' }), {
        target: { value: '采购申请' }
      })
      fireEvent.change(screen.getByRole('textbox', { name: 'workbench.applet.field.detail' }), {
        target: { value: '需要两台显示器' }
      })
      await waitFor(() => {
        expect(document.querySelector('[data-ui="workbench.applet.no-approver"]')).toBeTruthy()
      })
      expect(screen.getByRole('button', { name: 'workbench.applet.submit' })).toBeDisabled()
      expect(fetchImpl.mock.calls.filter((call) => call[1]?.method === 'POST')).toHaveLength(0)
    } finally {
      vi.unstubAllGlobals()
      removeAppletRuntime()
    }
  })

  it('submits the approval form through the shared title and detail fields', async () => {
    const body = PACKAGE_BODY.replace('"appId":"leave"', '"appId":"approval"')
    const digest = await sha256Hex(new TextEncoder().encode(body))
    installAppletRuntime('approval')
    const fetchImpl = appletFetch(digest, { body })
    vi.stubGlobal('fetch', fetchImpl)
    try {
      render(<WorkbenchAppletWindow appletId="approval" />)
      await waitFor(() => {
        expect(document.querySelector('[data-ui="workbench.applet.permission"]')).toBeTruthy()
      })
      expect(document.querySelector('[data-ui="workbench.applet.form"]')).toBeNull()
      fireEvent.click(screen.getByRole('button', { name: 'workbench.applet.permission.allow' }))
      await waitFor(() => {
        expect(document.querySelector('[data-ui="workbench.applet.form"]')).toBeTruthy()
      })
      expect(document.querySelector('[data-ui="workbench.applet.body"]')?.className).toContain('overflow-y-auto')
      expect(document.querySelector('[data-ui="workbench.applet.building"]')).toBeNull()
      expect(document.querySelector('[data-ui="workbench.applet.permission"]')).toBeNull()
      expect(screen.getByText('workbench.applet.approval.lead')).toHaveClass('text-[15px]', 'text-[#8a8a8a]')
      const title = screen.getByRole('textbox', { name: 'workbench.applet.field.title' })
      const detail = screen.getByRole('textbox', { name: 'workbench.applet.field.detail' })
      expect(title).toHaveClass('h-9', 'rounded-[8px]', 'bg-[#f5f5f5]', 'text-[13px]')
      expect(detail).toHaveClass('h-40', 'rounded-[8px]', 'bg-[#f5f5f5]')
      const submit = screen.getByRole('button', { name: 'workbench.applet.submit' })
      expect(submit).toBeDisabled()
      fireEvent.change(title, { target: { value: '  采购申请  ' } })
      fireEvent.change(detail, { target: { value: '  需要两台显示器  ' } })
      expect(submit).toBeEnabled()
      fireEvent.click(submit)
      await waitFor(() => {
        expect(screen.getByText('workbench.applet.submitted')).toBeInTheDocument()
      })
      const posted = fetchImpl.mock.calls.find((call) => call[1]?.method === 'POST')
      const postedBody = JSON.parse(String(posted?.[1]?.body)) as { values: { title: string; detail: string } }
      expect(postedBody.values).toEqual({ title: '采购申请', detail: '需要两台显示器' })
      expect(document.querySelector('[data-ui="workbench.applet.leave.profile"]')).toBeNull()
      expect(document.body.textContent).not.toContain('13800138000')
      fireEvent.click(screen.getByRole('button', { name: 'workbench.applet.nav.refresh' }))
      expect(document.querySelector('[data-ui="workbench.applet.loading"]')).toBeTruthy()
      await waitFor(() => {
        expect(document.querySelector('[data-ui="workbench.applet.form"]')).toBeTruthy()
      })
      expect(screen.getByRole('textbox', { name: 'workbench.applet.field.title' })).toHaveValue('')
      expect(screen.queryByText('workbench.applet.submitted')).not.toBeInTheDocument()
    } finally {
      vi.unstubAllGlobals()
      removeAppletRuntime()
    }
  })

  it('loads the applet system window and drives its native controls', async () => {
    const digest = await sha256Hex(new TextEncoder().encode(PACKAGE_BODY))
    installAppletRuntime()
    const fetchImpl = appletFetch(digest)
    vi.stubGlobal('fetch', fetchImpl)
    try {
      render(<WorkbenchAppletWindow appletId="leave" />)
      const panel = document.querySelector('[data-ui="workbench.applet.window"]')
      expect(panel).toHaveAttribute('data-applet', 'leave')
      expect(panel).toHaveAttribute('data-presentation', 'window')
      expect(document.querySelector('[data-ui="workbench.applet.loading"]')).toBeTruthy()
      expect(document.querySelector('.workbench-applet-load-bar')).toBeTruthy()

      await waitFor(() => {
        expect(document.querySelector('[data-ui="workbench.applet.permission"]')).toBeTruthy()
      })
      expect(fetchImpl).toHaveBeenCalledWith(
        'http://127.0.0.1:9/api/applet-runtime/leave/session',
        expect.objectContaining({ method: 'GET' })
      )
      expect(document.querySelector('[data-ui="workbench.applet.form"]')).toBeNull()
      expect(screen.queryByText('林')).not.toBeInTheDocument()
      expect(document.querySelector('[data-ui="workbench.applet.leave.profile"]')).toBeNull()
      expect(document.body.textContent).not.toContain('13800138000')
      expect(panel).toHaveAttribute('data-applet-version', '1.1.0')
      fireEvent.click(screen.getByRole('button', { name: 'workbench.applet.permission.allow' }))
      await waitFor(() => {
        expect(document.querySelector('[data-ui="workbench.applet.form"]')).toBeTruthy()
      })
      expect(document.querySelector('[data-ui="workbench.applet.permission"]')).toBeNull()
      const prompt = screen.getByText('workbench.applet.leave.lead')
      expect(prompt).toHaveClass('text-[15px]', 'text-[#8a8a8a]')
      expect(screen.getByRole('button', { name: 'workbench.applet.nav.back' })).toBeDisabled()
      expect(screen.getByRole('button', { name: 'workbench.applet.nav.forward' })).toBeDisabled()
      const type = screen.getByRole('combobox', { name: 'workbench.applet.leave.type' })
      expect(type).toHaveClass('h-9', 'rounded-[8px]', 'bg-[#f5f5f5]', 'text-[13px]')
      const body = document.querySelector('[data-ui="workbench.applet.body"]') as HTMLElement
      const restingHeight = body.scrollHeight
      fireEvent.pointerDown(type)
      fireEvent.click(type)
      const options = await screen.findAllByRole('option')
      expect(body.contains(options[0]!)).toBe(false)
      expect(body.scrollHeight).toBe(restingHeight)
      expect(options.map((option) => option.textContent)).toEqual([
        'workbench.applet.leave.type.personal',
        'workbench.applet.leave.type.marriage',
        'workbench.applet.leave.type.sick',
        'workbench.applet.leave.type.annual',
        'workbench.applet.leave.type.bereavement',
        'workbench.applet.leave.type.maternity',
        'workbench.applet.leave.type.paternity',
        'workbench.applet.leave.type.childcare'
      ])
      const swatches = options.map((option) =>
        option.querySelector('[data-leave-swatch]')?.getAttribute('data-leave-swatch')
      )
      expect(new Set(swatches).size).toBe(8)
      expect(options.some((option) => /^\d/.test(option.textContent ?? ''))).toBe(false)
      fireEvent.click(screen.getByRole('option', { name: 'workbench.applet.leave.type.personal' }))
      expect(type.querySelector('[data-leave-swatch]')).toHaveAttribute('data-leave-swatch', '#3B78F0')
      expect(type.textContent).not.toMatch(/\d/)
      expect(document.querySelector('textarea[name="reason"]')).toHaveClass('h-40', 'rounded-[8px]', 'bg-[#f5f5f5]')
      expect(document.querySelector('input[type="date"]')).toBeNull()
      expect(document.querySelector('select:not([aria-hidden="true"])')).toBeNull()
      expect(document.querySelector('[data-ui="workbench.applet.leave.range"]')).toBeNull()
      const start = document.querySelector('[data-ui="workbench.applet.leave.start"]') as HTMLElement
      const end = document.querySelector('[data-ui="workbench.applet.leave.end"]') as HTMLElement
      expect(start.tagName).toBe('BUTTON')
      expect(start).toHaveClass('h-9', 'rounded-[8px]', 'bg-[#f5f5f5]', 'text-left')
      expect(end).toHaveClass('h-9', 'rounded-[8px]', 'bg-[#f5f5f5]', 'text-left')
      expect(start).toHaveTextContent('workbench.applet.leave.date')
      expect(end).toHaveTextContent('workbench.applet.leave.date')
      fireEvent.click(start)
      const range = document.querySelector('[data-ui="workbench.applet.leave.range"]') as HTMLElement
      expect(range.querySelector('table')).toBeTruthy()
      expect(body.contains(range)).toBe(false)
      expect(body.scrollHeight).toBe(restingHeight)
      const weekdays = Array.from(range.querySelectorAll('[scope="col"]')).map((cell) => cell.textContent)
      expect(weekdays).toEqual(['一', '二', '三', '四', '五', '六', '日'])
      expect(
        within(range).getByRole('button', { name: 'workbench.applet.leave.calendar.previous' })
      ).toBeInTheDocument()
      expect(within(range).getByRole('button', { name: 'workbench.applet.leave.calendar.next' })).toBeInTheDocument()
      const submit = screen.getByRole('button', { name: 'workbench.applet.submit' })
      expect(submit).toHaveAttribute('data-slot', 'button')
      expect(submit).toHaveAttribute('data-variant', 'emphasis')
      expect(submit).toHaveClass('min-h-9', 'rounded-md', 'text-[13px]')
      expect(submit).toBeDisabled()
      await chooseLeaveDay('2026-10-06')
      expect(start).toHaveTextContent('2026-10-06')
      expect(end).toHaveTextContent('2026-10-06')
      expect(document.querySelector('[data-ui="workbench.applet.leave.days"]')).toHaveTextContent(
        'workbench.applet.leave.days'
      )
      fireEvent.change(document.querySelector('textarea[name="reason"]')!, { target: { value: '家里有事' } })
      expect(submit).toBeEnabled()
      fireEvent.click(end)
      await chooseLeaveDay('2026-10-08')
      expect(start).toHaveTextContent('2026-10-06')
      expect(end).toHaveTextContent('2026-10-08')
      expect(document.querySelector('[data-ui="workbench.applet.leave.days"]')).toHaveTextContent(
        'workbench.applet.leave.days'
      )
      expect(submit).toBeEnabled()
      fireEvent.click(submit)
      await waitFor(() => {
        expect(screen.getByText('workbench.applet.submitted')).toBeInTheDocument()
      })
      const posted = fetchImpl.mock.calls.find((call) => call[1]?.method === 'POST')
      const postedBody = JSON.parse(String(posted?.[1]?.body)) as {
        idempotencyKey: string
        values: { title: string; detail: string }
      }
      expect(postedBody.values).toEqual({
        leave_type: '事假',
        start_date: '2026-10-06',
        end_date: '2026-10-08',
        reason: '家里有事'
      })
      expect(postedBody.idempotencyKey).toMatch(/^[0-9a-f-]{36}$/)
      vi.mocked(runAppletWindowAction).mockClear()
      fireEvent.mouseDown(screen.getByRole('button', { name: 'workbench.applet.nav.home' }), { button: 0 })
      expect(runAppletWindowAction).not.toHaveBeenCalled()
      expect(screen.getByRole('button', { name: 'workbench.applet.nav.back' })).toBeEnabled()
      fireEvent.click(screen.getByRole('button', { name: 'workbench.applet.nav.back' }))
      expect(screen.queryByText('workbench.applet.submitted')).not.toBeInTheDocument()
      expect(screen.getByRole('button', { name: 'workbench.applet.submit' })).toBeEnabled()
      expect(document.querySelector('textarea[name="reason"]')).toHaveValue('家里有事')
      fireEvent.click(screen.getByRole('button', { name: 'workbench.applet.nav.forward' }))
      expect(screen.getByText('workbench.applet.submitted')).toBeInTheDocument()

      const bar = document.querySelector('[data-ui="workbench.applet.window-bar"]')
      const controls = document.querySelector('[aria-label="workbench.applet.window_controls"]')
      expect(bar!.compareDocumentPosition(controls!) & Node.DOCUMENT_POSITION_FOLLOWING).toBeTruthy()
      const close = screen.getByRole('button', { name: 'common.close' })
      expect(close).toHaveClass('size-6', 'rounded-[6px]')
      expect(close.className).not.toContain('rounded-full')
      expect(close.className).not.toContain('#ff5f57')
      fireEvent.click(close)
      fireEvent.click(screen.getByRole('button', { name: 'workbench.applet.minimize' }))
      fireEvent.click(screen.getByRole('button', { name: 'workbench.applet.maximize' }))
      fireEvent.mouseDown(document.querySelector('[data-ui="workbench.applet.window-bar"]')!, { button: 0 })
      expect(runAppletWindowAction).toHaveBeenCalledWith('close')
      expect(runAppletWindowAction).toHaveBeenCalledWith('minimize')
      expect(runAppletWindowAction).toHaveBeenCalledWith('toggle-maximize')
      expect(runAppletWindowAction).toHaveBeenCalledWith('start-dragging')
    } finally {
      vi.unstubAllGlobals()
      removeAppletRuntime()
    }
  })

  it('stays closed when the applet package does not settle within one second', async () => {
    installAppletRuntime()
    vi.stubGlobal('fetch', appletFetch('ab'.repeat(32), { hang: true }))
    vi.useFakeTimers()
    try {
      render(<WorkbenchAppletWindow appletId="leave" />)
      expect(document.querySelector('[data-ui="workbench.applet.loading"]')).toBeTruthy()
      await act(async () => {
        await vi.advanceTimersByTimeAsync(1000)
      })
      expect(document.querySelector('[data-ui="workbench.applet.form"]')).toBeNull()
      expect(document.querySelector('[data-ui="workbench.applet.failed"]')).toBeTruthy()
    } finally {
      vi.useRealTimers()
      vi.unstubAllGlobals()
      removeAppletRuntime()
    }
  })

  it('stays closed when the member is signed out', async () => {
    const digest = await sha256Hex(new TextEncoder().encode(PACKAGE_BODY))
    installAppletRuntime()
    vi.stubGlobal('fetch', appletFetch(digest, { signedIn: false }))
    try {
      render(<WorkbenchAppletWindow appletId="leave" />)
      await waitFor(() => {
        expect(document.querySelector('[data-ui="workbench.applet.signed-out"]')).toBeTruthy()
      })
      expect(document.querySelector('[data-ui="workbench.applet.form"]')).toBeNull()
      expect(document.querySelector('[data-ui="workbench.applet.permission"]')).toBeNull()
    } finally {
      vi.unstubAllGlobals()
      removeAppletRuntime()
    }
  })

  it('asks for the member name and photo when leave opens, then shows them after allow', async () => {
    const digest = await sha256Hex(new TextEncoder().encode(PACKAGE_BODY))
    installAppletRuntime()
    const fetchImpl = appletFetch(digest)
    vi.stubGlobal('fetch', fetchImpl)
    try {
      const view = render(<WorkbenchAppletWindow appletId="leave" />)
      await waitFor(() => {
        expect(document.querySelector('[data-ui="workbench.applet.permission.subject"]')).toHaveTextContent('林*')
      })
      expect(fetchImpl.mock.calls.some((call) => String(call[0]).endsWith('/profile'))).toBe(true)
      expect(document.querySelector('[data-ui="workbench.applet.permission.avatar"]')).toHaveAttribute(
        'src',
        LEAVE_AVATAR
      )
      expect(screen.queryByText('林')).not.toBeInTheDocument()
      expect(document.body.textContent).not.toContain('13800138000')
      fireEvent.click(screen.getByRole('button', { name: 'workbench.applet.permission.allow' }))
      await waitFor(() => {
        expect(document.querySelector('[data-ui="workbench.applet.leave.avatar"]')).toHaveAttribute('src', LEAVE_AVATAR)
      })
      const leaveProfile = document.querySelector('[data-ui="workbench.applet.leave.profile"]')
      const leaveBody = document.querySelector('[data-ui="workbench.applet.body"]')
      expect(leaveProfile).not.toHaveClass('absolute')
      expect(leaveBody?.className).toContain('overflow-y-auto')
      expect(leaveBody?.contains(document.querySelector('[data-ui="workbench.applet.form"]'))).toBe(true)
      expect(leaveBody?.contains(leaveProfile)).toBe(false)
      expect(leaveProfile?.compareDocumentPosition(leaveBody!) & Node.DOCUMENT_POSITION_FOLLOWING).toBeTruthy()
      expect(screen.getByText('林')).toBeInTheDocument()
      expect(document.querySelector('[data-ui="workbench.applet.permission"]')).toBeNull()
      expect(document.body.textContent).not.toContain('13800138000')
      expect(localStorage.getItem(APPLET_PERMISSION_GRANT_KEY)).toContain('"leave":"user"')
      view.unmount()

      render(<WorkbenchAppletWindow appletId="leave" />)
      await waitFor(() => {
        expect(document.querySelector('[data-ui="workbench.applet.leave.avatar"]')).toHaveAttribute('src', LEAVE_AVATAR)
      })
      expect(document.querySelector('[data-ui="workbench.applet.permission"]')).toBeNull()
      expect(document.body.textContent).not.toContain('13800138000')
    } finally {
      vi.unstubAllGlobals()
      removeAppletRuntime()
    }
  })

  it('keeps leave closed when authorization is denied and asks again next open', async () => {
    const digest = await sha256Hex(new TextEncoder().encode(PACKAGE_BODY))
    installAppletRuntime()
    vi.stubGlobal('fetch', appletFetch(digest))
    try {
      const view = render(<WorkbenchAppletWindow appletId="leave" />)
      await waitFor(() => {
        expect(document.querySelector('[data-ui="workbench.applet.permission.subject"]')).toHaveTextContent('林*')
      })
      expect(document.body.textContent).not.toContain('13800138000')
      vi.mocked(runAppletWindowAction).mockClear()
      fireEvent.click(screen.getByRole('button', { name: 'workbench.applet.permission.deny' }))
      expect(runAppletWindowAction).toHaveBeenCalledWith('close')
      expect(document.querySelector('[data-ui="workbench.applet.permission"]')).toBeTruthy()
      expect(document.querySelector('[data-ui="workbench.applet.form"]')).toBeNull()
      expect(screen.queryByText('林')).not.toBeInTheDocument()
      expect(document.body.textContent).not.toContain('13800138000')
      expect(localStorage.getItem(APPLET_PROFILE_GRANT_KEY)).toBeNull()
      expect(localStorage.getItem(APPLET_PERMISSION_GRANT_KEY)).toBeNull()
      view.unmount()

      render(<WorkbenchAppletWindow appletId="leave" />)
      await waitFor(() => {
        expect(document.querySelector('[data-ui="workbench.applet.permission"]')).toBeTruthy()
      })
      expect(document.querySelector('[data-ui="workbench.applet.form"]')).toBeNull()
      expect(screen.queryByText('林')).not.toBeInTheDocument()
    } finally {
      vi.unstubAllGlobals()
      removeAppletRuntime()
    }
  })

  it('lists every requested permission under a title bar that stays outside the dialog', async () => {
    const assertDialog = () => {
      const dialog = document.querySelector('[data-ui="workbench.applet.permission"]')
      expect(dialog).toBeTruthy()
      const rows = dialog!.querySelectorAll('li')
      expect(rows).toHaveLength(3)
      expect(rows[0]).toHaveAttribute('data-ui', 'workbench.applet.permission.user')
      expect(rows[1]).toHaveAttribute('data-ui', 'workbench.applet.permission.model')
      expect(rows[2]).toHaveAttribute('data-ui', 'workbench.applet.permission.knowledge')
      expect(dialog!.textContent).toContain('workbench.applet.permission.user.limit')
      expect(dialog!.textContent).toContain('workbench.applet.permission.model.limit')
      expect(dialog!.textContent).toContain('workbench.applet.permission.knowledge.limit')
      expect(dialog!.textContent).not.toContain('13800138000')
      expect(dialog!.textContent).not.toContain('林知夏')
      const close = screen.getByRole('button', { name: 'common.close' })
      expect(dialog!.contains(close)).toBe(false)
      expect(document.querySelector('[data-ui="workbench.applet.window-bar"]')).toBeTruthy()
      expect(dialog!.parentElement?.className).toContain('absolute')
      expect(dialog!.parentElement?.className).toContain('overflow-y-auto')
      expect(dialog!.parentElement?.className).not.toContain('fixed')
    }
    installAppletRuntime('pdf', ['knowledge', 'model', 'user'])
    const fetchImpl = vi.fn()
    vi.stubGlobal('fetch', fetchImpl)
    try {
      for (const id of ['pdf'] as const) {
        const view = render(<WorkbenchAppletWindow appletId={id} />)
        assertDialog()
        view.unmount()
      }
      expect(fetchImpl.mock.calls.length).toBeGreaterThan(0)
      expect(fetchImpl.mock.calls.every((call) => String(call[0]).endsWith('/profile'))).toBe(true)
      expect(document.body.textContent).not.toContain('13800138000')
      vi.unstubAllGlobals()
      const body = PACKAGE_BODY.replace('"appId":"leave"', '"appId":"travel"')
      const digest = await sha256Hex(new TextEncoder().encode(body))
      installAppletRuntime('travel', ['knowledge', 'model', 'user'])
      vi.stubGlobal('fetch', appletFetch(digest, { body }))
      const view = render(<WorkbenchAppletWindow appletId="travel" />)
      await waitFor(() => {
        expect(document.querySelector('[data-ui="workbench.applet.permission.subject"]')).toHaveTextContent('林*')
      })
      expect(document.body.textContent).not.toContain('13800138000')
      assertDialog()
      view.unmount()
    } finally {
      vi.unstubAllGlobals()
      removeAppletRuntime()
    }
  })

  it('removes the boot title bar after the applet title bar is rendered', () => {
    const boot = document.createElement('header')
    boot.id = 'applet-boot-bar'
    document.body.appendChild(boot)
    try {
      render(<WorkbenchAppletWindow appletId="pdf" />)
      expect(document.getElementById('applet-boot-bar')).toBeNull()
      expect(screen.getByRole('button', { name: 'common.close' })).toBeInTheDocument()
      expect(
        document
          .querySelector('[data-ui="workbench.applet.permission"]')
          ?.contains(screen.getByRole('button', { name: 'common.close' }))
      ).toBe(false)
    } finally {
      document.getElementById('applet-boot-bar')?.remove()
    }
  })

  it('keeps an earlier user-information allow when the package still asks only for that', async () => {
    const digest = await sha256Hex(new TextEncoder().encode(PACKAGE_BODY))
    installAppletRuntime()
    localStorage.setItem(APPLET_PROFILE_GRANT_KEY, JSON.stringify(['leave']))
    vi.stubGlobal('fetch', appletFetch(digest))
    try {
      render(<WorkbenchAppletWindow appletId="leave" />)
      await waitFor(() => {
        expect(document.querySelector('[data-ui="workbench.applet.leave.avatar"]')).toBeTruthy()
      })
      expect(document.querySelector('[data-ui="workbench.applet.permission"]')).toBeNull()
    } finally {
      vi.unstubAllGlobals()
      removeAppletRuntime()
    }
  })

  it('asks again when the package permission set changes', async () => {
    const digest = await sha256Hex(new TextEncoder().encode(PACKAGE_BODY))
    installAppletRuntime('leave', ['model', 'user'])
    localStorage.setItem(APPLET_PERMISSION_GRANT_KEY, JSON.stringify({ leave: 'user' }))
    vi.stubGlobal('fetch', appletFetch(digest))
    try {
      render(<WorkbenchAppletWindow appletId="leave" />)
      await waitFor(() => {
        expect(document.querySelector('[data-ui="workbench.applet.permission.user"]')).toBeTruthy()
      })
      expect(document.querySelector('[data-ui="workbench.applet.permission.model"]')).toBeTruthy()
      expect(document.querySelector('[data-ui="workbench.applet.leave.profile"]')).toBeNull()
    } finally {
      vi.unstubAllGlobals()
      removeAppletRuntime()
    }
  })

  it('uses the name initial when the leave photo is not an image', async () => {
    const digest = await sha256Hex(new TextEncoder().encode(PACKAGE_BODY))
    installAppletRuntime()
    vi.stubGlobal(
      'fetch',
      vi.fn((url: string, init?: RequestInit) => {
        const target = String(url)
        if (target.endsWith('/session')) {
          return Promise.resolve(
            new Response(JSON.stringify({ signedIn: true, displayName: '林', phone: '13800138000' }), { status: 200 })
          )
        }
        if (target.endsWith('/profile')) {
          return Promise.resolve(
            new Response(
              JSON.stringify({ displayName: '林', avatar: 'https://evil.example/a.png', phone: '13800138000' }),
              { status: 200 }
            )
          )
        }
        if (target.endsWith('/package/release')) return Promise.resolve(new Response(PACKAGE_BODY, { status: 200 }))
        if (init?.method === 'POST') return Promise.resolve(new Response(JSON.stringify({ id: '1' }), { status: 200 }))
        return Promise.resolve(
          new Response(JSON.stringify({ release: { version: '1.1.0', digest }, trial: null }), { status: 200 })
        )
      })
    )
    try {
      render(<WorkbenchAppletWindow appletId="leave" />)
      await waitFor(() => {
        expect(document.querySelector('[data-ui="workbench.applet.permission"]')).toBeTruthy()
      })
      fireEvent.click(screen.getByRole('button', { name: 'workbench.applet.permission.allow' }))
      await waitFor(() => {
        expect(document.querySelector('[data-ui="workbench.applet.leave.profile"]')).toBeTruthy()
      })
      const avatar = document.querySelector('[data-ui="workbench.applet.leave.avatar"]')
      expect(avatar?.tagName).toBe('SPAN')
      expect(avatar).toHaveTextContent('林')
      expect(avatar).not.toHaveAttribute('src')
      expect(document.body.textContent).not.toContain('evil.example')
      expect(document.body.textContent).not.toContain('13800138000')
    } finally {
      vi.unstubAllGlobals()
      removeAppletRuntime()
    }
  })

  it('stays closed when the applet runtime is missing', async () => {
    const fetchImpl = vi.fn()
    vi.stubGlobal('fetch', fetchImpl)
    try {
      render(<WorkbenchAppletWindow appletId="leave" />)
      await waitFor(() => {
        expect(document.querySelector('[data-ui="workbench.applet.failed"]')).toBeTruthy()
      })
      expect(fetchImpl).not.toHaveBeenCalled()
      expect(document.querySelector('[data-ui="workbench.applet.form"]')).toBeNull()
    } finally {
      vi.unstubAllGlobals()
      removeAppletRuntime()
    }
  })

  it('keeps the pinned package when a newer release arrives', async () => {
    const cachedBody = PACKAGE_BODY.replace('1.1.0', '1.0.0')
    const cachedDigest = await sha256Hex(new TextEncoder().encode(cachedBody))
    const nextDigest = await sha256Hex(new TextEncoder().encode(PACKAGE_BODY))
    localStorage.setItem(
      CACHE_KEY,
      JSON.stringify({ leave: { release: { version: '1.0.0', digest: cachedDigest, body: cachedBody } } })
    )
    installAppletRuntime()
    vi.stubGlobal(
      'fetch',
      vi.fn((url: string) => {
        const target = String(url)
        if (target.endsWith('/session')) {
          return Promise.resolve(new Response(JSON.stringify({ signedIn: true, displayName: '林' }), { status: 200 }))
        }
        if (target.endsWith('/package/release')) return Promise.resolve(new Response(PACKAGE_BODY, { status: 200 }))
        return Promise.resolve(
          new Response(JSON.stringify({ release: { version: '1.1.0', digest: nextDigest }, trial: null }), {
            status: 200
          })
        )
      })
    )
    try {
      render(<WorkbenchAppletWindow appletId="leave" />)
      await waitFor(() => {
        expect(document.querySelector('[data-ui="workbench.applet.permission"]')).toBeTruthy()
      })
      expect(document.querySelector('[data-ui="workbench.applet.form"]')).toBeNull()
      expect(document.querySelector('[data-ui="workbench.applet.window"]')).toHaveAttribute(
        'data-applet-version',
        '1.0.0'
      )
      await waitFor(() => {
        expect(document.querySelector('[data-ui="workbench.applet.update"]')).toHaveTextContent(
          'workbench.applet.update_ready'
        )
      })
      expect(document.querySelector('[data-ui="workbench.applet.window"]')).toHaveAttribute(
        'data-applet-version',
        '1.0.0'
      )
    } finally {
      vi.unstubAllGlobals()
      removeAppletRuntime()
    }
  })

  it('keeps pinned sessions and caps the recent list at 50', () => {
    const now = Date.now()
    const sessions = [
      session('pinned', new Date(now - 80 * 86_400_000).toISOString(), true, 'Pinned'),
      ...Array.from({ length: 51 }, (_, index) =>
        session(`recent-${index}`, new Date(now - index * 60_000).toISOString(), false, `Recent ${index}`)
      )
    ]
    renderHome({ sessions })

    expect(document.querySelector('[data-session-place="recent"][data-session-id="pinned"]')).toHaveAttribute(
      'data-pinned',
      'true'
    )
    expect(document.querySelector('[data-ui="workbench.tasks.pinned"]')).toBeTruthy()
    expect(document.querySelector('[data-session-place="recent"][data-session-id="recent-0"]')).toBeTruthy()
    expect(document.querySelector('[data-session-place="recent"][data-session-id="recent-8"]')).toBeTruthy()
    expect(document.querySelector('[data-session-place="recent"][data-session-id="recent-49"]')).toBeTruthy()
    expect(document.querySelector('[data-session-place="recent"][data-session-id="recent-50"]')).toBeNull()
    expect(document.querySelectorAll('[data-session-place="recent"]')).toHaveLength(51)
    expect(document.querySelectorAll('[data-session-place="project"]')).toHaveLength(0)
    const sessionAvatar = document.querySelector(
      '[data-session-id="recent-0"] [data-ui="workbench.session.avatar"] [data-a3s-bot-avatar="true"]'
    )
    expect(sessionAvatar).toHaveAttribute('data-avatar-variant', 'default')
    expect(sessionAvatar).toHaveAttribute('data-bot-activity', 'completed')
    expect(sessionAvatar).toHaveAttribute('data-animation-enabled', 'false')

    fireEvent.click(document.querySelector('[data-ui="workbench.tasks.more"]')!)

    expect(document.querySelector('[data-ui="workbench.tasks.history"] [data-session-id="recent-49"]')).toBeTruthy()
    expect(document.querySelector('[data-ui="workbench.tasks.history"] [data-session-id="recent-50"]')).toBeNull()
    expect(document.querySelectorAll('[data-session-place="recent"]')).toHaveLength(51)
    expect(document.querySelector('[data-ui="workbench.tasks.more"]')).toBeTruthy()
  })

  it('shows a document-expert face for each task state', () => {
    const now = new Date().toISOString()
    renderHome({
      homeActive: false,
      activeSessionId: 'open-task',
      sessions: [
        { ...session('running', now, false, 'Running'), avatar: 'working' },
        { ...session('waiting', now, false, 'Waiting'), avatar: 'paused' },
        { ...session('broken', now, false, 'Broken'), avatar: 'failed' },
        { ...session('open-task', now, false, 'Open'), avatar: 'failed' },
        session('settled', now, false, 'Settled')
      ]
    })

    const face = (id: string) =>
      document.querySelector(
        `[data-session-id="${id}"] [data-ui="workbench.session.avatar"] [data-a3s-bot-avatar="true"]`
      )

    expect(face('running')).toHaveAttribute('data-bot-activity', 'working')
    expect(face('running')).toHaveAttribute('data-animation-enabled', 'true')
    expect(face('running')).toHaveAttribute('data-avatar-variant', 'default')
    expect(face('running')?.querySelector('.a3s-bot-avatar__glasses')).toBeNull()
    expect(face('waiting')).toHaveAttribute('data-bot-activity', 'paused')
    expect(face('waiting')).toHaveAttribute('data-animation-enabled', 'false')
    expect(face('broken')).toHaveAttribute('data-bot-activity', 'failed')
    expect(face('broken')).toHaveAttribute('data-animation-enabled', 'false')
    expect(face('open-task')).toHaveAttribute('data-bot-activity', 'failed')
    expect(face('open-task')).toHaveAttribute('data-animation-enabled', 'true')
    expect(face('settled')).toHaveAttribute('data-bot-activity', 'completed')
    expect(face('settled')).toHaveAttribute('data-animation-enabled', 'false')
  })

  it('keeps project tasks out of the inbox and lists a project before it has tasks', () => {
    const now = Date.now()
    renderHome({
      projects: [{ id: 'empty-project', name: '测试' }],
      sessions: [
        {
          ...session('project-task', new Date(now).toISOString(), false, 'Alpha plan'),
          projectId: 'a3s',
          projectName: 'A3S'
        },
        session('fresh', new Date(now - 60_000).toISOString(), false, 'Alpha note'),
        session('stale', new Date(now - 10 * 86_400_000).toISOString(), false, 'Old note')
      ]
    })

    expect(document.querySelector('[data-session-place="project"][data-session-id="project-task"]')).toBeTruthy()
    expect(document.querySelector('[data-session-place="recent"][data-session-id="project-task"]')).toBeNull()
    expect(document.querySelector('[data-project-id="empty-project"]')).toHaveTextContent('测试')
    expect(document.querySelector('[data-project-id="empty-project"]')).not.toHaveTextContent(
      'workbench.projects.tasks_empty'
    )
    expect(document.querySelector('[data-project-id="empty-project"] [data-project-action="toggle"]')).toHaveAttribute(
      'aria-expanded',
      'false'
    )
    expect(document.querySelector('[data-task-date-group]')).toBeTruthy()
    expect(document.querySelector('[data-task-date-group] h3')).toHaveTextContent('workbench.day.today')
    expect(document.querySelector('[data-session-id="fresh"]')).toBeTruthy()
    expect(document.querySelector('[data-session-id="stale"]')).toBeTruthy()
    expect(document.querySelector('[data-ui="workbench.tasks.more"]')).toBeTruthy()
  })

  it('marks every literal occurrence and leaves an empty query unmarked', () => {
    expect(matchesQuery('Alpha plan', '  PLAN ')).toBe(true)
    expect(matchesQuery('Alpha plan', 'missing')).toBe(false)
    expect(highlightQuery('Alpha plan', '   ')).toBe('Alpha plan')
    const { container } = render(<>{highlightQuery('Plan plan', 'plan')}</>)
    const marks = container.querySelectorAll('mark')
    expect(marks).toHaveLength(2)
    expect(marks[0]).toHaveTextContent('Plan')
    expect(marks[1]).toHaveTextContent('plan')
    expect(marks[0]).toHaveClass('bg-[#fff3b0]')
    const dotted = render(<>{highlightQuery('a.b.c', '.')}</>).container
    expect(dotted.querySelectorAll('mark')).toHaveLength(2)
    expect(dotted).toHaveTextContent('a.b.c')
  })

  it('filters the project list and the task list from the top field', async () => {
    const props = renderHome({
      projects: [
        ...Array.from({ length: 8 }, (_, index) => ({ id: `p-${index}`, name: `Folder ${index}` })),
        { id: 'plan', name: 'Alpha plan' },
        { id: 'note', name: 'Beta note' },
        { id: 'dots', name: 'a.b.c' }
      ],
      sessions: [
        session('a', '2026-10-01T00:00:00.000Z', false, 'Alpha plan'),
        session('b', '2026-09-01T00:00:00.000Z', false, 'Beta note')
      ]
    })

    fireEvent.click(document.querySelector('[data-ui="workbench.projects.more"]')!)
    const catalog = document.querySelector('[data-ui="workbench.projects.catalog"]')!
    const catalogScrim = catalog.parentElement
    expect(document.querySelector('[data-ui="workbench.home"]')).not.toContainElement(catalog)
    expect(catalogScrim?.parentElement).toBe(document.body)
    expect(catalogScrim).toHaveClass('fixed', 'inset-0', 'z-[220]')
    const projectSearch = screen.getByRole('searchbox', { name: 'workbench.projects.catalog_search' })
    expect(projectSearch).toHaveFocus()
    expect(catalog.querySelectorAll('[data-project-catalog-id]')).toHaveLength(11)

    fireEvent.change(projectSearch, { target: { value: '.' } })
    expect(catalog.querySelectorAll('[data-project-catalog-id]')).toHaveLength(1)
    expect(catalog.querySelector('[data-project-catalog-id="dots"]')?.querySelectorAll('mark')).toHaveLength(2)
    expect(document.querySelector('[data-ui="workbench.projects"] [data-project-id="p-0"]')).toBeTruthy()

    fireEvent.change(projectSearch, { target: { value: 'missing' } })
    expect(catalog.querySelector('[data-ui="workbench.projects.catalog.empty"]')).toHaveTextContent(
      'workbench.projects.catalog_empty'
    )
    fireEvent.keyDown(projectSearch, { key: 'Escape' })
    expect(projectSearch).toHaveValue('')
    expect(catalog.querySelector('[data-project-catalog-id="plan"]')).toBeTruthy()

    fireEvent.change(projectSearch, { target: { value: 'plan' } })
    fireEvent.keyDown(projectSearch, { key: 'Enter' })
    expect(document.querySelector('[data-project-id="plan"] [data-project-action="toggle"]')).toHaveAttribute(
      'aria-expanded',
      'true'
    )
    await waitFor(() =>
      expect(document.querySelector('[data-ui="workbench.projects.catalog"]')).not.toBeInTheDocument()
    )

    fireEvent.click(document.querySelector('[data-ui="workbench.tasks.more"]')!)
    const history = document.querySelector('[data-ui="workbench.tasks.history"]')!
    const historyScrim = history.parentElement
    expect(document.querySelector('[data-ui="workbench.home"]')).not.toContainElement(history)
    expect(historyScrim?.parentElement).toBe(document.body)
    expect(historyScrim).toHaveClass('fixed', 'inset-0', 'z-[220]')
    const taskSearch = screen.getByRole('searchbox', { name: 'workbench.tasks.history_search' })
    expect(taskSearch).toHaveFocus()
    fireEvent.change(taskSearch, { target: { value: 'plan' } })
    expect(history.querySelector('[data-session-id="a"] mark')).toHaveTextContent('plan')
    expect(history.querySelector('[data-session-id="a"] mark')).toHaveClass('bg-[#fff3b0]')
    expect(history.querySelector('[data-session-id="b"]')).toBeNull()
    expect(document.querySelector('[data-session-place="recent"][data-session-id="b"]')).toBeTruthy()
    fireEvent.keyDown(taskSearch, { key: 'Enter' })
    expect(props.onOpenSession).toHaveBeenCalledWith('a')
    await waitFor(() => expect(document.querySelector('[data-ui="workbench.tasks.history"]')).not.toBeInTheDocument())
  })

  it('shows a recent project window and does not expand every empty project', () => {
    renderHome({
      projects: Array.from({ length: 10 }, (_, index) => ({ id: `p-${index}`, name: `Folder ${index}` }))
    })

    const rail = document.querySelector('[data-ui="workbench.projects"]')!
    expect(rail.querySelectorAll('[data-project-id]')).toHaveLength(8)
    expect(rail.querySelector('[data-project-id="p-8"]')).toBeNull()
    expect(rail.textContent).not.toContain('workbench.projects.tasks_empty')
    expect(rail.querySelectorAll('[aria-expanded="true"]')).toHaveLength(0)

    fireEvent.click(rail.querySelector('[data-ui="workbench.projects.more"]')!)
    const catalog = document.querySelector('[data-ui="workbench.projects.catalog"]')!
    fireEvent.click(catalog.querySelector('[data-project-catalog-id="p-9"]')!)

    expect(rail.querySelector('[data-project-id="p-9"] [data-project-action="toggle"]')).toHaveAttribute(
      'aria-expanded',
      'true'
    )
    expect(rail.querySelectorAll('[aria-expanded="true"]')).toHaveLength(1)
    expect(rail.querySelector('[data-project-id="p-9"]')).not.toHaveTextContent('workbench.projects.tasks_empty')
  })

  it('parks the work sidebar off a narrow catalog so the page keeps its width', () => {
    vi.stubGlobal('matchMedia', (query: string) => ({
      matches: query === '(max-width: 760px)',
      media: query,
      addEventListener: () => undefined,
      removeEventListener: () => undefined
    }))
    try {
      renderHome({ catalog: 'skill' })
      const aside = document.querySelector('#workbench-sidebar') as HTMLElement
      const main = document.querySelector('[data-ui="workbench.home"] main') as HTMLElement
      expect(aside.style.left).toBe('-260px')
      expect(main.style.marginLeft).toBe('0px')
      expect(aside).toHaveAttribute('aria-hidden', 'true')
    } finally {
      vi.unstubAllGlobals()
    }
  })

  it('parks the work sidebar when the page marks it collapsed', () => {
    renderHome({ sidebarCollapsed: true })
    const aside = document.querySelector('#workbench-sidebar') as HTMLElement
    expect(aside.style.left).toBe('-260px')
    expect(aside).toHaveAttribute('aria-hidden', 'true')
  })

  it('keeps the work sidebar at a fixed width', () => {
    renderHome()
    expect(document.querySelector('[data-ui="workbench.home"] aside')).toHaveStyle({ width: '260px' })
    expect(screen.queryByRole('separator')).not.toBeInTheDocument()
  })

  it('collapses the work sidebar from the seam button and expands it again', () => {
    window.localStorage.removeItem('workbench.sidebarCollapsed')
    try {
      renderHome()
      const collapse = screen.getByRole('button', { name: 'workbench.sidebar.collapse' })
      expect(collapse).toHaveAttribute('aria-pressed', 'true')
      expect(collapse).toHaveStyle({ left: '260px' })

      fireEvent.click(collapse)

      const expand = screen.getByRole('button', { name: 'workbench.sidebar.expand' })
      const aside = document.querySelector('[data-ui="workbench.home"] aside') as HTMLElement
      expect(expand).toHaveAttribute('aria-pressed', 'false')
      expect(expand).toHaveStyle({ left: '13px' })
      expect(expand.querySelector('svg')).toHaveClass('rotate-180')
      expect(aside.style.width).toBe('260px')
      expect(aside.className).toContain('workbench-sidebar-slide')
      expect(aside.style.transition).toBe('none')
      expect(aside.style.left).toBe('-260px')
      expect(aside.style.transform).toBe('')
      expect(aside).toHaveAttribute('aria-hidden', 'true')
      expect(window.localStorage.getItem('workbench.sidebarCollapsed')).toBe('1')

      fireEvent.click(expand)

      expect(screen.getByRole('button', { name: 'workbench.sidebar.collapse' })).toHaveStyle({ left: '260px' })
      expect(aside.style.width).toBe('260px')
      expect(aside.style.left).toBe('0px')
      expect(window.localStorage.getItem('workbench.sidebarCollapsed')).toBe('0')
    } finally {
      window.localStorage.removeItem('workbench.sidebarCollapsed')
    }
  })

  it('opens a search dialog and opens a matching task without filtering the sidebar', async () => {
    const props = renderHome({
      sessions: [
        session('a', '2026-10-01T00:00:00.000Z', false, 'Alpha plan'),
        session('b', '2026-09-01T00:00:00.000Z', false, 'Beta note')
      ]
    })

    expect(document.querySelector('[data-session-place="recent"][data-session-id="b"]')).toBeTruthy()
    fireEvent.click(screen.getByRole('button', { name: 'workbench.search' }))
    const dialog = screen.getByRole('dialog', { name: 'workbench.search' })
    const scrim = dialog.parentElement
    expect(document.querySelector('[data-ui="workbench.home"]')).not.toContainElement(dialog)
    expect(scrim?.parentElement).toBe(document.body)
    expect(scrim).toHaveClass('fixed', 'inset-0', 'z-[220]')
    expect(dialog).toHaveAttribute('aria-modal', 'true')
    expect(screen.getByRole('searchbox', { name: 'workbench.search' }).parentElement).toHaveClass(
      'h-11',
      'bg-transparent'
    )
    expect(dialog.querySelector('[data-session-id="a"]')).toBeNull()
    expect(dialog.querySelector('[data-ui="workbench.search.hint"]')).toHaveTextContent('workbench.search.hint')

    fireEvent.change(screen.getByRole('searchbox', { name: 'workbench.search' }), { target: { value: 'alpha' } })
    expect(dialog.querySelector('[data-session-id="a"]')).toBeTruthy()
    expect(dialog.querySelector('[data-session-id="a"] mark')).toHaveTextContent('Alpha')
    expect(dialog.querySelector('[data-session-id="a"] mark')).toHaveClass('bg-[#fff3b0]')
    expect(dialog.querySelector('[data-session-id="b"]')).toBeNull()
    expect(dialog.querySelector('[data-session-id="a"]')).toHaveAttribute('aria-selected', 'true')
    expect(document.querySelector('[data-session-place="recent"][data-session-id="b"]')).toBeTruthy()

    fireEvent.change(screen.getByRole('searchbox', { name: 'workbench.search' }), { target: { value: 'missing' } })
    expect(dialog.querySelector('[data-ui="workbench.search.empty"]')).toHaveTextContent('workbench.search.empty')

    fireEvent.change(screen.getByRole('searchbox', { name: 'workbench.search' }), { target: { value: 'alpha' } })
    fireEvent.click(dialog.querySelector('[data-session-id="a"]')!)
    expect(props.onOpenSession).toHaveBeenCalledWith('a')
    await waitFor(() => expect(screen.queryByRole('dialog', { name: 'workbench.search' })).not.toBeInTheDocument())
    expect(screen.queryByRole('button', { name: 'settings.scheduledTasks.title' })).not.toBeInTheDocument()
    expect(screen.queryByRole('button', { name: 'settings.nav.archivedTasks' })).not.toBeInTheDocument()
  })

  it('keeps task search above the page and opens the keyboard selection', async () => {
    const props = renderHome({
      sessions: [
        session('a', '2026-10-01T00:00:00.000Z', false, 'Alpha plan'),
        session('b', '2026-09-01T00:00:00.000Z', false, 'Beta note')
      ]
    })

    fireEvent.click(screen.getByRole('button', { name: 'workbench.search' }))
    const searchbox = screen.getByRole('searchbox', { name: 'workbench.search' })
    fireEvent.keyDown(searchbox, { key: 'Escape' })
    await waitFor(() => expect(screen.queryByRole('dialog', { name: 'workbench.search' })).not.toBeInTheDocument())

    fireEvent.click(screen.getByRole('button', { name: 'workbench.search' }))
    const dialog = screen.getByRole('dialog', { name: 'workbench.search' })
    fireEvent.mouseDown(dialog.parentElement!)
    await waitFor(() => expect(screen.queryByRole('dialog', { name: 'workbench.search' })).not.toBeInTheDocument())

    fireEvent.click(screen.getByRole('button', { name: 'workbench.search' }))
    fireEvent.change(screen.getByRole('searchbox', { name: 'workbench.search' }), { target: { value: 'n' } })
    const open = screen.getByRole('dialog', { name: 'workbench.search' })
    expect(open.querySelector('[data-session-id="a"]')).toHaveAttribute('aria-selected', 'true')
    fireEvent.keyDown(screen.getByRole('searchbox', { name: 'workbench.search' }), { key: 'ArrowDown' })
    expect(open.querySelector('[data-session-id="b"]')).toHaveAttribute('aria-selected', 'true')
    fireEvent.keyDown(screen.getByRole('searchbox', { name: 'workbench.search' }), { key: 'Enter' })
    expect(props.onOpenSession).toHaveBeenCalledWith('b')
    await waitFor(() => expect(screen.queryByRole('dialog', { name: 'workbench.search' })).not.toBeInTheDocument())
  })

  it('creates a project from the name alone', async () => {
    const onCreateProject = vi.fn().mockResolvedValue(undefined)
    renderHome({ onCreateProject })

    fireEvent.click(document.querySelector('[data-project-action="create"]')!)
    const dialog = document.querySelector('[data-ui="workbench.projects.create"]')!
    expect(dialog).toHaveClass('w-[320px]', 'z-[220]')
    expect(dialog).toHaveTextContent('workbench.projects.create')
    expect(dialog).toHaveTextContent('workbench.projects.create_description')
    expect(screen.getByRole('heading', { name: 'workbench.projects.create' })).toHaveClass('text-[15px]', 'font-medium')
    const close = screen.getByRole('button', { name: 'common.close' })
    expect(close.querySelector('svg')).toBeInTheDocument()
    expect(close).toHaveAttribute('data-variant', 'ghost')
    expect(screen.queryByRole('button', { name: 'workbench.projects.create_folder' })).not.toBeInTheDocument()
    expect(screen.getByRole('button', { name: 'common.cancel' })).toBeEnabled()
    const confirm = screen.getByRole('button', { name: 'workbench.projects.create_confirm' })
    expect(confirm).toBeDisabled()
    expect(confirm).toHaveAttribute('data-variant', 'emphasis')
    expect(screen.getByRole('button', { name: 'common.cancel' })).toHaveAttribute('data-variant', 'outline')
    const input = screen.getByRole('textbox', { name: 'workbench.projects.create_name' })
    expect(input).toHaveClass('h-9', 'bg-[#f5f5f5]', 'text-[13px]')
    expect(input.className).not.toContain('bg-white')
    expect(input.className).not.toContain('h-10')
    expect(input).toHaveFocus()
    fireEvent.change(input, { target: { value: '  测试  ' } })
    expect(confirm).toBeEnabled()
    fireEvent.click(confirm)

    await waitFor(() => expect(onCreateProject).toHaveBeenCalledWith({ name: '测试' }))
    await waitFor(() => expect(document.querySelector('[data-ui="workbench.projects.create"]')).not.toBeInTheDocument())
  })

  it('validates the project name inline before creating', () => {
    const onCreateProject = vi.fn()
    renderHome({ onCreateProject, projects: [{ id: 'proj', name: 'A3S' }] })

    fireEvent.click(document.querySelector('[data-project-action="create"]')!)
    const input = screen.getByRole('textbox', { name: 'workbench.projects.create_name' })
    const confirm = screen.getByRole('button', { name: 'workbench.projects.create_confirm' })

    fireEvent.change(input, { target: { value: 'a/b' } })
    expect(input).toHaveAttribute('aria-invalid', 'true')
    expect(screen.getByRole('alert')).toHaveTextContent('workbench.projects.create_invalid_chars')
    expect(confirm).toBeDisabled()

    fireEvent.change(input, { target: { value: 'a3s' } })
    expect(screen.getByRole('alert')).toHaveTextContent('workbench.projects.create_duplicate')
    expect(confirm).toBeDisabled()

    fireEvent.change(input, { target: { value: 'Kense' } })
    expect(screen.queryByRole('alert')).not.toBeInTheDocument()
    expect(confirm).toBeEnabled()
  })

  it('keeps the draft open when creating the project fails', async () => {
    const onCreateProject = vi.fn().mockRejectedValue(new Error('denied'))
    renderHome({ onCreateProject })

    fireEvent.click(document.querySelector('[data-project-action="create"]')!)
    fireEvent.change(screen.getByRole('textbox', { name: 'workbench.projects.create_name' }), {
      target: { value: '测试' }
    })
    fireEvent.click(screen.getByRole('button', { name: 'workbench.projects.create_confirm' }))

    await waitFor(() => expect(onCreateProject).toHaveBeenCalled())
    await waitFor(() => expect(screen.getByRole('button', { name: 'workbench.projects.create_confirm' })).toBeEnabled())
    expect(screen.getByRole('textbox', { name: 'workbench.projects.create_name' })).toHaveValue('测试')
  })

  it('cancels the create dialog and discards the draft', () => {
    renderHome()
    fireEvent.click(document.querySelector('[data-project-action="create"]')!)
    fireEvent.change(screen.getByRole('textbox', { name: 'workbench.projects.create_name' }), {
      target: { value: '草稿' }
    })
    fireEvent.click(screen.getByRole('button', { name: 'common.cancel' }))
    expect(document.querySelector('[data-ui="workbench.projects.create"]')).not.toBeInTheDocument()

    fireEvent.click(document.querySelector('[data-project-action="create"]')!)
    expect(screen.getByRole('textbox', { name: 'workbench.projects.create_name' })).toHaveValue('')
  })

  it('creates a task in a project and drops a task onto a project', () => {
    const onCreateTaskInProject = vi.fn()
    const onMoveSessionToProject = vi.fn()
    const onOpenSession = vi.fn()
    renderHome({
      projects: [{ id: 'proj', name: 'A3S' }],
      sessions: [session('fresh', '2026-10-02T00:00:00.000Z', false, 'Alpha note')],
      onCreateTaskInProject,
      onMoveSessionToProject,
      onOpenSession
    })

    fireEvent.click(screen.getByRole('button', { name: 'workbench.projects.new_task' }))
    expect(onCreateTaskInProject).toHaveBeenCalledWith('proj')

    const row = document.querySelector('[data-session-id="fresh"]')!
    const project = document.querySelector('[data-project-id="proj"]')!
    const restorePoint = pointAt(project)
    const time = row.querySelector('time')!
    sendPointer(time, 'pointerdown', 0, 0)
    sendPointer(time, 'pointermove', 0, 2)
    sendPointer(time, 'pointerup', 0, 2)
    expect(onMoveSessionToProject).not.toHaveBeenCalled()

    sendPointer(row, 'pointerdown', 0, 0)
    sendPointer(row, 'pointermove', 0, 24)
    expect(project).toHaveAttribute('data-drop-active', 'true')
    expect(document.querySelector('[data-ui="workbench.session.drag"]')).toHaveTextContent('Alpha note')
    sendPointer(row, 'pointerup', 0, 24)
    expect(onMoveSessionToProject).toHaveBeenCalledWith('fresh', 'proj')
    expect(project).not.toHaveAttribute('data-drop-active')
    fireEvent.click(row.querySelector('button')!)
    expect(onOpenSession).not.toHaveBeenCalled()

    Document.prototype.elementFromPoint = () => document.body
    sendPointer(row, 'pointerdown', 0, 0)
    sendPointer(row, 'pointermove', 30, 0)
    sendPointer(row, 'pointerup', 30, 0)
    expect(onMoveSessionToProject).toHaveBeenCalledTimes(1)
    restorePoint()
  })

  it('drops a project task back onto the task list and ignores an inbox task', () => {
    const onMoveSessionToInbox = vi.fn()
    renderHome({
      projects: [{ id: 'proj', name: 'A3S' }],
      sessions: [
        { ...session('fresh', '2026-10-02T00:00:00.000Z', false, 'Alpha note'), projectId: 'proj', projectName: 'A3S' },
        session('loose', '2026-10-02T00:00:00.000Z', false, 'Loose note')
      ],
      onMoveSessionToInbox
    })

    const tasks = document.querySelector('[data-ui="workbench.tasks"]')!
    const restorePoint = pointAt(tasks)
    const projectRow = document.querySelector('[data-session-place="project"][data-session-id="fresh"]')!
    sendPointer(projectRow, 'pointerdown', 0, 0)
    sendPointer(projectRow, 'pointermove', 0, 24)
    expect(tasks).toHaveAttribute('data-drop-inbox', 'true')
    sendPointer(projectRow, 'pointerup', 0, 24)
    expect(onMoveSessionToInbox).toHaveBeenCalledWith('fresh')
    expect(tasks).not.toHaveAttribute('data-drop-inbox')

    const loose = document.querySelector('[data-session-place="recent"][data-session-id="loose"]')!
    sendPointer(loose, 'pointerdown', 0, 0)
    sendPointer(loose, 'pointermove', 0, 24)
    expect(tasks).not.toHaveAttribute('data-drop-inbox')
    sendPointer(loose, 'pointerup', 0, 24)
    expect(onMoveSessionToInbox).toHaveBeenCalledTimes(1)

    const rename = projectRow.querySelector('[data-session-action="rename"]')!
    sendPointer(rename, 'pointerdown', 0, 0)
    sendPointer(rename, 'pointermove', 0, 24)
    sendPointer(rename, 'pointerup', 0, 24)
    expect(onMoveSessionToInbox).toHaveBeenCalledTimes(1)
    restorePoint()
  })
})
