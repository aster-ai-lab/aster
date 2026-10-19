import type { OfficeArtifact } from '@a3s-lab/office/core'
import type * as OfficeReactModule from '@a3s-lab/office/react'
import { EmptyState } from '@a3s-lab/ui'
import type { Editor } from '@tiptap/core'
import { useCallback, useEffect, useRef, useState } from 'react'
import { useTranslation } from 'react-i18next'

import { FileEditorLoading } from '@renderer/pages/documents/FileEditorLoading'
import { readLocalDocumentBytes, subscribeLocalDocumentChanges } from '@renderer/pages/documents/useLocalDocuments'
import type { AbsoluteFilePath } from '@shared/types/file'
import type { ReviewProgress } from '@shared/types/reviewSource'
import { parentDirectoryOfFile } from '@shared/utils/file'

import { reviewComposerDraft } from '../reviewHandoff'
import { fillWorkbenchComposer } from '../workbenchComposerBridge'
import { ReviewAttentionDecorations, setReviewAttention, reviewQuoteRange } from './reviewAttention'
import { useActiveReviewFinding, useReviewFindingFocusNonce, type ReviewFinding } from './reviewFindings'

const REVIEW_EXTENSIONS = [ReviewAttentionDecorations]
const PHASE_LABEL = {
  reviewing: 'workbench.review.progress.reviewing',
  finished: 'workbench.review.progress.finished',
  failed: 'workbench.review.progress.failed',
  cancelled: 'workbench.review.progress.cancelled'
} as const

type OfficeReact = typeof OfficeReactModule

type CommentEditor = {
  isDestroyed: boolean
  state: {
    doc: {
      descendants: (visitor: (node: { isText: boolean; text?: string }, pos: number) => boolean | void) => void
    }
  }
  commands: {
    insertDocumentCommentThread: (
      comment: { id: string; author: string; date: string; text: string; resolved: boolean },
      range: { from: number; to: number }
    ) => boolean
  }
  chain?: () => {
    command?: (command: (props: ReviewMarkCommand) => boolean) => { run: () => boolean }
    focus: () => {
      setTextSelection: (range: { from: number; to: number }) => {
        scrollIntoView: () => { run: () => boolean }
      }
    }
  }
  view?: { dom: ParentNode }
}

type ReviewMarkTransaction = {
  docChanged: boolean
  addMark: (from: number, to: number, mark: unknown) => unknown
  setMeta: (key: string, value: boolean) => void
}

type ReviewMarkCommand = {
  tr: ReviewMarkTransaction
  state: {
    schema: {
      marks: {
        documentComment?: { create: (attrs: { id: string }) => unknown }
      }
    }
  }
  dispatch?: (transaction: ReviewMarkTransaction) => void
}

type ReviewCommentContent = {
  comments?: Array<{ id: string }>
}

