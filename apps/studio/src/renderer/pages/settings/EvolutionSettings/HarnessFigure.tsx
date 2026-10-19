import {
  CircleCheck,
  CircleDashed,
  CircleOff,
  FileText,
  FoldVertical,
  Gauge,
  MessageCircle,
  Pause,
  Play,
  ShieldCheck,
  SkipForward,
  Wrench,
  X
} from 'lucide-react'
import { Fragment, useCallback, useEffect, useMemo, useRef, useState, type ComponentType } from 'react'
import { useTranslation } from 'react-i18next'

import { useRestingDialog } from '@renderer/components/restingDialog'

import { harnessSectionKey, type HarnessExecutionStep } from './harnessExecutionFlow'
import {
  BOARD_H,
  BOARD_W,
  FIGURE_BOXES,
  FIGURE_CAPTIONS,
  FIGURE_FRAMES,
  FIGURE_STORIES,
  TURN_BOX,
  figureEdges,
  figureStep,
  type FigureStory
} from './harnessFigureLayout'
import { HarnessInternals } from './HarnessInternals'
import { componentMounted, componentOrder, type HarnessSnapshot } from './harnessSnapshots'

import './harnessFigure.css'

const STORY_ICON: Record<string, ComponentType<{ size?: number; strokeWidth?: number; 'aria-hidden'?: boolean }>> = {
  text: FileText,
  compact: FoldVertical,
  question: MessageCircle,
  tool: Wrench,
  confirm: ShieldCheck,
  deny: CircleOff,
  budget: Gauge,
  gate: CircleCheck,
  open: CircleDashed
}

const BEAT_MS = 1700

function prefersMotion(): boolean {
  if (typeof window.matchMedia !== 'function') return false
  return !window.matchMedia('(prefers-reduced-motion: reduce)').matches
}

