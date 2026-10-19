import { openUiFormFromJson } from '@a3s-lab/intelligent-ui/openui'
import {
  Brain,
  Check,
  ChevronDown,
  CircleAlert,
  Copy,
  File,
  FileCode,
  FileQuestion,
  FileText,
  Folder,
  FoldVertical,
  Globe,
  Image as ImageIcon,
  Link2,
  Loader2
} from 'lucide-react'
import {
  lazy,
  Suspense,
  useEffect,
  useId,
  useLayoutEffect,
  useMemo,
  useRef,
  useState,
  type MouseEvent,
  type ReactNode
} from 'react'
import { createPortal } from 'react-dom'
import { useTranslation } from 'react-i18next'
import type { Components } from 'streamdown'

import './task-conversation.css'

import { Streamdown, useIsCodeFenceIncomplete } from 'streamdown'

import { FormPartView } from '@renderer/components/chat/form/FormPartView'
import AgentSessionForkBlock from '@renderer/components/chat/messages/blocks/AgentSessionForkBlock'
import MessageImageBlock from '@renderer/components/chat/messages/blocks/MessageImageBlock'
import { useMessageParts } from '@renderer/components/chat/messages/blocks/MessagePartsContext'
import RetryStatusBlock from '@renderer/components/chat/messages/blocks/RetryStatusBlock'
import TranslationBlock from '@renderer/components/chat/messages/blocks/TranslationBlock'
import MessageAttachments from '@renderer/components/chat/messages/frame/MessageAttachments'
import { OpenUiMessage } from '@renderer/components/chat/messages/markdown/OpenUiMessage'
import { splitOpenUiFences } from '@renderer/components/chat/messages/markdown/splitOpenUiFences'
import { useOptionalMessageListActions } from '@renderer/components/chat/messages/MessageListProvider'
import { useOptionalRightPanelActions } from '@renderer/components/chat/panes/Shell'
import { filesDriveHref, parseTeamPassage, teamPassageIsStale } from '@renderer/drive/encyclopediaClient'
import { TaskFindingSave } from '@renderer/drive/TaskFindingSave'
import { useTabs } from '@renderer/hooks/tab'
import { publishWikiRetrieval } from '@renderer/pages/knowledge/wikiRetrievalPhase'
import {
  catalogPublishesReview,
  ensurePluginCatalog,
  usePluginCatalog
} from '@renderer/pages/settings/DependenciesSettings/pluginCatalog'
import { fileHandleFromPart } from '@renderer/utils/file/fileHandle'
import { isInlineFilePath, normalizeInlineFilePath, parseFileLinkHref } from '@renderer/utils/filePath'
import { stripCitationMarkers } from '@renderer/utils/message/citations'
import { getNodeText } from '@renderer/utils/reactNodeText'
import type { CompactionAnchorData } from '@shared/ai/compaction'
import type { CherryMessagePart } from '@shared/data/types/message'
import { proofFromParts, type WorkbenchProof } from '@shared/types/workbenchProof'

import { noteOfficialWritingAgentProse } from '../officialWritingAgentEdit'
import { notePresentationAgentProse } from '../presentationDeck'
import { useOptionalWorkbenchWorkspaceTabs } from '../workbenchWorkspaceTabs'
import { highlightTaskCode } from './highlightTaskCode'
import { TaskAutoReview } from './TaskAutoReview'
import {
  extractTaskCodeDiff,
  parseDirectoryListing,
  taskToolCopy,
  taskToolPayload,
  taskToolResultText,
  wikiSearchView,
  type DirectoryListing,
  type DirectoryListingEntry,
  type WikiSearchHit
} from './taskDiff'
import { TaskDiffView } from './TaskDiffView'
import {
  normalizeTaskConversationMarkdown,
  prepareMermaidDiagram,
  projectTaskConversationTurn,
  splitTrailingOpenFence,
  turnFileEdits,
  workspaceProductsForTurn,
  type TaskToolState,
  type TaskTurnItem,
  type TaskWorkspaceProductKind
} from './turn'
import { turnShowsWorkingStatus, WorkingStatus } from './WorkingStatus'

const MessageVideo = lazy(() => import('@renderer/components/chat/messages/frame/MessageVideo'))

const TASK_CODE_LANGUAGES = [
  'typescript',
  'ts',
  'tsx',
  'javascript',
  'js',
  'jsx',
  'python',
  'py',
  'bash',
  'sh',
  'shell',
  'json',
  'rust',
  'rs',
  'go',
  'css',
  'html',
  'sql',
  'yaml',
  'yml',
  'markdown',
  'md',
  'text',
  'plaintext',
  'txt'
]

const TASK_PLUGINS = {
  renderers: [
    { language: 'mermaid', component: TaskMermaidRenderer },
    { language: TASK_CODE_LANGUAGES, component: TaskCodeRenderer }
  ]
}

const TASK_COMPONENTS = {
  a: TaskLink,
  code: TaskInlineOrFence,
  inlineCode: TaskInlineCode,
  p: TaskParagraph
} satisfies Partial<Components>

const TASK_CONTROLS = { code: false, mermaid: false, table: false } as const

const DIAGRAM_KINDS: ReadonlyArray<{ id: string; key: string; test: RegExp }> = [
  { id: 'sequence', key: 'workbench.diagram.sequence', test: /^sequencediagram\b/i },
  { id: 'class', key: 'workbench.diagram.class', test: /^classdiagram\b/i },
  { id: 'state', key: 'workbench.diagram.state', test: /^statediagram(?:-v2)?\b/i },
  { id: 'er', key: 'workbench.diagram.er', test: /^erdiagram\b/i },
  { id: 'gantt', key: 'workbench.diagram.gantt', test: /^gantt\b/i },
  { id: 'pie', key: 'workbench.diagram.pie', test: /^pie\b/i },
  { id: 'mindmap', key: 'workbench.diagram.mindmap', test: /^mindmap\b/i },
  { id: 'git', key: 'workbench.diagram.git', test: /^gitgraph\b/i },
  { id: 'journey', key: 'workbench.diagram.journey', test: /^journey\b/i },
  { id: 'timeline', key: 'workbench.diagram.timeline', test: /^timeline\b/i },
  { id: 'quadrant', key: 'workbench.diagram.quadrant', test: /^quadrantchart\b/i },
  {
    id: 'architecture',
    key: 'workbench.diagram.architecture',
    test: /^(?:architecture-beta|c4(?:context|container|component|dynamic|deployment))\b/i
  },
  { id: 'flowchart', key: 'workbench.diagram.flowchart', test: /^(?:flowchart|graph)\b/i }
]