/** a3s-office Word editor for a `/review` document, with quotes anchored as comments. */
export function ReviewOfficeDocument({
  filePath,
  fileName,
  findings,
  revision,
  progress,
  phase = 'finished'
}: {
  filePath: AbsoluteFilePath
  fileName: string
  findings: readonly ReviewFinding[]
  revision?: string
  phase?: 'reviewing' | 'finished' | 'failed' | 'cancelled'
  progress?: ReviewProgress
}) {
  const { t } = useTranslation()
  const [placed, setPlaced] = useState(0)
  const [editors, setEditors] = useState<OfficeReact | null>(null)
  const [content, setContent] = useState<OfficeArtifact['content'] | null>(null)
  const [failed, setFailed] = useState<'read' | 'watch' | null>(null)
  const [conflict, setConflict] = useState<'unbound' | 'changed' | null>(null)
  const generation = useRef(0)
  const editorRef = useRef<CommentEditor | null>(null)
  const appliedRef = useRef(new Set<string>())
  const findingsRef = useRef(findings)
  findingsRef.current = findings
  const activeFindingId = useActiveReviewFinding()
  const activeFindingIdRef = useRef(activeFindingId)
  activeFindingIdRef.current = activeFindingId
  const focusNonce = useReviewFindingFocusNonce()
  const attention = phase === 'reviewing' ? (progress?.quote ?? '') : ''
  const attentionRef = useRef(attention)
  attentionRef.current = attention
  useEffect(() => {
    if (editorRef.current) setReviewAttention(editorRef.current as unknown as Editor, attention)
  }, [attention])

  useEffect(() => {
    const watch = new AbortController()
    let reading: AbortController | null = null
    let after = 0
    let reconnect: ReturnType<typeof setTimeout> | undefined
    const withdraw = () => {
      generation.current += 1
      reading?.abort()
      setEditors(null)
      setContent(null)
      setConflict(null)
      editorRef.current = null
      appliedRef.current = new Set()
      setPlaced(0)
    }
    withdraw()
    setFailed(null)
    if (!revision || !/^[a-f0-9]{64}$/u.test(revision)) {
      setConflict('unbound')
      return
    }
    const root = parentDirectoryOfFile(filePath)
    const load = async () => {
      if (watch.signal.aborted) return
      withdraw()
      setFailed(null)
      const current = generation.current
      const request = new AbortController()
      reading = request
      const retired = () => watch.signal.aborted || request.signal.aborted || current !== generation.current
      try {
        const office = await import('@a3s-lab/office/react')
        try {
          await import('@a3s-lab/office/styles.css')
        } catch {
          // The editor still mounts when the stylesheet cannot be fetched.
        }
        if (retired()) return
        const bytes = await readLocalDocumentBytes(
          root,
          filePath,
          undefined,
          32 * 1024 * 1024,
          () => undefined,
          request.signal
        )
        const digest = await crypto.subtle.digest('SHA-256', bytes.slice().buffer)
        const actualRevision = [...new Uint8Array(digest)].map((byte) => byte.toString(16).padStart(2, '0')).join('')
        if (retired()) return
        if (actualRevision !== revision) {
          setConflict('changed')
          return
        }
        const { importOfficeFile } = await import('@a3s-lab/office/core')
        const imported = await importOfficeFile(new File([bytes.slice()], fileName), { signal: request.signal })
        if (retired()) return
        if (imported.content.type !== 'document') throw new Error('kind')
        setEditors(office)
        setContent(imported.content)
      } catch {
        if (!retired()) setFailed('read')
      }
    }
    const connect = () => {
      const disconnected = () => {
        if (watch.signal.aborted) return
        withdraw()
        setFailed('watch')
        reconnect = setTimeout(connect, 2000)
      }
      void subscribeLocalDocumentChanges(
        root,
        after,
        watch.signal,
        (change) => {
          if (watch.signal.aborted) return
          after = change.revision
          if (change.fullScan || change.paths.includes('')) void load()
        },
        () => void load()
      ).then(disconnected, disconnected)
    }
    connect()
    return () => {
      generation.current += 1
      watch.abort()
      reading?.abort()
      clearTimeout(reconnect)
    }
  }, [fileName, filePath, revision])

  const apply = useCallback(
    (editor: CommentEditor) => {
      if (!content || content.type !== 'document' || editor.isDestroyed) return
      const anchored = anchorReviewFindings(editor, findingsRef.current, appliedRef.current)
      if (anchored.length === 0) return
      setPlaced(appliedRef.current.size)
      const stamped = new Date().toISOString()
      setContent((current) => {
        if (!current || current.type !== 'document') return current
        return appendReviewComments(current, anchored, stamped)
      })
    },
    [content]
  )

  const placeActiveFinding = useCallback((editor: CommentEditor) => {
    const id = activeFindingIdRef.current
    if (!id) {
      clearActiveComment(editor)
      return
    }
    const finding = findingsRef.current.find((item) => item.id === id)
    if (finding) focusFinding(editor, finding)
  }, [])

  useEffect(() => {
    const editor = editorRef.current
    if (!editor) return
    apply(editor)
    placeActiveFinding(editor)
  }, [findings, activeFindingId, focusNonce, apply, placeActiveFinding])

  if (conflict === 'unbound' && phase === 'reviewing') {
    return <FileEditorLoading label={t('workbench.review.progress.opening')} />
  }
  if (conflict) {
    return (
      <EmptyState
        title={
          conflict === 'changed'
            ? t('workbench.review.source_changed.title')
            : t('workbench.review.source_unbound.title')
        }
        description={
          conflict === 'changed'
            ? t('workbench.review.source_changed.description')
            : t('workbench.review.source_unbound.description')
        }
        className="h-full"
      />
    )
  }
  if (failed) {
    if (failed === 'watch') {
      return (
        <EmptyState
          title={t('workbench.review.source_unavailable.title')}
          description={t('workbench.review.source_unavailable.description')}
          className="h-full"
        />
      )
    }
    return <p role="status">{t('file_preview.unavailable.title')}</p>
  }
  if (!editors || !content || content.type !== 'document') {
    return (
      <FileEditorLoading
        label={t(phase === 'reviewing' ? 'workbench.review.progress.opening' : 'file_preview.loading')}
      />
    )
  }

  const renderedGeneration = generation.current
  return (
    <div className="flex h-full min-h-0 flex-1 flex-col overflow-hidden" data-review-office-document="">
      <div
        role="status"
        data-review-outcome={phase}
        className="flex flex-wrap items-center gap-2 border-b px-3 py-2 text-[13px]">
        <strong>{fileName}</strong>
        {progress ? (
          <span>{t('workbench.review.progress.batch', { index: progress.index, total: progress.total })}</span>
        ) : null}
        <span>{t(PHASE_LABEL[phase])}</span>
        <span>{t('workbench.review.progress.placed', { count: placed })}</span>
        {phase !== 'reviewing' ? (
          <button
            type="button"
            className="ml-auto rounded border px-2 py-1"
            onClick={() => fillWorkbenchComposer(reviewComposerDraft(t('workbench.review.file'), filePath, fileName))}>
            {t('workbench.review.progress.again')}
          </button>
        ) : null}
        {findings.length > placed ? (
          <span>{t('workbench.review.progress.unplaced', { count: findings.length - placed })}</span>
        ) : null}
        {phase === 'finished' && findings.length === 0 ? <span>{t('workbench.review.progress.empty')}</span> : null}
      </div>
      <editors.DocumentEditor
        key={renderedGeneration}
        preview
        extensions={REVIEW_EXTENSIONS}
        className="h-full min-h-0"
        content={content}
        defaultRibbonCollapsed
        defaultCommentsOpen={placed > 0}
        onChange={(next) => {
          if (renderedGeneration === generation.current) setContent(next)
        }}
        onEditorReady={(editor) => {
          if (renderedGeneration !== generation.current) return
          const live = editor as unknown as CommentEditor
          editorRef.current = live
          apply(live)
          setReviewAttention(live as unknown as Editor, attentionRef.current)
          placeActiveFinding(live)
        }}
      />
    </div>
  )
}

