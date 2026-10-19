import { ChevronDown, Loader2, MessageSquare } from 'lucide-react'
import { useEffect, useId, useLayoutEffect, useRef, useState } from 'react'
import { createPortal } from 'react-dom'
import { useTranslation } from 'react-i18next'

import { A3SBotAvatar } from '@renderer/components/bot/A3SBotAvatar'
import { useRefresh } from '@renderer/components/chat/messages/blocks/MessagePartsContext'
import { useOptionalRightPanelActions } from '@renderer/components/chat/panes/Shell'
import { actOnWorkbenchProof, loadWorkbenchProofs } from '@renderer/data/studioAgentPlane'
import type { WorkbenchProof, WorkbenchProofAction } from '@shared/types/workbenchProof'

import { useOptionalWorkbenchWorkspaceTabs } from '../workbenchWorkspaceTabs'

/** Exact quoted spans only. Ambiguous or markdown-transformed quotes stay in the finding card. */
export function quotedRange(root: Element, quote: string): Range | null {
  const walker = document.createTreeWalker(root, NodeFilter.SHOW_TEXT)
  const nodes: Text[] = []
  let full = ''
  while (walker.nextNode()) {
    const node = walker.currentNode as Text
    nodes.push(node)
    full += node.data
  }
  const start = full.indexOf(quote)
  if (!quote || start < 0 || full.indexOf(quote, start + 1) >= 0) return null
  let offset = 0
  const range = document.createRange()
  let begun = false
  for (const node of nodes) {
    const end = offset + node.length
    if (!begun && start < end) {
      range.setStart(node, start - offset)
      begun = true
    }
    if (begun && start + quote.length <= end) {
      range.setEnd(node, start + quote.length - offset)
      return range
    }
    offset = end
  }
  return null
}

function quoteTargets(turn: Element, quote: string) {
  return Array.from(turn.querySelectorAll<HTMLElement>('[data-task-prose]'))
    .filter((node) => !node.querySelector('[data-task-prose]'))
    .flatMap((node) => {
      const range = quotedRange(node, quote)
      return range ? [{ node, range }] : []
    })
}