function diagramKind(source: string): { id: string; key: string } {
  const first = source.trim().split('\n', 1)[0] ?? ''
  return DIAGRAM_KINDS.find((kind) => kind.test.test(first)) ?? { id: 'diagram', key: 'workbench.diagram.chart' }
}

function DiagramHeader({ kind }: { kind: { id: string; key: string } }) {
  const { t } = useTranslation()
  return (
    <div data-chat-diagram-header="">
      <span data-task-diagram-label="">{t(kind.key)}</span>
    </div>
  )
}

function taskToolStateKey(state: TaskToolState): string {
  if (state === 'pending') return 'workbench.tool.pending'
  if (state === 'running') return 'workbench.tool.running'
  if (state === 'success') return 'workbench.tool.done'
  return 'workbench.tool.failed'
}

function splitReasoningPeek(text: string, maxLines = 5): { earlier: string; tail: string } {
  const normalized = text.replace(/\r\n/g, '\n').replace(/\r/g, '\n')
  const lines = normalized.split('\n')
  if (lines.length <= maxLines) return { earlier: '', tail: normalized }
  return {
    earlier: lines.slice(0, -maxLines).join('\n'),
    tail: lines.slice(-maxLines).join('\n')
  }
}

function TaskDisclosure({ open, children }: { open: boolean; children: ReactNode }) {
  return (
    <div
      aria-hidden={open ? undefined : true}
      data-open={open ? 'true' : undefined}
      data-task-disclosure=""
      inert={!open}>
      <div data-task-disclosure-clip="">{children}</div>
    </div>
  )
}

function isFenceNode(node: ReactNode): boolean {
  if (!node || typeof node !== 'object' || !('type' in node)) return false
  const type = (node as { type?: unknown }).type
  if (type === TaskFence || type === TaskCodeRenderer || type === TaskMermaidRenderer) return true
  if (type !== Suspense) return false
  return isFenceNode((node as { props?: { children?: ReactNode } }).props?.children)
}

function TaskParagraph({ children }: { children?: ReactNode }) {
  const nodes = Array.isArray(children) ? children : [children]
  if (nodes.length === 1 && isFenceNode(nodes[0])) return nodes[0]
  return <p>{children}</p>
}

function TaskToolMark({ state }: { state: TaskToolState }) {
  if (state === 'pending') return <i aria-hidden="true" data-task-tool-wait="" />
  if (state === 'running') return <i aria-hidden="true" data-task-tool-spinner="" data-tool-spinner="" />
  if (state === 'success') return <Check aria-hidden="true" size={16} strokeWidth={1.7} />
  return <CircleAlert aria-hidden="true" size={16} strokeWidth={1.7} />
}

function ToolPayload({ kind, label, text }: { kind: 'input' | 'output' | 'error'; label: string; text: string }) {
  return (
    <section className="task-tool-payload" data-tool-payload={kind}>
      <h4>{label}</h4>
      <pre
        className="task-tool-result"
        data-task-tool-error={kind === 'error' ? '' : undefined}
        data-task-tool-result={kind === 'output' ? '' : undefined}
        data-tool-input={kind === 'input' ? '' : undefined}
        data-tool-output={kind === 'output' ? '' : undefined}>
        {text}
      </pre>
    </section>
  )
}