/** Meta Harness as a stepped architecture figure: frames, stores, and a moving packet. */
export function HarnessFigure({
  onSelect,
  snapshot
}: {
  onSelect: (id: string | undefined) => void
  snapshot: HarnessSnapshot
}) {
  const { t } = useTranslation()
  const figureRef = useRef<HTMLDivElement>(null)
  const stageRef = useRef<HTMLDivElement>(null)
  const [scale, setScale] = useState(1)
  const [storyId, setStoryId] = useState(FIGURE_STORIES[0].id)
  const [beatIndex, setBeatIndex] = useState(0)
  const [playing, setPlaying] = useState(prefersMotion)
  const [detailId, setDetailId] = useState<string>()
  const detailHold = useRef<string | undefined>(undefined)
  const detailPresence = useRestingDialog(Boolean(detailId))
  const [hover, setHover] = useState<{ id: string; left: number; top: number }>()
  const edges = useMemo(() => figureEdges(), [])
  const story = FIGURE_STORIES.find((item) => item.id === storyId) ?? FIGURE_STORIES[0]
  const beat = story.beats[beatIndex] ?? story.beats[0]
  const lit = new Set(beat.light)
  const activeEdge = beat.edgeId
  const focusId = beat.light[1] ?? beat.light[0] ?? ''
  const focus = figureStep(focusId)
  const filled = useMemo(() => {
    const seen: string[] = []
    for (const item of story.beats.slice(0, beatIndex + 1)) {
      const id = item.light[1] ?? item.light[0]
      if (!id || seen.includes(id)) continue
      seen.push(id)
    }
    return seen.flatMap((id) => {
      const step = figureStep(id)
      return step ? [step] : []
    })
  }, [beatIndex, story.beats])

  useEffect(() => {
    const stage = stageRef.current
    if (!stage) return
    const apply = () => {
      const width = stage.clientWidth
      if (width <= 0) return
      const fit = width / BOARD_W
      setScale(fit > 1 ? 1 : Math.max(fit, 0.78))
    }
    apply()
    if (typeof ResizeObserver === 'undefined') return
    const observer = new ResizeObserver(apply)
    observer.observe(stage)
    return () => observer.disconnect()
  }, [])

  const advance = useCallback(() => {
    if (beatIndex + 1 < story.beats.length) {
      setBeatIndex(beatIndex + 1)
      return
    }
    const storyIndex = FIGURE_STORIES.findIndex((item) => item.id === story.id)
    const following = FIGURE_STORIES[(storyIndex + 1) % FIGURE_STORIES.length]
    setStoryId(following.id)
    setBeatIndex(0)
  }, [beatIndex, story])

  useEffect(() => {
    onSelect(focus?.id)
  }, [focus?.id, onSelect])

  useEffect(() => {
    const stage = stageRef.current
    if (!stage || !focusId) return
    const node = stage.querySelector<HTMLElement>(`[data-fig-node="${focusId}"]`)
    if (!node) return
    const stageRect = stage.getBoundingClientRect()
    const nodeRect = node.getBoundingClientRect()
    const pad = 20
    if (nodeRect.top < stageRect.top + pad) stage.scrollTop -= stageRect.top + pad - nodeRect.top
    else if (nodeRect.bottom > stageRect.bottom - pad) stage.scrollTop += nodeRect.bottom - (stageRect.bottom - pad)
    if (nodeRect.left < stageRect.left + pad) stage.scrollLeft -= stageRect.left + pad - nodeRect.left
    else if (nodeRect.right > stageRect.right - pad) stage.scrollLeft += nodeRect.right - (stageRect.right - pad)
  }, [focusId, scale])

  useEffect(() => {
    if (!playing) return
    const timer = window.setTimeout(() => advance(), BEAT_MS)
    return () => window.clearTimeout(timer)
  }, [advance, playing])

  const chooseStory = (next: FigureStory) => {
    setStoryId(next.id)
    setBeatIndex(0)
  }

  const stepOnce = () => {
    setPlaying(false)
    advance()
  }

  const trackHover = (id: string, element: HTMLButtonElement) => {
    const host = figureRef.current
    if (!host) return
    const nodeRect = element.getBoundingClientRect()
    const hostRect = host.getBoundingClientRect()
    setHover({
      id,
      left: nodeRect.left - hostRect.left + nodeRect.width / 2,
      top: nodeRect.top - hostRect.top
    })
  }

  const openDetail = (id: string) => {
    setPlaying(false)
    setDetailId(id)
    onSelect(id)
  }

  if (detailId) detailHold.current = detailId
  const shownDetailId = detailId ?? detailHold.current
  const detail = shownDetailId === 'intent-stamp' ? undefined : figureStep(shownDetailId ?? '')

  return (
    <div
      className="harness-figure"
      data-snapshot={snapshot.id}
      data-story={story.id}
      data-ui="settings.evolution.figure"
      ref={figureRef}>
      <div className="harness-figure__stage" ref={stageRef}>
        <div className="harness-figure__scale" style={{ width: BOARD_W * scale, height: BOARD_H * scale }}>
          <div
            className="harness-figure__board"
            style={{ width: BOARD_W, height: BOARD_H, transform: `scale(${scale})` }}>
            {FIGURE_FRAMES.map((frame) => (
              <section className="harness-figure__frame" key={frame.id} style={boxStyle(frame)}>
                <h2>{t(frame.labelKey)}</h2>
              </section>
            ))}
            {FIGURE_CAPTIONS.map((caption) => (
              <p className="harness-figure__column" key={caption.labelKey} style={{ left: caption.x, top: caption.y }}>
                {t(caption.labelKey)}
              </p>
            ))}
            <svg className="harness-figure__edges" viewBox={`0 0 ${BOARD_W} ${BOARD_H}`} aria-hidden="true">
              <defs>
                <marker id="harness-arrow" markerWidth="8" markerHeight="8" refX="7" refY="4" orient="auto">
                  <path d="M0 0 L8 4 L0 8 Z" />
                </marker>
                <marker id="harness-arrow-live" markerWidth="8" markerHeight="8" refX="7" refY="4" orient="auto">
                  <path d="M0 0 L8 4 L0 8 Z" />
                </marker>
              </defs>
              {edges.map((edge) => (
                <path
                  className={edge.id === activeEdge ? 'is-live' : undefined}
                  d={edge.d}
                  data-fig-edge={edge.id}
                  key={edge.id}
                  markerEnd={edge.id === activeEdge ? 'url(#harness-arrow-live)' : 'url(#harness-arrow)'}
                />
              ))}
              {playing && edges.some((edge) => edge.id === activeEdge) ? (
                <circle className="harness-figure__packet" r="5">
                  <animateMotion
                    dur="1.1s"
                    fill="freeze"
                    path={edges.find((edge) => edge.id === activeEdge)?.d}
                    rotate="0"
                  />
                </circle>
              ) : null}
            </svg>
            {edges.map((edge) =>
              edge.labelKey ? (
                <span
                  className={edge.id === activeEdge ? 'harness-figure__pill is-live' : 'harness-figure__pill'}
                  data-fig-pill={edge.id}
                  key={edge.id}
                  style={{ left: edge.labelX, top: edge.labelY }}>
                  {t(edge.labelKey)}
                </span>
              ) : null
            )}
            <article className="harness-figure__turn" style={boxStyle(TURN_BOX)}>
              <h2>{t('settings.evolution.figure.turn')}</h2>
              {focus ? (
                <div className="harness-figure__turn-card">
                  <p className="harness-figure__tag">{t(harnessSectionKey(focus))}</p>
                  <ol className="harness-figure__turn-list">
                    {filled.map((step) => (
                      <li className={step.id === focus.id ? 'is-current' : undefined} key={step.id}>
                        {step.label}
                      </li>
                    ))}
                  </ol>
                  {focus.ruleKey ? <p className="harness-figure__turn-rule">{t(focus.ruleKey)}</p> : null}
                  <p className="harness-figure__turn-role">{t(focus.roleKey)}</p>
                </div>
              ) : null}
            </article>
            {FIGURE_BOXES.map((box) => {
              const step = figureStep(box.id)
              if (!step) return null
              const mounted = componentMounted(snapshot, box.id)
              const order = componentOrder(snapshot, box.id)
              return (
                <button
                  className={`harness-figure__node is-${box.shape}${lit.has(box.id) ? ' is-lit' : ''}${hover?.id === box.id ? ' is-hover' : ''}${mounted === false ? ' is-unmounted' : ''}`}
                  data-fig-node={box.id}
                  data-kind={step.kind}
                  data-mounted={mounted === undefined ? undefined : String(mounted)}
                  data-order={order === undefined ? undefined : String(order)}
                  key={box.id}
                  onClick={() => openDetail(box.id)}
                  onMouseEnter={(event) => trackHover(box.id, event.currentTarget)}
                  onMouseLeave={() => setHover((current) => (current?.id === box.id ? undefined : current))}
                  style={boxStyle(box)}
                  type="button">
                  <strong>{step.label}</strong>
                  {step.ruleKey ? <small>{t(step.ruleKey)}</small> : null}
                  {order ? <em>{order}</em> : null}
                </button>
              )
            })}
            {componentMounted(snapshot, 'intent-stamp') ? (
              <button
                className={`harness-figure__host${hover?.id === 'intent-stamp' ? ' is-hover' : ''}`}
                data-fig-node="intent-stamp"
                data-mounted="true"
                data-order={componentOrder(snapshot, 'intent-stamp')}
                onClick={() => openDetail('intent-stamp')}
                onMouseEnter={(event) => trackHover('intent-stamp', event.currentTarget)}
                onMouseLeave={() => setHover((current) => (current?.id === 'intent-stamp' ? undefined : current))}
                style={{ left: 780, top: 20, width: 340, height: 22 }}
                type="button">
                host:intent_stamp
              </button>
            ) : null}
          </div>
        </div>
      </div>
      {hover && hover.id !== detailId ? (
        <p className="harness-figure__peek" style={{ left: hover.left, top: Math.max(8, hover.top - 28) }}>
          {t('settings.evolution.detail.peek')}
        </p>
      ) : null}
      {detailPresence.present && shownDetailId ? (
        <aside
          aria-label={t('settings.evolution.detail.title')}
          className={`harness-figure__detail a3s-dialog-surface${detailPresence.state === 'closed' ? ' pointer-events-none' : ''}`}
          data-state={detailPresence.state}
          data-ui="settings.evolution.detail"
          role="dialog">
          <header>
            <h2>{t('settings.evolution.detail.title')}</h2>
            <button onClick={() => setDetailId(undefined)} type="button">
              <X aria-hidden size={14} strokeWidth={1.75} />
              {t('settings.evolution.detail.close')}
            </button>
          </header>
          <DetailBody detail={detail} detailId={shownDetailId} snapshot={snapshot} />
        </aside>
      ) : null}
      <div className="harness-figure__controls">
        <p className="harness-figure__caption" data-ui="settings.evolution.figure.caption">
          {focus ? t(focus.roleKey) : ''}
        </p>
        <div className="harness-figure__bar">
          <div className="harness-figure__transport">
            <button aria-pressed={playing} onClick={() => setPlaying((current) => !current)} type="button">
              {playing ? (
                <Pause aria-hidden size={14} strokeWidth={1.75} />
              ) : (
                <Play aria-hidden size={14} strokeWidth={1.75} />
              )}
              {playing ? t('settings.evolution.figure.pause') : t('settings.evolution.figure.play')}
            </button>
            <button onClick={stepOnce} type="button">
              <SkipForward aria-hidden size={14} strokeWidth={1.75} />
              {t('settings.evolution.figure.next')}
            </button>
          </div>
          <div className="harness-figure__tabs" role="tablist">
            {FIGURE_STORIES.map((item) => {
              const Icon = STORY_ICON[item.id] ?? FileText
              return (
                <Fragment key={item.id}>
                  {item.id === 'gate' ? <span aria-hidden className="harness-figure__tab-split" /> : null}
                  <button
                    aria-selected={item.id === story.id}
                    className={item.id === story.id ? 'is-selected' : undefined}
                    onClick={() => chooseStory(item)}
                    role="tab"
                    type="button">
                    <Icon aria-hidden size={14} strokeWidth={1.75} />
                    {t(item.labelKey)}
                  </button>
                </Fragment>
              )
            })}
          </div>
        </div>
      </div>
    </div>
  )
}

