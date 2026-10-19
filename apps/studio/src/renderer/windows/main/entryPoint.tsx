import '@renderer/assets/styles/index.css'
import '@renderer/assets/styles/tailwind.css'
import { createRoot } from 'react-dom/client'

import { loggerService } from '@logger'
import { adoptEditorCenter } from '@renderer/data/memberOrgs'
import type { WorkbenchWindowAppletId } from '@renderer/pages/agents/WorkbenchApplets'
import { CLOUD_WORKSPACE_SCHEME, isCloudWorkspace } from '@renderer/pages/documents/cloudWorkspace'
import { prepareWindow } from '@renderer/windows/prepareWindow'

import MainApp from './MainApp'

const logger = loggerService.withContext('MainEntry')

// prepareWindow is best-effort. A hung locale request must not leave the
// white boot layer up; the tree still mounts and MainWindowRuntime clears it.
const BOOT_LIMIT_MS = 4000
const FILE_EDITOR_BOOT_MS = 800

function fileEditorSearch(): { root: string; path: string; readOnly: boolean } | null {
  const params = new URLSearchParams(window.location.search)
  if (params.get('file-editor') !== '1') return null
  const root = params.get('root')?.trim() ?? ''
  const path = params.get('path')?.trim() ?? ''
  if (!root || !path) return null
  return { root, path, readOnly: params.get('readonly') === '1' }
}

function isFileEditorWindow(): boolean {
  return new URLSearchParams(window.location.search).get('file-editor') === '1'
}

// Bubble phase. The embedded browser reloads itself in capture and marks the event handled.
window.addEventListener('keydown', (event) => {
  if (event.defaultPrevented || event.isComposing || event.altKey || event.shiftKey) return
  if (event.key !== 'r' && event.key !== 'R') return
  if (!event.metaKey && !event.ctrlKey) return
  if (isFileEditorWindow()) return
  event.preventDefault()
  window.location.reload()
})

const earlyFileEditor = fileEditorSearch()
const editorChunk = earlyFileEditor ? import('@renderer/pages/documents/FileEditorWindow') : null
if (earlyFileEditor && /\.(?:docx|docm|dotx|xlsx|xlsm|pptx|pptm|pdf|odt|ods|odp)$/i.test(earlyFileEditor.path)) {
  void import('@renderer/pages/documents/OfficeDocumentSurface').then((module) => module.prefetchOfficeEditors())
}

await Promise.race([
  prepareWindow({ preference: earlyFileEditor ? [] : 'all' }).catch((error: unknown) => {
    logger.error('window prepare failed', error as Error)
  }),
  new Promise<void>((resolve) => {
    setTimeout(resolve, earlyFileEditor ? FILE_EDITOR_BOOT_MS : BOOT_LIMIT_MS)
  })
])

const WORKBENCH_APPLET =
  /^(approval|leave|travel|expense|meeting|seal|notice|pdf|inbox|plan|probation|clock|correction|ai-resource)$/

function readWorkbenchApplet(): {
  id: WorkbenchWindowAppletId
  channel: 'release' | 'trial' | 'invalid'
  tool: string | null
} | null {
  const params = new URLSearchParams(window.location.search)
  const id = params.get('workbench-applet')
  if (!id || !WORKBENCH_APPLET.test(id)) return null
  const raw = params.get('channel')
  const channel = raw == null || raw === '' || raw === 'release' ? 'release' : raw === 'trial' ? 'trial' : 'invalid'
  return { id: id as WorkbenchWindowAppletId, channel, tool: params.get('tool') }
}

function readFileEditor(): { root: string; path: string; readOnly: boolean } | null {
  return fileEditorSearch()
}

const NOTICE_SLUG = /^(terms|privacy|copyright|support)$/

function readNotice(): 'terms' | 'privacy' | 'copyright' | 'support' | null {
  const slug = new URLSearchParams(window.location.search).get('notice')
  if (!slug || !NOTICE_SLUG.test(slug)) return null
  return slug as 'terms' | 'privacy' | 'copyright' | 'support'
}