export function TaskToolRow({ name, part, state }: { name: string; part?: unknown; state: TaskToolState }) {
  const { t } = useTranslation()
  const openArtifactFile = useOptionalMessageListActions()?.openArtifactFile
  const diff = useMemo(() => extractTaskCodeDiff(name, part), [name, part])
  const wiki = useMemo(() => wikiSearchView(name, part), [name, part])
  const copy = useMemo(() => taskToolCopy(name, part, diff), [diff, name, part])
  const payload = useMemo(() => {
    const next = taskToolPayload(part)
    if (!wiki) return next
    return { ...next, output: undefined }
  }, [part, wiki])
  const resultText = useMemo(() => (diff || wiki ? undefined : taskToolResultText(part)), [diff, part, wiki])
  const directory = useMemo(() => (resultText ? parseDirectoryListing(resultText) : undefined), [resultText])
  useEffect(() => {
    if (!wiki) return
    if (state === 'error') {
      publishWikiRetrieval('failed')
      return
    }
    publishWikiRetrieval(wiki.phase === 'preparing' ? 'preparing' : 'idle')
  }, [state, wiki])
  const fallbackOutput = resultText || diff || wiki || directory ? undefined : payload.output
  // A diff already shows the file body. Other calls keep a short parameter block.
  const inputText = diff ? undefined : payload.input
  const outputText = directory ? undefined : (resultText ?? fallbackOutput)
  const expandable = Boolean(diff || inputText || outputText || directory || wiki?.phase === 'hits' || payload.error)
  const [open, setOpen] = useState(false)
  const title = copy.titleKey ? t(copy.titleKey, copy.titleValues) : (copy.titleText ?? name)
  const summary = [copy.summaryKey ? t(copy.summaryKey) : '', copy.summaryText ?? ''].filter(Boolean).join(' · ')
  const summaryNode = (
    <span data-tool-summary="">
      <span data-agent-tool-icon="">
        <TaskToolMark state={state} />
      </span>
      <span data-agent-tool-copy="">
        <strong data-task-tool-label="">{title}</strong>
        {summary ? <small>{summary}</small> : null}
      </span>
      <em data-state={state} data-task-tool-status="" data-tool-status={state}>
        {copy.statusKey ? t(copy.statusKey) : t(taskToolStateKey(state))}
      </em>
      {expandable ? <ChevronDown aria-hidden="true" size={16} strokeWidth={1.7} /> : null}
    </span>
  )
  const hover = useDiffHover(Boolean(diff) && !open)
  const details = expandable ? (
    <TaskDisclosure open={open}>
      <div data-agent-tool-details="">
        {inputText ? <ToolPayload kind="input" label={t('workbench.tool.input_label')} text={inputText} /> : null}
        {diff ? (
          <TaskDiffView
            diff={diff}
            failed={state === 'error'}
            onOpenFile={openArtifactFile ? (path) => void openArtifactFile(path) : undefined}
          />
        ) : null}
        {wiki?.phase === 'hits' ? (
          <section className="task-tool-payload" data-tool-payload="output">
            <h4>{t('workbench.tool.output_label')}</h4>
            <WikiSearchHits hits={wiki.hits} />
          </section>
        ) : null}
        {directory ? (
          <section className="task-tool-payload" data-tool-payload="output">
            <h4>{t('workbench.tool.output_label')}</h4>
            <DirectoryListing listing={directory} />
          </section>
        ) : null}
        {outputText ? <ToolPayload kind="output" label={t('workbench.tool.output_label')} text={outputText} /> : null}
        {payload.error ? (
          <ToolPayload kind="error" label={t('workbench.tool.error_label')} text={payload.error} />
        ) : null}
      </div>
    </TaskDisclosure>
  ) : null

  if (!expandable) {
    return (
      <section
        className="task-tool-row"
        data-state={state}
        data-task-tool-name={name}
        data-task-tool-row=""
        data-task-tool-state={state}
        data-tool-call="true">
        <div className="task-tool-row__summary">{summaryNode}</div>
      </section>
    )
  }

  return (
    <>
      <details
        className="task-tool-row"
        data-open={open ? 'true' : undefined}
        data-state={state}
        data-task-tool-name={name}
        data-task-tool-row=""
        data-task-tool-state={state}
        data-tool-call="true"
        open={open}
        onToggle={(event) => setOpen(event.currentTarget.open)}>
        <summary
          className="task-tool-row__summary"
          {...hover.handlers}
          onClick={(event) => {
            event.preventDefault()
            setOpen((value) => !value)
          }}>
          {summaryNode}
        </summary>
        {details}
      </details>
      {hover.point && diff
        ? createPortal(
            <div className="task-conversation">
              <div
                className="task-hover-preview"
                data-code-diff-hover-preview="true"
                style={{ left: hover.point.x, top: hover.point.y }}
                {...hover.previewHandlers}>
                <TaskDiffView diff={diff} failed={state === 'error'} />
              </div>
            </div>,
            document.body
          )
        : null}
    </>
  )
}

const DIRECTORY_ICON = { size: 16, strokeWidth: 1.7 } as const

function DirectoryKindIcon({ kind }: { kind: DirectoryListingEntry['kind'] }) {
  if (kind === 'dir') return <Folder aria-hidden="true" {...DIRECTORY_ICON} />
  if (kind === 'link') return <Link2 aria-hidden="true" {...DIRECTORY_ICON} />
  if (kind === 'unknown') return <FileQuestion aria-hidden="true" {...DIRECTORY_ICON} />
  return <File aria-hidden="true" {...DIRECTORY_ICON} />
}

function DirectoryListing({ listing }: { listing: DirectoryListing }) {
  return (
    <div className="task-dir" data-task-directory="">
      <p className="task-dir__path">{listing.path}</p>
      {listing.empty ? (
        <p className="task-dir__empty">(empty directory)</p>
      ) : (
        <ul className="task-dir__list">
          {listing.entries.map((entry, index) => (
            <li key={`${entry.kind}:${entry.name}:${index}`} className="task-dir__row" data-kind={entry.kind}>
              <DirectoryKindIcon kind={entry.kind} />
              <span className="task-dir__name">{entry.name}</span>
              <span className="task-dir__size">{entry.size}</span>
            </li>
          ))}
        </ul>
      )}
      {listing.footer ? <p className="task-dir__meta">{listing.footer}</p> : null}
      {listing.more ? <p className="task-dir__meta">{listing.more}</p> : null}
    </div>
  )
}

function WikiSearchHits({ hits }: { hits: WikiSearchHit[] }) {
  const newestGeneration = new Map<string, number>()
  for (const hit of hits) {
    const team = parseTeamPassage(hit.path)
    if (!team) continue
    const known = newestGeneration.get(team.fileId)
    if (known === undefined || team.generation > known) newestGeneration.set(team.fileId, team.generation)
  }
  return (
    <ul className="task-wiki-hits" data-wiki-search-hits="">
      {hits.map((hit) => {
        const team = parseTeamPassage(hit.path)
        const newest = team ? newestGeneration.get(team.fileId) : undefined
        const olderThanSibling = team !== null && newest !== undefined && team.generation < newest
        return (
          <li key={`${hit.path}\n${hit.name}`} className="task-wiki-hit" data-wiki-search-hit="">
            <span className="task-wiki-hit__name">{hit.name}</span>
            {hit.path ? <WikiHitPlace olderThanSibling={olderThanSibling} path={hit.path} /> : null}
            {hit.passage ? <p className="task-wiki-hit__passage">{hit.passage}</p> : null}
          </li>
        )
      })}
    </ul>
  )
}

/** A team passage opens its drive file. A newer generation is flagged first; a second click still opens it. */
function WikiHitPlace({ olderThanSibling = false, path }: { olderThanSibling?: boolean; path: string }) {
  const { t } = useTranslation()
  const { openTab } = useTabs()
  const [stale, setStale] = useState(false)
  const team = parseTeamPassage(path)
  if (!team) return <span className="task-wiki-hit__path">{path}</span>
  const href = filesDriveHref(team.fileId)
  return (
    <>
      <a
        className="task-wiki-hit__path"
        data-ui="wiki.team-citation"
        data-passage={team.passageId}
        href={href}
        onClick={(event) => {
          event.preventDefault()
          if (stale) {
            openTab(href)
            return
          }
          // A newer hit for the same file is already in this result. Flag it
          // before asking the encyclopedia, and still open on the next click.
          if (olderThanSibling) {
            setStale(true)
            return
          }
          void teamPassageIsStale(team)
            .catch(() => false)
            .then((isStale) => {
              if (isStale) setStale(true)
              else openTab(href)
            })
        }}>
        {t('knowledge.encyclopedia.open_source')}
      </a>
      {stale ? (
        <span className="task-wiki-hit__path" data-ui="wiki.team-citation-stale" role="status">
          {t('knowledge.encyclopedia.passage_updated')}
        </span>
      ) : null}
    </>
  )
}

