import { Check, Copy, FileCode } from 'lucide-react'
import { useEffect, useId, useMemo, useState, type KeyboardEvent } from 'react'
import { useTranslation } from 'react-i18next'

import { pairLineSpans, taskDiffPatchText, type DiffToken, type TaskCodeDiff, type TaskDiffLine } from './taskDiff'

interface SplitRow {
  kind: 'context' | 'change' | 'hunk'
  left?: TaskDiffLine
  right?: TaskDiffLine
  leftSpans: DiffToken[]
  rightSpans: DiffToken[]
}

type DiffView = 'unified' | 'split'

let rememberedView: DiffView = 'unified'

/** Editor diff card shared by the expanded tool row and the hover preview. */
export function TaskDiffView({
  diff,
  failed,
  onOpenFile
}: {
  diff: TaskCodeDiff
  failed?: boolean
  onOpenFile?: (path: string) => void
}) {
  const { t } = useTranslation()
  const labelId = useId()
  const [mode, setMode] = useState<DiffView>(rememberedView)
  const [copied, setCopied] = useState(false)
  const path = diff.path?.trim()
  const title = path ? path.replace(/\\/g, '/').split('/').pop() || path : t('workbench.diff.untitled')
  const directory = path && path !== title ? path : ''
  const status = failed
    ? t('workbench.tool.failed')
    : diff.operation === 'delete'
      ? t('workbench.diff.deleted')
      : diff.lines.length === 0
        ? t('workbench.tool.diff_pending')
        : t('workbench.diff.applied')
  const showSplit = diff.comparisonComplete && diff.additions + diff.deletions > 0
  const view: DiffView = showSplit ? mode : 'unified'
  const spans = useMemo(() => pairLineSpans(diff.lines), [diff.lines])
  const rows = useMemo(() => (view === 'split' ? splitRows(diff.lines, spans) : []), [diff.lines, spans, view])

  useEffect(() => {
    if (!copied) return undefined
    const timer = window.setTimeout(() => setCopied(false), 1600)
    return () => window.clearTimeout(timer)
  }, [copied])

  function choose(next: DiffView) {
    rememberedView = next
    setMode(next)
  }

  function onViewKey(event: KeyboardEvent<HTMLDivElement>) {
    if (event.key !== 'ArrowLeft' && event.key !== 'ArrowRight') return
    event.preventDefault()
    const next: DiffView = event.key === 'ArrowRight' ? 'split' : 'unified'
    choose(next)
    const buttons = event.currentTarget.querySelectorAll('button')
    buttons[next === 'split' ? 1 : 0]?.focus()
  }

  async function copyDiff() {
    try {
      await navigator.clipboard.writeText(taskDiffPatchText(diff))
      setCopied(true)
    } catch {
      setCopied(false)
    }
  }

  return (
    <section
      aria-label={`${t('workbench.diff.change')} · ${title}`}
      className="task-code-diff"
      data-code-diff-mode={view}
      data-code-diff-operation={diff.operation}
      data-code-diff-preview="true"
      data-code-diff-status={failed ? 'error' : diff.lines.length === 0 ? 'pending' : 'applied'}>
      <header className="task-code-diff__header">
        <span aria-hidden="true" className="task-code-diff__icon">
          <FileCode size={16} strokeWidth={1.7} />
        </span>
        <span className="task-code-diff__title">
          {onOpenFile && path ? (
            <button
              className="task-code-diff__open-file"
              data-code-diff-open-file="true"
              title={t('workbench.diff.open', { path })}
              type="button"
              onClick={() => onOpenFile(path)}>
              <strong id={labelId}>{title}</strong>
              {directory ? <small>{directory}</small> : null}
            </button>
          ) : (
            <>
              <strong id={labelId} title={path}>
                {title}
              </strong>
              {directory ? <small title={path}>{directory}</small> : null}
            </>
          )}
        </span>
        <span className="task-code-diff__meta">
          {diff.lines.length > 0 ? (
            <span data-code-diff-stats="">
              {!diff.comparisonComplete ? <span data-code-diff-partial="">{t('workbench.tool.preview')}</span> : null}
              <span data-code-diff-additions="">+{diff.additions}</span>
              <span data-code-diff-deletions="">−{diff.deletions}</span>
            </span>
          ) : null}
          <small data-code-diff-status="">{status}</small>
        </span>
        <span className="task-code-diff__actions">
          {showSplit ? (
            <span
              aria-label={t('workbench.diff.view')}
              className="task-code-diff__segments"
              role="radiogroup"
              onKeyDown={onViewKey}>
              <button aria-pressed={view === 'unified'} type="button" onClick={() => choose('unified')}>
                {t('workbench.diff.unified')}
              </button>
              <button aria-pressed={view === 'split'} type="button" onClick={() => choose('split')}>
                {t('workbench.diff.split')}
              </button>
            </span>
          ) : null}
          {diff.lines.length > 0 ? (
            <button
              aria-label={copied ? t('workbench.diff.copied') : t('workbench.diff.copy')}
              data-code-diff-copied={copied ? 'true' : undefined}
              data-code-diff-copy=""
              title={copied ? t('workbench.diff.copied') : t('workbench.diff.copy')}
              type="button"
              onClick={() => void copyDiff()}>
              {copied ? <Check size={14} strokeWidth={1.7} /> : <Copy size={14} strokeWidth={1.7} />}
            </button>
          ) : null}
        </span>
      </header>
      {diff.lines.length > 0 ? (
        <div className={view === 'split' ? 'task-code-diff__body task-code-diff__body--split' : 'task-code-diff__body'}>
          {view === 'split' ? (
            <>
              <div className="task-code-diff__split-head">
                <span>{t('workbench.diff.before')}</span>
                <span>{t('workbench.diff.after')}</span>
              </div>
              {rows.map((row, index) =>
                row.kind === 'hunk' ? (
                  <div key={index} className="task-code-diff__split-hunk">
                    <code>{row.left?.text}</code>
                  </div>
                ) : (
                  <div key={index} className="task-code-diff__split-row">
                    <SplitCell line={row.left} side="old" spans={row.leftSpans} />
                    <SplitCell line={row.right} side="new" spans={row.rightSpans} />
                  </div>
                )
              )}
            </>
          ) : (
            diff.lines.map((line, index) => (
              <div key={index} className="task-code-diff__line" data-kind={line.kind}>
                {line.kind === 'hunk' ? null : (
                  <>
                    <span aria-hidden="true" data-code-diff-gutter="old">
                      {line.kind === 'addition' ? '' : (line.oldLine ?? '')}
                    </span>
                    <span aria-hidden="true" data-code-diff-gutter="new">
                      {line.kind === 'deletion' ? '' : (line.newLine ?? '')}
                    </span>
                    <span aria-hidden="true" data-code-diff-mark="">
                      {line.kind === 'addition' ? '+' : line.kind === 'deletion' ? '−' : ' '}
                    </span>
                  </>
                )}
                <DiffCode spans={spans[index] ?? []} text={line.text} />
              </div>
            ))
          )}
        </div>
      ) : null}
      {diff.truncated ? <p className="task-code-diff__note">{t('workbench.diff.truncated')}</p> : null}
    </section>
  )
}