function commentFor(finding: ReviewFinding, date: string) {
  return {
    id: finding.id,
    author: '合同审查',
    date,
    resolved: false,
    text: [
      `【${finding.title}】`,
      finding.summary,
      `证据：${finding.evidenceQuote}`,
      finding.guidance ? `建议：${finding.guidance}` : ''
    ]
      .filter((line) => line.trim())
      .join('\n')
  }
}

/** Anchor every new unique quote in one transaction, without replacing the document HTML. */
export function anchorReviewFindings(
  editor: CommentEditor,
  findings: readonly ReviewFinding[],
  applied: Set<string>
): ReviewFinding[] {
  if (editor.isDestroyed) return []
  const pending: Array<{ finding: ReviewFinding; range: { from: number; to: number } }> = []
  const seen = new Set(applied)
  for (const finding of findings) {
    if (seen.has(finding.id)) continue
    const range = reviewQuoteRange(editor.state.doc, finding.evidenceQuote)
    if (!range) continue
    seen.add(finding.id)
    pending.push({ finding, range })
  }
  if (pending.length === 0) return []
  const command = editor.chain?.().command
  if (!command) {
    const anchored: ReviewFinding[] = []
    for (const item of pending) {
      const comment = commentFor(item.finding, new Date().toISOString())
      if (!editor.commands.insertDocumentCommentThread(comment, item.range)) continue
      applied.add(item.finding.id)
      anchored.push(item.finding)
    }
    return anchored
  }
  const accepted = command(({ tr, state, dispatch }) => {
    const mark = state.schema.marks.documentComment
    if (!mark || !dispatch) {
      tr.setMeta('preventDispatch', true)
      return false
    }
    for (const item of pending) {
      tr.addMark(item.range.from, item.range.to, mark.create({ id: item.finding.id }))
    }
    if (!tr.docChanged) {
      tr.setMeta('preventDispatch', true)
      return false
    }
    tr.setMeta('preventUpdate', true)
    dispatch(tr)
    return true
  }).run()
  if (!accepted) return []
  for (const item of pending) applied.add(item.finding.id)
  return pending.map((item) => item.finding)
}