function useDiffHover(enabled: boolean) {
  const [point, setPoint] = useState<{ x: number; y: number } | null>(null)
  const anchorRef = useRef<HTMLElement | null>(null)
  const openTimer = useRef<number | null>(null)
  const closeTimer = useRef<number | null>(null)
  const clearOpen = () => {
    if (openTimer.current !== null) window.clearTimeout(openTimer.current)
    openTimer.current = null
  }
  const clearClose = () => {
    if (closeTimer.current !== null) window.clearTimeout(closeTimer.current)
    closeTimer.current = null
  }
  useEffect(() => {
    return () => {
      clearOpen()
      clearClose()
    }
  }, [])
  useEffect(() => {
    if (!point) return undefined
    const onKey = (event: globalThis.KeyboardEvent) => {
      if (event.key === 'Escape') setPoint(null)
    }
    window.addEventListener('keydown', onKey)
    return () => window.removeEventListener('keydown', onKey)
  }, [point])
  const scheduleClose = () => {
    clearClose()
    closeTimer.current = window.setTimeout(() => {
      closeTimer.current = null
      setPoint(null)
    }, 160)
  }
  const handlers = {
    onMouseEnter: (event: MouseEvent<HTMLElement>) => {
      if (!enabled) return
      anchorRef.current = event.currentTarget
      clearClose()
      if (point || openTimer.current !== null) return
      openTimer.current = window.setTimeout(() => {
        openTimer.current = null
        const anchor = anchorRef.current
        if (!anchor) return
        setPoint(placeHover(anchor.getBoundingClientRect()))
      }, 280)
    },
    onMouseLeave: () => {
      clearOpen()
      scheduleClose()
    }
  }
  const previewHandlers = {
    onMouseEnter: () => clearClose(),
    onMouseLeave: () => scheduleClose()
  }
  return { handlers, previewHandlers, point }
}

function placeHover(rect: DOMRect): { x: number; y: number } {
  const width = Math.min(576, window.innerWidth - 24)
  const left = Math.min(Math.max(12, rect.left), Math.max(12, window.innerWidth - width - 12))
  const estimated = 240
  const below = rect.bottom + 8
  const top = below + estimated > window.innerHeight - 12 ? Math.max(12, rect.top - estimated - 8) : below
  return { x: left, y: top }
}

function FenceCard({ code, language, diagram }: { code: string; language?: string; diagram?: boolean }) {
  if (diagram) {
    const kind = diagramKind(code)
    return (
      <div
        data-chat-diagram="source"
        data-diagram-kind={kind.id}
        data-language="mermaid"
        data-task-diagram="source"
        data-task-fence="source">
        <DiagramHeader kind={kind} />
        <pre>
          <code>{code}</code>
        </pre>
      </div>
    )
  }
  return (
    <div data-chat-code="source" data-language={language ?? ''} data-streamdown="code-block" data-task-fence="source">
      <div data-streamdown="code-block-header">
        <span data-task-code-language="">{language}</span>
      </div>
      <div data-streamdown="code-block-body">
        <pre>
          <code>{code}</code>
        </pre>
      </div>
    </div>
  )
}

function TaskHighlightedCode({ code, language }: { code: string; language: string }) {
  const { t } = useTranslation()
  const [html, setHtml] = useState<string | undefined>()
  const [copied, setCopied] = useState(false)

  useEffect(() => {
    let cancelled = false
    void highlightTaskCode(code, language).then((next) => {
      if (!cancelled && next) setHtml(next)
    })
    return () => {
      cancelled = true
    }
  }, [code, language])

  async function copySource() {
    try {
      await navigator.clipboard.writeText(code)
      setCopied(true)
      window.setTimeout(() => setCopied(false), 1600)
    } catch {
      setCopied(false)
    }
  }

  return (
    <div
      data-chat-code={html ? 'shiki' : 'source'}
      data-language={language}
      data-streamdown="code-block"
      data-task-code={html ? 'highlighted' : 'pending'}>
      <div data-streamdown="code-block-header">
        <span data-task-code-language="">{language}</span>
      </div>
      <div data-streamdown="code-block-actions">
        <span
          aria-label={copied ? t('workbench.code.copied') : t('workbench.code.copy')}
          data-streamdown="code-block-copy-button"
          tabIndex={0}
          onClick={() => void copySource()}
          onKeyDown={(event) => {
            if (event.key !== 'Enter' && event.key !== ' ') return
            event.preventDefault()
            void copySource()
          }}>
          {copied ? <Check size={14} strokeWidth={1.7} /> : <Copy size={14} strokeWidth={1.7} />}
          <span
            style={{
              position: 'absolute',
              width: 1,
              height: 1,
              padding: 0,
              margin: -1,
              overflow: 'hidden',
              clip: 'rect(0, 0, 0, 0)',
              whiteSpace: 'nowrap',
              border: 0
            }}>
            {copied ? t('workbench.code.copied') : t('workbench.code.copy')}
          </span>
        </span>
      </div>
      <div data-streamdown="code-block-body" data-task-code-body="">
        {html ? (
          <div dangerouslySetInnerHTML={{ __html: html }} />
        ) : (
          <pre>
            <code>{code}</code>
          </pre>
        )}
      </div>
    </div>
  )
}

function safeDiagramSvg(svg: string): string | undefined {
  if (/<script|on\w+=/i.test(svg)) return undefined
  return svg
}