function readMailCompose(): boolean {
  return new URLSearchParams(window.location.search).get('mail-compose') === '1'
}

function readAgentWiki(): { agentId: string; title: string } | null {
  const params = new URLSearchParams(window.location.search)
  if (params.get('agent-wiki') !== '1') return null
  const agentId = params.get('agent')?.trim() ?? ''
  if (!/^[A-Za-z0-9][A-Za-z0-9_-]{0,63}$/.test(agentId)) return null
  return { agentId, title: params.get('title')?.trim() ?? '' }
}

const AIX_IDENTIFIER = /^[a-z][a-z0-9-]{0,127}$/

function readAixApplet(): { slug: string; view: string; title: string } | null {
  const params = new URLSearchParams(window.location.search)
  if (params.get('aix-applet') !== '1') return null
  const slug = params.get('slug') ?? ''
  const view = params.get('view') ?? ''
  if (slug.length > 64 || !AIX_IDENTIFIER.test(slug) || !AIX_IDENTIFIER.test(view)) return null
  return { slug, view, title: params.get('title')?.trim() ?? '' }
}

const aixApplet = readAixApplet()
const agentWiki = aixApplet ? null : readAgentWiki()
const applet = aixApplet || agentWiki ? null : readWorkbenchApplet()
const fileEditor = agentWiki || applet ? null : readFileEditor()
const mailCompose = applet || fileEditor ? false : readMailCompose()
const notice = applet || fileEditor || mailCompose ? null : readNotice()
const root = createRoot(document.getElementById('root') as HTMLElement)
if (aixApplet) {
  document.getElementById('spinner')?.remove()
  // eslint-disable-next-line no-restricted-syntax
  console.timeEnd('init')
  const { AixAppletWindow } = await import('@renderer/pages/plugins/AixAppletWindow')
  root.render(<AixAppletWindow slug={aixApplet.slug} view={aixApplet.view} title={aixApplet.title} />)
} else if (agentWiki) {
  document.getElementById('spinner')?.remove()
  // eslint-disable-next-line no-restricted-syntax
  console.timeEnd('init')
  const { AgentWikiWindow } = await import('@renderer/pages/agents/AgentWikiWindow')
  root.render(<AgentWikiWindow agentId={agentWiki.agentId} title={agentWiki.title} />)
} else if (fileEditor) {
  if (isCloudWorkspace(fileEditor.root)) adoptEditorCenter(fileEditor.root.slice(CLOUD_WORKSPACE_SCHEME.length))
  document.getElementById('spinner')?.remove()
  // eslint-disable-next-line no-restricted-syntax
  console.timeEnd('init')
  const { FileEditorWindow } = await (editorChunk ?? import('@renderer/pages/documents/FileEditorWindow'))
  root.render(<FileEditorWindow root={fileEditor.root} path={fileEditor.path} readOnly={fileEditor.readOnly} />)
} else if (mailCompose) {
  document.getElementById('spinner')?.remove()
  // eslint-disable-next-line no-restricted-syntax
  console.timeEnd('init')
  const { MailComposeWindow } = await import('@renderer/pages/mail/MailComposeWindow')
  root.render(<MailComposeWindow />)
} else if (notice) {
  document.getElementById('spinner')?.remove()
  // eslint-disable-next-line no-restricted-syntax
  console.timeEnd('init')
  const { NoticeWindow } = await import('@renderer/pages/settings/AboutSettings')
  root.render(<NoticeWindow slug={notice} />)
} else if (applet) {
  document.getElementById('spinner')?.remove()
  // Paired with console.time('init') in index.html. The applet window does not mount MainApp.
  // eslint-disable-next-line no-restricted-syntax
  console.timeEnd('init')
  const { WorkbenchAppletWindow } = await import('@renderer/pages/agents/WorkbenchAppletWindow')
  root.render(<WorkbenchAppletWindow appletId={applet.id} channel={applet.channel} tool={applet.tool} />)
} else {
  root.render(<MainApp />)
}