function DetailBody({
  detail,
  detailId,
  snapshot
}: {
  detail: HarnessExecutionStep | undefined
  detailId: string
  snapshot: HarnessSnapshot
}) {
  const { t } = useTranslation()
  const mounted = componentMounted(snapshot, detailId)
  const label = detailId === 'intent-stamp' ? 'host:intent_stamp' : (detail?.label ?? detailId)
  const composeNote =
    detailId === 'intent-stamp'
      ? t('settings.evolution.detail.host')
      : detailId === 'permission-projection' || detailId === 'completion-gate'
        ? t('settings.evolution.detail.kernel')
        : mounted === undefined
          ? t('settings.evolution.detail.scheduler')
          : mounted
            ? t('settings.evolution.detail.mounted')
            : t('settings.evolution.detail.absent')

  return (
    <div>
      <p className="harness-figure__detail-label">{label}</p>
      {detail ? <p className="harness-figure__tag">{t(harnessSectionKey(detail))}</p> : null}
      {detail?.ruleKey ? <p className="harness-figure__turn-rule">{t(detail.ruleKey)}</p> : null}
      {detail ? <p className="harness-figure__turn-role">{t(detail.roleKey)}</p> : null}
      <p className="harness-figure__detail-note">{composeNote}</p>
      <p className="harness-figure__detail-order">{t('settings.evolution.detail.order')}</p>
      <ol>
        {snapshot.components.map((entry) => (
          <li
            className={
              entry === detailId || (detailId === 'intent-stamp' && entry === 'host:intent_stamp')
                ? 'is-current'
                : undefined
            }
            key={entry}>
            {entry}
          </li>
        ))}
      </ol>
      <HarnessInternals detailId={detailId} snapshotId={snapshot.id} />
    </div>
  )
}

function boxStyle(box: { x: number; y: number; w: number; h: number }) {
  return { left: box.x, top: box.y, width: box.w, height: box.h }
}