function TaskMermaidDiagram({ source }: { source: string }) {
  const { t } = useTranslation()
  const reactId = useId().replace(/:/g, '')
  const [svg, setSvg] = useState<string | undefined>()
  const [failed, setFailed] = useState(false)
  const prepared = useMemo(() => prepareMermaidDiagram(source), [source])
  const kind = diagramKind(prepared)

  useEffect(() => {
    let cancelled = false
    setFailed(false)
    setSvg(undefined)
    const dark = document.documentElement.classList.contains('dark')
    void import('mermaid')
      .then(async (mod) => {
        const mermaid = mod.default
        mermaid.initialize({ startOnLoad: false, securityLevel: 'strict', theme: dark ? 'dark' : 'default' })
        const rendered = await mermaid.render(`task-mermaid-${reactId}`, prepared)
        const next = safeDiagramSvg(rendered.svg)
        if (cancelled) return
        if (!next) setFailed(true)
        else setSvg(next)
      })
      .catch(() => {
        if (!cancelled) setFailed(true)
      })
    return () => {
      cancelled = true
    }
  }, [prepared, reactId])

  if (failed) {
    return (
      <div data-chat-diagram="failed" data-diagram-kind={kind.id} data-task-diagram="source">
        <DiagramHeader kind={kind} />
        <p data-chat-diagram-note="">{t('workbench.diagram.failed')}</p>
        <pre>
          <code>{prepared}</code>
        </pre>
      </div>
    )
  }
  if (!svg) {
    return (
      <div aria-busy="true" data-chat-diagram="loading" data-diagram-kind={kind.id} data-task-diagram="loading">
        <DiagramHeader kind={kind} />
        <div data-chat-diagram-body="">
          <span data-chat-diagram-skeleton="" />
        </div>
      </div>
    )
  }
  return (
    <div
      data-chat-diagram="rendered"
      data-chat-diagram-engine="mermaid"
      data-diagram-kind={kind.id}
      data-task-diagram="rendered">
      <DiagramHeader kind={kind} />
      <div data-chat-diagram-body="">
        <div data-chat-diagram-canvas="" dangerouslySetInnerHTML={{ __html: svg }} />
      </div>
    </div>
  )
}

function TaskFence({ code, language, isIncomplete }: { code: string; language: string; isIncomplete: boolean }) {
  const { t } = useTranslation()
  const catalog = usePluginCatalog()
  useEffect(() => {
    void ensurePluginCatalog()
  }, [])
  const normalized = language.toLowerCase()
  if (normalized === 'openui') return <OpenUiMessage source={code} streaming={isIncomplete} />
  if (normalized === 'json' && !isIncomplete) {
    const form = openUiFormFromJson(code)
    if (form) return <OpenUiMessage source={form} streaming={false} />
  }
  if (!isIncomplete && !code.trim()) return null
  if (normalized === 'a3s-review-finding' || normalized === 'a3s-review-findings') {
    if (!catalogPublishesReview(catalog)) return <FenceCard code={code} language={normalized} />
    return (
      <details data-review-protocol="">
        <summary>{t('workbench.review.protocol')}</summary>
        <FenceCard code={code} language={normalized} />
      </details>
    )
  }
  if (normalized === 'mermaid') {
    if (isIncomplete) return <FenceCard code={code} diagram language="mermaid" />
    return <TaskMermaidDiagram source={code} />
  }
  if (isIncomplete) return <FenceCard code={code} language={normalized || language} />
  return <TaskHighlightedCode code={code} language={normalized || language} />
}

function TaskMermaidRenderer({
  code,
  isIncomplete
}: {
  code: string
  isIncomplete: boolean
  language: string
  meta?: string
}) {
  return <TaskFence code={code} isIncomplete={isIncomplete} language="mermaid" />
}

function TaskCodeRenderer({
  code,
  isIncomplete,
  language
}: {
  code: string
  isIncomplete: boolean
  language: string
  meta?: string
}) {
  return <TaskFence code={code} isIncomplete={isIncomplete} language={language} />
}

function TaskPathCitation({
  children,
  codeClassName,
  path
}: {
  children?: ReactNode
  codeClassName?: string
  path: string
}) {
  const { t } = useTranslation()
  const openArtifactFile = useOptionalMessageListActions()?.openArtifactFile
  const body = codeClassName ? <code className={codeClassName}>{children}</code> : children
  if (!openArtifactFile) return <>{body}</>
  return (
    <span
      aria-label={t('workbench.diff.open', { path })}
      className="task-path-citation"
      data-workspace-path-citation="true"
      tabIndex={0}
      title={path}
      onClick={() => void openArtifactFile(path)}
      onKeyDown={(event) => {
        if (event.key !== 'Enter' && event.key !== ' ') return
        event.preventDefault()
        void openArtifactFile(path)
      }}>
      {body}
    </span>
  )
}

function TaskInlineCode({ children, className }: { children?: ReactNode; className?: string }) {
  const text = getNodeText(children).trim()
  if (!isInlineFilePath(text)) return <code className={className}>{children}</code>
  return (
    <TaskPathCitation codeClassName={className} path={normalizeInlineFilePath(text)}>
      {children}
    </TaskPathCitation>
  )
}

function TaskLink({ children, href }: { children?: ReactNode; href?: string }) {
  const file = parseFileLinkHref(href)
  const external = typeof href === 'string' && /^https?:/i.test(href)
  if (file) return <TaskPathCitation path={file}>{children}</TaskPathCitation>
  return (
    <a href={href} rel={external ? 'noreferrer noopener' : undefined} target={external ? '_blank' : undefined}>
      {children}
    </a>
  )
}

function TaskInlineOrFence({ children, className }: { children?: ReactNode; className?: string }) {
  const text = getNodeText(children)
  const language = /language-([\w-+]+)/.exec(className || '')?.[1] ?? null
  const isIncomplete = useIsCodeFenceIncomplete()
  if (!language) return <code className={className}>{children}</code>
  return <TaskFence code={text} isIncomplete={isIncomplete} language={language} />
}

function TaskFilePart({ part, createdAt }: { part: Extract<CherryMessagePart, { type: 'file' }>; createdAt: string }) {
  if (part.mediaType.startsWith('image/')) {
    if (!part.url) return null
    return <MessageImageBlock isSingle sources={[{ handle: fileHandleFromPart(part), url: part.url }]} />
  }
  const handle = fileHandleFromPart(part)
  if (!handle) return null
  const name = part.filename ?? ''
  return (
    <MessageAttachments createdAt={createdAt} ext={name.match(/\.[^.]+$/)?.[0] ?? ''} handle={handle} name={name} />
  )
}