/** Keep the imported document bytes. Comments are a sidecar, not a new HTML snapshot. */
export function appendReviewComments<T extends ReviewCommentContent>(
  content: T,
  findings: readonly ReviewFinding[],
  date: string
): T {
  const existing = content.comments ?? []
  const ids = new Set(existing.map((comment) => comment.id))
  const added = findings.filter((finding) => !ids.has(finding.id)).map((finding) => commentFor(finding, date))
  if (added.length === 0) return content
  return { ...content, comments: [...existing, ...added] }
}

function focusFinding(editor: CommentEditor, finding: ReviewFinding) {
  const range = reviewQuoteRange(editor.state.doc, finding.evidenceQuote)
  if (range) {
    editor.chain?.().focus().setTextSelection(range).scrollIntoView().run()
  }
  const root = editor.view?.dom
  if (!(root instanceof Element)) return
  clearActiveComment(editor)
  const mark = root.querySelector(`[data-comment-id="${cssEscape(finding.id)}"]`)
  if (!(mark instanceof HTMLElement)) return
  mark.classList.add('is-active-comment')
  scrollQuoteIntoDocument(mark)
  window.requestAnimationFrame(() => scrollQuoteIntoDocument(mark))
}

function scrollQuoteIntoDocument(mark: HTMLElement) {
  const scroller = mark.closest('.work-document-scroll')
  if (!(scroller instanceof HTMLElement)) return
  const rects = [...mark.getClientRects()].filter((rect) => rect.width >= 8 && rect.height >= 8 && rect.height <= 40)
  const rect = rects.reduce<(typeof rects)[number] | null>(
    (top, item) => (!top || item.top < top.top ? item : top),
    null
  )
  if (!rect) return
  const frame = scroller.getBoundingClientRect()
  const top = documentScrollTopForQuote(scroller.scrollTop, frame.top, frame.height, rect.top, rect.height)
  scroller.scrollTo({ top, behavior: 'smooth' })
}

export function documentScrollTopForQuote(
  scrollerTop: number,
  frameTop: number,
  frameHeight: number,
  markTop: number,
  markHeight: number
): number {
  return Math.max(0, scrollerTop + markTop - frameTop - (frameHeight - markHeight) / 2)
}

function clearActiveComment(editor: CommentEditor) {
  editor.view?.dom.querySelectorAll('.is-active-comment').forEach((mark) => {
    mark.classList.remove('is-active-comment')
  })
}

function cssEscape(value: string): string {
  return typeof CSS !== 'undefined' && typeof CSS.escape === 'function'
    ? CSS.escape(value)
    : value.replace(/["\\]/g, '\\$&')
}