function DiffCode({ text, spans }: { text: string; spans: readonly DiffToken[] }) {
  if (spans.length === 0) return <code>{text || ' '}</code>
  return (
    <code>
      {spans.map((span, index) =>
        span.changed ? (
          <mark key={index} data-code-diff-token="">
            {span.text}
          </mark>
        ) : (
          <span key={index}>{span.text}</span>
        )
      )}
    </code>
  )
}

function SplitCell({ line, side, spans }: { line?: TaskDiffLine; side: 'old' | 'new'; spans: readonly DiffToken[] }) {
  const gutter = !line ? '' : side === 'old' ? (line.oldLine ?? '') : (line.newLine ?? '')
  return (
    <div className="task-code-diff__split-cell" data-code-diff-side={side} data-kind={line?.kind ?? 'empty'}>
      <span aria-hidden="true" data-code-diff-gutter={side}>
        {gutter}
      </span>
      <DiffCode spans={line ? spans : []} text={line?.text ?? ''} />
    </div>
  )
}

function splitRows(lines: readonly TaskDiffLine[], spans: readonly DiffToken[][]): SplitRow[] {
  const result: SplitRow[] = []
  for (let index = 0; index < lines.length; index += 1) {
    const line = lines[index]
    if (!line) continue
    if (line.kind === 'hunk') {
      result.push({ kind: 'hunk', left: line, right: line, leftSpans: [], rightSpans: [] })
      continue
    }
    if (line.kind === 'context') {
      result.push({ kind: 'context', left: line, right: line, leftSpans: [], rightSpans: [] })
      continue
    }
    const deletions: TaskDiffLine[] = []
    const deletionSpans: DiffToken[][] = []
    const additions: TaskDiffLine[] = []
    const additionSpans: DiffToken[][] = []
    while (lines[index]?.kind === 'deletion') {
      deletions.push(lines[index] as TaskDiffLine)
      deletionSpans.push(spans[index] ?? [])
      index += 1
    }
    while (lines[index]?.kind === 'addition') {
      additions.push(lines[index] as TaskDiffLine)
      additionSpans.push(spans[index] ?? [])
      index += 1
    }
    const count = Math.max(deletions.length, additions.length)
    for (let offset = 0; offset < count; offset += 1) {
      result.push({
        kind: 'change',
        left: deletions[offset],
        right: additions[offset],
        leftSpans: deletionSpans[offset] ?? [],
        rightSpans: additionSpans[offset] ?? []
      })
    }
    index -= 1
  }
  return result
}