function TaskReasoningRow({ id, streaming, text }: { id: string; streaming: boolean; text: string }) {
  const { t } = useTranslation()
  const [expanded, setExpanded] = useState(false)
  const scrollerRef = useRef<HTMLDivElement>(null)
  const peek = splitReasoningPeek(text)
  const showPeek = streaming && !expanded
  const truncated = showPeek && peek.earlier.length > 0
  useLayoutEffect(() => {
    const node = scrollerRef.current
    if (!showPeek || !node) return
    node.scrollTop = node.scrollHeight
  }, [showPeek, peek.tail])
  if (!text.trim()) return null
  return (
    <section
      className="task-reasoning"
      data-open={expanded ? 'true' : undefined}
      data-reasoning-preview={showPeek ? 'true' : undefined}
      data-state={streaming ? 'running' : 'success'}
      data-ui="part:message-reasoning"
      id={id}>
      <button
        aria-expanded={expanded}
        className="task-reasoning__summary"
        type="button"
        onClick={() => setExpanded((value) => !value)}>
        <span data-tool-summary="">
          <span data-agent-tool-icon="">
            <Brain aria-hidden="true" size={16} strokeWidth={1.7} />
          </span>
          <span data-agent-tool-copy="">
            <strong>{streaming ? t('workbench.reasoning.running') : t('workbench.reasoning.done')}</strong>
          </span>
          <ChevronDown aria-hidden="true" size={16} strokeWidth={1.7} />
        </span>
      </button>
      {expanded ? (
        <div data-reasoning-content="" data-reasoning-full="">
          <p className="task-reasoning__body" data-reasoning-body="">
            {text}
          </p>
        </div>
      ) : showPeek ? (
        <div
          ref={scrollerRef}
          data-reasoning-content=""
          data-reasoning-summary=""
          data-reasoning-truncated={truncated ? 'true' : undefined}>
          <p className="task-reasoning__body" data-reasoning-body="">
            {peek.tail}
          </p>
        </div>
      ) : null}
    </section>
  )
}

function TaskCompactRow({ id, content, compactedContent }: { id: string; content: string; compactedContent: string }) {
  const { t } = useTranslation()
  const [open, setOpen] = useState(false)
  const summary = content.trim()
  const original = compactedContent.trim()
  if (!summary && !original) return null
  return (
    <details
      className="task-reasoning"
      data-open={open ? 'true' : undefined}
      data-ui="part:message-compact"
      id={id}
      open={open}
      onToggle={(event) => setOpen(event.currentTarget.open)}>
      <summary
        className="task-reasoning__summary"
        onClick={(event) => {
          event.preventDefault()
          setOpen((value) => !value)
        }}>
        <span data-tool-summary="">
          <span data-agent-tool-icon="">
            <FoldVertical aria-hidden="true" size={16} strokeWidth={1.7} />
          </span>
          <span data-agent-tool-copy="">
            <strong>{t('message.message.compact.title')}</strong>
          </span>
          <ChevronDown aria-hidden="true" size={16} strokeWidth={1.7} />
        </span>
      </summary>
      <TaskDisclosure open={open}>
        <div data-reasoning-content="">
          {summary ? <p className="task-reasoning__body">{summary}</p> : null}
          {original ? (
            <p className="task-reasoning__body" data-compact-original="">
              {original}
            </p>
          ) : null}
        </div>
      </TaskDisclosure>
    </details>
  )
}

function compactionSavedTokens(data: CompactionAnchorData | undefined): number | undefined {
  if (data?.preTokens === undefined || data.postTokens === undefined || data.preTokens <= data.postTokens) {
    return undefined
  }
  return data.preTokens - data.postTokens
}

function TaskCompactionAnchor({ data }: { data?: CompactionAnchorData }) {
  const { t } = useTranslation()
  if (data?.status === 'skipped') return null
  const compacting = data?.status === 'compacting'
  const saved = compactionSavedTokens(data)
  if (!compacting && saved === undefined && data?.phase !== 'in-loop') return null
  const label = compacting
    ? t('chat.compaction.compacting')
    : saved === undefined
      ? t('chat.compaction.compacted_plain')
      : t('chat.compaction.compacted', { count: saved })
  return (
    <p
      className="task-compaction"
      data-state={compacting ? 'running' : 'done'}
      role={compacting ? 'status' : undefined}
      aria-live={compacting ? 'polite' : undefined}>
      {compacting ? <Loader2 aria-hidden="true" size={12} strokeWidth={1.7} /> : null}
      {label}
    </p>
  )
}

function TaskConversationReset() {
  const { t } = useTranslation()
  return <p className="task-compaction">{t('message.conversation_reset')}</p>
}

function TaskTurnBlock({
  part,
  partId,
  streaming,
  createdAt,
  text
}: {
  part: CherryMessagePart
  partId: string
  streaming: boolean
  createdAt: string
  text?: string
}) {
  switch (part.type) {
    case 'reasoning':
      return (
        <TaskReasoningRow
          id={partId}
          streaming={streaming && part.state === 'streaming'}
          text={stripCitationMarkers(text ?? part.text ?? '')}
        />
      )
    case 'file':
      return <TaskFilePart createdAt={createdAt} part={part} />
    case 'data-form':
      return <FormPartView part={part} />
    case 'data-translation':
      return <TranslationBlock id={partId} content={part.data.content} isStreaming={streaming} />
    case 'data-video': {
      const data = part.data
      if (!data || (!data.url && !data.filePath)) return null
      return (
        <Suspense fallback={null}>
          <MessageVideo filePath={data.filePath} url={data.url} />
        </Suspense>
      )
    }
    case 'data-compact':
      return <TaskCompactRow compactedContent={part.data.compactedContent} content={part.data.content} id={partId} />
    case 'data-compaction-anchor':
      return <TaskCompactionAnchor data={part.data} />
    case 'data-conversation-reset':
      return <TaskConversationReset />
    case 'data-agent-session-fork':
      return <AgentSessionForkBlock sourceSessionId={part.data.sourceSessionId} />
    case 'data-retry':
      if (!part.data) return null
      return <RetryStatusBlock data={part.data} />
    default:
      return null
  }
}