export function TaskAutoReview({
  proof,
  onChange
}: {
  proof: WorkbenchProof
  onChange: (proof: WorkbenchProof) => void
}) {
  const { t } = useTranslation()
  const refresh = useRefresh()
  const panel = useOptionalRightPanelActions()
  const workspace = useOptionalWorkbenchWorkspaceTabs()
  const ref = useRef<HTMLElement>(null)
  const anchor = useRef<HTMLSpanElement>(null)
  const highlightId = `review-${useId().replace(/:/g, '')}`
  const [expanded, setExpanded] = useState(true)
  const [busy, setBusy] = useState(false)
  const [error, setError] = useState('')
  const [now, setNow] = useState(Date.now())
  const [turn, setTurn] = useState<HTMLElement | null>(null)
  const [focused, setFocused] = useState<string | null>(null)
  const [rail, setRail] = useState<{
    width: number
    span: number
    top: number
    left: number
    height: number
    side: boolean
    shown: boolean
    paths: { id: string; path: string; x: number; y: number }[]
  } | null>(null)
  const { review } = proof
  const active = review.status === 'running' || review.status === 'remediating'
  const visible =
    proof.policy.reviewMode &&
    review.status !== 'pending' &&
    review.status !== 'off' &&
    (active || review.status === 'failed' || review.status === 'cancelled' || review.findings.length > 0)

  useLayoutEffect(() => {
    const turn = anchor.current?.closest('[data-task-conversation-turn]')
    setTurn(turn instanceof HTMLElement ? turn : null)
    if (active) turn?.setAttribute('data-review-scanning', '')
    else turn?.removeAttribute('data-review-scanning')
    return () => {
      turn?.removeAttribute('data-review-scanning')
    }
  }, [active, proof.runId, visible])

  useLayoutEffect(() => {
    if (!turn || !visible) return
    let frame = 0
    const measure = () => {
      const section = ref.current
      if (!section) return
      const bounds = turn.getBoundingClientRect()
      if (bounds.width === 0) {
        setRail(null)
        return
      }
      const viewport = turn.closest('.messages-container')?.getBoundingClientRect() ?? {
        top: 0,
        left: 0,
        right: innerWidth,
        bottom: innerHeight
      }
      const available = viewport.right - bounds.right - 28
      const side = available >= 180
      const width = Math.min(260, side ? available : viewport.right - viewport.left - 32)
      const left = side ? bounds.right + 16 : viewport.right - width - 16
      const height = Math.max(100, Math.min(viewport.bottom, innerHeight) - Math.max(0, viewport.top) - 16)
      const anchors = review.findings.flatMap((finding) => {
        if (finding.status === 'waived') return []
        const targets = quoteTargets(turn, finding.evidenceQuote)
        if (targets.length !== 1) return []
        const rects = targets[0].range.getClientRects()
        const rect = rects[rects.length - 1]
        return rect ? [{ id: finding.id, rect }] : []
      })
      const targetTop = anchors[0]?.rect.top ?? bounds.bottom
      const top = Math.max(
        Math.max(0, viewport.top) + 8,
        Math.min(
          targetTop,
          Math.min(viewport.bottom, innerHeight) - Math.min(section.getBoundingClientRect().height, height) - 8
        )
      )
      const paths = anchors.flatMap(({ id, rect }) => {
        if (!side) return []
        const card = Array.from(section.querySelectorAll<HTMLElement>('[data-task-review-finding]')).find(
          (node) => node.dataset.taskReviewFinding === id
        )
        if (!card) return []
        // Keep connectors in the gutter so they never cross the rest of a prose line.
        const x = bounds.width + 2
        const y = rect.top + rect.height / 2 - top
        const end = card.getBoundingClientRect().top - top + 22
        const edge = bounds.width + 16
        return [{ id, x, y, path: `M ${x + 4} ${y} C ${edge - 12} ${y}, ${edge - 12} ${end}, ${edge} ${end}` }]
      })
      const shown = bounds.bottom > viewport.top && bounds.top < viewport.bottom
      const next = { width, span: left - bounds.left, top, left, height, side, shown, paths }
      setRail((previous) => (JSON.stringify(previous) === JSON.stringify(next) ? previous : next))
    }
    const schedule = () => {
      cancelAnimationFrame(frame)
      frame = requestAnimationFrame(measure)
    }
    const resize = typeof ResizeObserver === 'undefined' ? null : new ResizeObserver(schedule)
    resize?.observe(turn)
    if (ref.current) resize?.observe(ref.current)
    const viewport = turn.closest('.messages-container')
    if (viewport) resize?.observe(viewport)
    const mutations = new MutationObserver((records) => {
      if (records.some((record) => !ref.current?.contains(record.target))) schedule()
    })
    mutations.observe(turn, { childList: true, subtree: true, characterData: true })
    window.addEventListener('resize', schedule)
    document.addEventListener('scroll', schedule, true)
    // Background WebViews can suspend animation frames before the first observation.
    measure()
    return () => {
      cancelAnimationFrame(frame)
      resize?.disconnect()
      mutations.disconnect()
      window.removeEventListener('resize', schedule)
      document.removeEventListener('scroll', schedule, true)
    }
  }, [turn, visible, expanded, review.findings, rail?.left, rail?.top, rail?.width])

  useEffect(() => {
    if (!active) return
    const timer = window.setInterval(() => setNow(Date.now()), 1000)
    return () => window.clearInterval(timer)
  }, [active])

  useEffect(() => {
    const registry = globalThis.CSS?.highlights
    if (!registry || typeof Highlight === 'undefined') return
    if (!turn) return
    const names = ['high', 'warning', 'info', 'addressed', 'accepted'] as const
    const paint = () => {
      for (const name of names) {
        const ranges = review.findings
          .filter((finding) => (finding.status === 'pending' ? finding.severity : finding.status) === name)
          .flatMap((finding) => {
            const matches = quoteTargets(turn, finding.evidenceQuote)
            return matches.length === 1 ? [matches[0].range] : []
          })
        const highlight = new Highlight(...ranges)
        highlight.priority = name === 'high' ? 3 : name === 'warning' ? 2 : 1
        registry.set(`${highlightId}-${name}`, highlight)
      }
      const selected = review.findings.find((finding) => finding.id === focused && finding.status !== 'waived')
      const targets = selected ? quoteTargets(turn, selected.evidenceQuote) : []
      const focus = new Highlight(...(targets.length === 1 ? [targets[0].range] : []))
      focus.priority = 4
      registry.set(`${highlightId}-focus`, focus)
    }
    paint()
    const observer = new MutationObserver((records) => {
      if (records.some((record) => !ref.current?.contains(record.target))) paint()
    })
    observer.observe(turn, { childList: true, subtree: true, characterData: true })
    const hover = (event: PointerEvent) => {
      if (ref.current?.contains(event.target as Node)) return
      if (ref.current?.contains(document.activeElement)) return
      const finding = review.findings.find(
        (item) =>
          item.status !== 'waived' &&
          quoteTargets(turn, item.evidenceQuote).some(({ range }) =>
            Array.from(range.getClientRects()).some(
              (rect) =>
                event.clientX >= rect.left &&
                event.clientX <= rect.right &&
                event.clientY >= rect.top &&
                event.clientY <= rect.bottom
            )
          )
      )
      setFocused(finding?.id ?? null)
    }
    turn.addEventListener('pointermove', hover)
    return () => {
      turn.removeEventListener('pointermove', hover)
      observer.disconnect()
      for (const name of names) registry.delete(`${highlightId}-${name}`)
      registry.delete(`${highlightId}-focus`)
    }
  }, [highlightId, review.findings, turn, focused])

  if (!visible) return null
  const act = async (action: WorkbenchProofAction) => {
    if (busy) return
    setBusy(true)
    setError('')
    try {
      onChange(await actOnWorkbenchProof(proof.sessionId, proof.messageId, action))
      refresh()
    } catch (cause) {
      setError(cause instanceof Error ? cause.message : t('workbench.proof.action_failed'))
    } finally {
      setBusy(false)
    }
  }
  const elapsed = review.startedAt ? Math.max(0, Math.floor((now - Date.parse(review.startedAt)) / 1000)) : 0
  const content = (
    <div
      data-task-review-floating=""
      style={{
        position: 'fixed',
        zIndex: 30,
        pointerEvents: 'none',
        left: rail?.left ?? 0,
        top: rail?.top ?? 0,
        width: rail?.width ?? 260,
        visibility: turn && !rail?.shown ? 'hidden' : undefined
      }}>
      {rail?.side && expanded ? (
        <svg className="task-review-connectors" aria-hidden="true" style={{ left: -rail.span, width: rail.span }}>
          {rail.paths.map(({ id, path }) => (
            <path key={id} d={path} data-task-review-connector={id} />
          ))}
        </svg>
      ) : null}
      <section
        ref={ref}
        aria-label={t('workbench.composer.review')}
        data-task-auto-review=""
        className="task-review-overlay"
        data-task-review-layout={rail?.side ? 'rail' : 'overlay'}
        style={{ maxHeight: rail?.height, overflowY: 'auto', pointerEvents: 'auto' }}
        data-task-review-scan={active ? '' : undefined}
        data-task-auto-review-state={review.status}
        data-task-auto-review-count={review.findings.length}>
        <style>{`::highlight(${highlightId}-high) { background: transparent; text-decoration: underline dashed #dc2626; }
        ::highlight(${highlightId}-warning) { background: transparent; text-decoration: underline dashed #b07813; }
        ::highlight(${highlightId}-info), ::highlight(${highlightId}-addressed) { background: transparent; text-decoration: underline dashed #3867d6; }
        ::highlight(${highlightId}-accepted) { background: transparent; text-decoration: underline dashed #16a34a; }
        ::highlight(${highlightId}-focus) { background: #ffedb3; }`}</style>
        <header>
          <button type="button" aria-expanded={expanded} onClick={() => setExpanded((value) => !value)}>
            {active ? (
              <Loader2 size={14} className="animate-spin motion-reduce:animate-none" aria-hidden="true" />
            ) : (
              <ChevronDown size={14} aria-hidden="true" />
            )}
            <A3SBotAvatar size={20} color="#c62828" animated={active} activity={active ? 'working' : 'idle'} />
            <strong>{t('workbench.composer.review')}</strong>
            <span role="status">
              {
                {
                  off: '',
                  pending: '',
                  running: t('workbench.proof.review_running'),
                  remediating: t('workbench.proof.review_remediating'),
                  open_queue: t('workbench.proof.review_open_queue'),
                  completed: t('workbench.proof.review_completed'),
                  failed: t('workbench.proof.review_failed'),
                  cancelled: t('workbench.proof.review_cancelled')
                }[review.status]
              }
              {active ? ` · ${elapsed}s` : ` · ${review.findings.length}`}
            </span>
          </button>
          {active ? (
            <button type="button" disabled={busy} onClick={() => void act({ action: 'cancel' })}>
              {t('common.cancel')}
            </button>
          ) : null}
          {review.status === 'failed' && proof.producerStatus === 'completed' ? (
            <button type="button" disabled={busy} onClick={() => void act({ action: 'retry' })}>
              {t('workbench.proof.retry')}
            </button>
          ) : null}
        </header>
        {review.error ? <p role="alert">{review.error}</p> : null}
        {review.status === 'failed' && proof.producerStatus !== 'completed' ? (
          <p>{t('workbench.proof.rerun_required')}</p>
        ) : null}
        {error ? <p role="alert">{error}</p> : null}
        {expanded ? (
          <ul>
            {(['high', 'warning', 'info'] as const).flatMap((severity) =>
              review.findings
                .filter((finding) => finding.severity === severity)
                .map((finding) => (
                  <li
                    key={finding.id}
                    tabIndex={0}
                    onPointerEnter={() => setFocused(finding.id)}
                    onPointerLeave={(event) => {
                      if (!event.currentTarget.contains(document.activeElement)) setFocused(null)
                    }}
                    onFocus={() => setFocused(finding.id)}
                    onBlur={(event) => {
                      if (!event.currentTarget.contains(event.relatedTarget)) setFocused(null)
                    }}
                    data-finding-focused={focused === finding.id ? '' : undefined}
                    data-task-review-finding={finding.id}
                    data-finding-status={finding.status}
                    data-finding-severity={severity}>
                    <strong>
                      <MessageSquare size={13} aria-hidden="true" />
                      {finding.claim}
                    </strong>
                    <span data-finding-severity-label="">
                      {
                        {
                          high: t('workbench.proof.severity_high'),
                          warning: t('workbench.proof.severity_warning'),
                          info: t('workbench.proof.severity_info')
                        }[severity]
                      }
                    </span>
                    <span>
                      {
                        {
                          pending: t('workbench.proof.finding_pending'),
                          addressed: t('workbench.proof.finding_addressed'),
                          accepted: t('workbench.proof.finding_accepted'),
                          waived: t('workbench.proof.finding_waived')
                        }[finding.status]
                      }
                    </span>
                    <button
                      className="task-review-quote"
                      type="button"
                      onClick={() => {
                        const targets = turn ? quoteTargets(turn, finding.evidenceQuote) : []
                        if (targets.length !== 1) {
                          setError(t('workbench.proof.quote_not_visible'))
                          return
                        }
                        setError('')
                        targets[0].node.scrollIntoView({
                          block: 'center',
                          behavior: matchMedia('(prefers-reduced-motion: reduce)').matches ? 'instant' : 'smooth'
                        })
                      }}>
                      “{finding.evidenceQuote}”
                    </button>
                    <p>{finding.guidance}</p>
                    {finding.reply ? <p>{finding.reply}</p> : null}
                    {finding.status === 'addressed' ? (
                      <button
                        type="button"
                        disabled={busy}
                        onClick={() => void act({ action: 'accept', findingId: finding.id })}>
                        {t('workbench.proof.accept')}
                      </button>
                    ) : null}
                    {finding.status === 'pending' && !active ? (
                      <button
                        type="button"
                        disabled={busy}
                        onClick={() => void act({ action: 'ignore', findingId: finding.id })}>
                        {t('workbench.proof.ignore')}
                      </button>
                    ) : null}
                    {proof.policy.artifactTrace && finding.receiptDigest ? (
                      <button
                        type="button"
                        disabled={busy || !workspace}
                        onClick={() => {
                          setBusy(true)
                          setError('')
                          void loadWorkbenchProofs(proof.sessionId)
                            .then((proofs) => {
                              const owner = [proof, ...proofs].find(
                                (item) =>
                                  item.policy.artifactTrace &&
                                  item.artifacts.some((version) => version.receiptDigest === finding.receiptDigest)
                              )
                              const artifact = owner?.artifacts.find(
                                (version) => version.receiptDigest === finding.receiptDigest
                              )
                              if (!owner || !artifact || !workspace)
                                throw new Error(t('workbench.proof.snapshot_failed'))
                              workspace.openLineage(owner, artifact.id, true)
                              panel?.tryOpen('files', { userInitiated: true })
                            })
                            .catch((cause) =>
                              setError(cause instanceof Error ? cause.message : t('workbench.proof.action_failed'))
                            )
                            .finally(() => setBusy(false))
                        }}>
                        {t('workbench.lineage.title')}
                      </button>
                    ) : null}
                  </li>
                ))
            )}
          </ul>
        ) : null}
      </section>
    </div>
  )
  return (
    <>
      <span ref={anchor} hidden aria-hidden="true" />
      {createPortal(content, document.body)}
    </>
  )
}