function ProductMark({ kind }: { kind: TaskWorkspaceProductKind }) {
  const Icon =
    kind === 'page'
      ? Globe
      : kind === 'image'
        ? ImageIcon
        : kind === 'document'
          ? FileText
          : kind === 'code'
            ? FileCode
            : File
  return <Icon aria-hidden="true" size={16} strokeWidth={1.7} />
}

function TaskWorkspaceProducts({
  messageId,
  streaming,
  proof
}: {
  messageId: string
  streaming: boolean
  proof: WorkbenchProof | null
}) {
  const { t } = useTranslation()
  const openArtifactFile = useOptionalMessageListActions()?.openArtifactFile
  const parts = useMessageParts(messageId)
  const products = useMemo(() => workspaceProductsForTurn(parts, streaming), [parts, streaming])
  const workspace = useOptionalWorkbenchWorkspaceTabs()
  const panel = useOptionalRightPanelActions()
  const artifacts = proof?.policy.artifactTrace ? proof.artifacts : []
  if (products.length === 0 && artifacts.length === 0 && !proof?.traceError) return null
  return (
    <section aria-label={t('workbench.products')} data-task-products="">
      <header className="task-products-bar">
        <span data-task-products-label="">
          {t('workbench.products')} · {products.length}
        </span>
      </header>
      <ul className="task-products-list">
        {products.map((product) => (
          <li key={product.path}>
            <div className="task-product-item">
              <button
                aria-label={t('workbench.products.open', { name: product.name })}
                className="task-product-row"
                data-task-product=""
                data-task-product-kind={product.kind}
                data-task-product-path={product.path}
                title={product.path}
                type="button"
                onClick={() => void openArtifactFile?.(product.path)}>
                <ProductMark kind={product.kind} />
                <span className="task-product-row__name">{product.name}</span>
              </button>
              <TaskFindingSave file={product} peers={products} streaming={streaming} />
            </div>
          </li>
        ))}
        {artifacts.map((artifact) => (
          <li key={artifact.id}>
            <button
              aria-label={t('workbench.lineage.open', { name: artifact.name })}
              className="task-lineage-open"
              data-task-lineage-open=""
              data-task-artifact-version={artifact.id}
              type="button"
              disabled={!workspace}
              onClick={() => {
                if (!proof || !workspace) return
                workspace.openLineage(proof, artifact.id)
                panel?.tryOpen('files', { userInitiated: true })
              }}>
              <Link2 aria-hidden="true" size={16} strokeWidth={1.7} />
              <span>{artifact.name}</span>
              <span>v{artifact.ordinal}</span>
              <code>{artifact.contentDigest.slice(7, 19)}</code>
              <span>
                {
                  {
                    traced: t('workbench.proof.trace_traced'),
                    incomplete: t('workbench.proof.trace_incomplete'),
                    failed: t('workbench.proof.trace_failed')
                  }[artifact.status]
                }
              </span>
            </button>
          </li>
        ))}
      </ul>
      {proof?.policy.artifactTrace && proof.traceError ? (
        <p role="alert">
          {t('workbench.proof.trace_failed')}: {proof.traceError}
        </p>
      ) : null}
    </section>
  )
}

function TaskProse({ id, text, streaming }: { id: string; text: string; streaming: boolean }) {
  const aixSegments = useMemo(() => splitOpenUiFences(text), [text])
  const source = useMemo(() => {
    const normalized = normalizeTaskConversationMarkdown(text)
    return splitTrailingOpenFence(normalized)
  }, [text])
  if (aixSegments.some((segment) => segment.kind !== 'markdown')) {
    return (
      <div className="task-conversation__markdown" data-task-prose={id}>
        {aixSegments.map((segment) =>
          segment.kind === 'openui' ? (
            <OpenUiMessage
              key={segment.start}
              source={segment.source}
              streaming={streaming || segment.incomplete === true}
            />
          ) : (
            <TaskProse
              key={segment.start}
              id={`${id}:prose:${segment.start}`}
              text={segment.source}
              streaming={streaming}
            />
          )
        )}
      </div>
    )
  }
  return (
    <div className="task-conversation__markdown" data-task-prose={id}>
      {source.closed ? (
        <Streamdown
          animated={false}
          className="task-conversation__markdown"
          components={TASK_COMPONENTS}
          controls={TASK_CONTROLS}
          dir="auto"
          isAnimating={streaming}
          lineNumbers={false}
          mode="streaming"
          parseIncompleteMarkdown={streaming}
          plugins={TASK_PLUGINS}>
          {source.closed}
        </Streamdown>
      ) : null}
      {source.open ? (
        <FenceCard
          code={source.open.code}
          diagram={source.open.language === 'mermaid'}
          language={source.open.language || undefined}
        />
      ) : null}
    </div>
  )
}

/** Assistant prose for a surface that already holds the text, such as ToBot. */
export function TaskConversationProse({ id, text, streaming }: { id: string; text: string; streaming: boolean }) {
  return <TaskProse id={id} text={text} streaming={streaming} />
}

/** Parsed attributes from a leaked call, shaped like a tool part's input bag. */
function leakedToolPart(input: Record<string, unknown> | undefined): { input: Record<string, unknown> } | undefined {
  if (!input || Object.keys(input).length === 0) return undefined
  return { input }
}

type TaskToolTurnItem = Extract<TaskTurnItem, { kind: 'tool' }>

type TaskTurnView = TaskTurnItem | { kind: 'tools'; id: string; tools: TaskToolTurnItem[] }

/** Consecutive tool calls share one left rail. Prose between calls starts a new run. */
function clusterConsecutiveTools(items: TaskTurnItem[]): TaskTurnView[] {
  const views: TaskTurnView[] = []
  let tools: TaskToolTurnItem[] = []
  const flush = () => {
    const [first, ...rest] = tools
    tools = []
    if (!first) return
    if (rest.length === 0) {
      views.push(first)
      return
    }
    views.push({ kind: 'tools', id: `tools:${first.id}`, tools: [first, ...rest] })
  }
  for (const item of items) {
    if (item.kind === 'tool') {
      tools.push(item)
      continue
    }
    flush()
    views.push(item)
  }
  flush()
  return views
}

/**
 * One assistant turn: reasoning, tool rows, and prose, in arrival order.
 * The task workspace and ToBot share this view.
 */
export function TaskConversationTurn({
  parts,
  streaming,
  awaiting = false,
  createdAt = '',
  children
}: {
  parts: readonly CherryMessagePart[]
  streaming: boolean
  /** The saved row has settled, but the topic stream is still live. */
  awaiting?: boolean
  createdAt?: string
  children?: ReactNode
}) {
  const items = useMemo(() => projectTaskConversationTurn(parts, streaming), [parts, streaming])
  const views = useMemo(() => clusterConsecutiveTools(items), [items])
  const waiting = turnShowsWorkingStatus(streaming || awaiting, views[views.length - 1])

  return (
    <div className="task-conversation" data-task-conversation-turn="" data-task-turn-state={waiting ? 'live' : 'idle'}>
      {views.map((item) => {
        if (item.kind === 'tools') {
          return (
            <div
              key={item.id}
              className="task-tool-group"
              data-task-tool-count={item.tools.length}
              data-task-tool-group="">
              <ol className="task-tool-group__list">
                {item.tools.map((tool) => {
                  const part = tool.partIndex === undefined ? leakedToolPart(tool.input) : parts[tool.partIndex]
                  return (
                    <li key={tool.id} className="task-tool-group__item">
                      <TaskToolRow name={tool.name} part={part} state={tool.state} />
                    </li>
                  )
                })}
              </ol>
            </div>
          )
        }
        if (item.kind === 'tool') {
          const part = item.partIndex === undefined ? leakedToolPart(item.input) : parts[item.partIndex]
          return <TaskToolRow key={item.id} name={item.name} part={part} state={item.state} />
        }
        if (item.kind === 'note') {
          return (
            <p key={item.id} data-task-error="">
              {item.text}
            </p>
          )
        }
        if (item.kind === 'block') {
          const part = parts[item.index]
          if (!part) return null
          return (
            <TaskTurnBlock
              key={item.id}
              createdAt={createdAt}
              part={part}
              partId={item.id}
              streaming={streaming}
              text={item.text}
            />
          )
        }
        return <TaskProse key={item.id} id={item.id} streaming={item.streaming} text={item.text} />
      })}
      {waiting ? <WorkingStatus /> : null}
      {children}
    </div>
  )
}

/**
 * Assistant turn for the task workspace.
 * Live and restored messages share this row and fence presentation.
 */
function TaskChangedFiles({
  messageId,
  streaming,
  turnActive
}: {
  messageId: string
  streaming: boolean
  turnActive: boolean
}) {
  const { t } = useTranslation()
  const openArtifactFile = useOptionalMessageListActions()?.openArtifactFile
  const parts = useMessageParts(messageId)
  const edits = useMemo(
    () => turnFileEdits(parts, streaming || turnActive),
    [parts, streaming, turnActive]
  )
  const [openPath, setOpenPath] = useState<string | null>(null)
  const open = edits.find((edit) => edit.path === openPath)
  if (edits.length === 0) return null
  return (
    <section aria-label={t('workbench.turn.changed_files')} data-task-changed-files="">
      <header className="task-products-bar">
        <span data-task-products-label="">{t('workbench.turn.changed_files')}</span>
      </header>
      <ul>
        {edits.map((edit) => (
          <li key={edit.path}>
            <button
              type="button"
              className="task-product-row"
              data-task-changed-file=""
              data-task-changed-path={edit.path}
              title={edit.path}
              onClick={() => {
                setOpenPath(edit.path)
                void openArtifactFile?.(edit.path)
              }}>
              <span className="task-product-row__name">{edit.path}</span>
            </button>
          </li>
        ))}
      </ul>
      {open ? (
        <TaskDiffView diff={open.diff} onOpenFile={openArtifactFile ? (path) => void openArtifactFile(path) : undefined} />
      ) : null}
    </section>
  )
}

export function TaskConversationMessage({
  messageId,
  streaming,
  awaiting = false,
  turnActive = false,
  createdAt
}: {
  messageId: string
  streaming: boolean
  awaiting?: boolean
  turnActive?: boolean
  createdAt: string
}) {
  const parts = useMessageParts(messageId)
  const hostProof = useMemo(() => proofFromParts(parts, messageId), [parts, messageId])
  const [localProof, setLocalProof] = useState<{ source: WorkbenchProof | null; value: WorkbenchProof | null }>({
    source: hostProof,
    value: hostProof
  })
  const proof = localProof.source === hostProof ? localProof.value : hostProof
  const setProof = (value: WorkbenchProof) => setLocalProof({ source: hostProof, value })
  const workspace = useOptionalWorkbenchWorkspaceTabs()
  useEffect(() => {
    if (proof) workspace?.syncProof(proof)
  }, [proof, workspace])
  const items = useMemo(() => projectTaskConversationTurn(parts, streaming), [parts, streaming])
  const agentProse = useMemo(
    () =>
      items
        .filter((item): item is Extract<typeof item, { kind: 'prose' }> => item.kind === 'prose')
        .map((item) => item.text)
        .join('\n\n'),
    [items]
  )
  useEffect(() => {
    if (streaming || !agentProse.trim()) return
    noteOfficialWritingAgentProse(agentProse)
    notePresentationAgentProse(agentProse)
  }, [agentProse, streaming])

  return (
    <TaskConversationTurn parts={parts} streaming={streaming} awaiting={awaiting} createdAt={createdAt}>
      {proof ? <TaskAutoReview proof={proof} onChange={setProof} /> : null}
      <TaskWorkspaceProducts messageId={messageId} streaming={streaming} proof={proof} />
      <TaskChangedFiles messageId={messageId} streaming={streaming} turnActive={turnActive} />
    </TaskConversationTurn>
  )
}
