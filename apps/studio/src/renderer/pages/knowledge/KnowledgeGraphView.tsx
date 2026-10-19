import { useEffect, useMemo, useRef, useState, type PointerEvent as ReactPointerEvent } from 'react'
import { useTranslation } from 'react-i18next'

import { graphLabel, graphLabelSize, graphShowsLabel, noteGraph } from './knowledgeGraph'
import type { ReadingNote } from './noteReading'

interface GraphView {
  x: number
  y: number
  scale: number
}

const RESET_VIEW: GraphView = { x: 0, y: 0, scale: 1 }

/** Zoomable local graph. Unresolved names are not nodes and are not created. */
export function KnowledgeGraphView({
  notes,
  openedPath,
  label,
  coverage,
  onOpen
}: {
  notes: readonly ReadingNote[]
  openedPath: string
  label: string
  coverage: string | null
  onOpen: (path: string) => void
}) {
  const { t } = useTranslation()
  const [orphans, setOrphans] = useState(false)
  const [hover, setHover] = useState<string | null>(null)
  const [view, setView] = useState<GraphView>(RESET_VIEW)
  const svgRef = useRef<SVGSVGElement>(null)
  const viewRef = useRef(view)
  viewRef.current = view
  const graph = useMemo(() => noteGraph(notes, { centerPath: openedPath, orphans }), [notes, openedPath, orphans])
  const graphKey = graph.nodes.map((node) => node.path).join('\n')
  useEffect(() => {
    setView(RESET_VIEW)
    setHover(null)
  }, [graphKey, orphans])
  useEffect(() => {
    const svg = svgRef.current
    if (!svg) return
    const onWheel = (event: WheelEvent) => {
      event.preventDefault()
      zoomAt(svg, event.clientX, event.clientY, event.deltaY < 0 ? 1.12 : 1 / 1.12, viewRef, setView)
    }
    svg.addEventListener('wheel', onWheel, { passive: false })
    return () => svg.removeEventListener('wheel', onWheel)
  }, [])
  const neighbors = useMemo(() => {
    const map = new Map<string, Set<string>>()
    for (const edge of graph.edges) {
      const from = map.get(edge.from) ?? new Set<string>()
      from.add(edge.to)
      map.set(edge.from, from)
      const to = map.get(edge.to) ?? new Set<string>()
      to.add(edge.from)
      map.set(edge.to, to)
    }
    return map
  }, [graph.edges])
  const at = new Map(graph.nodes.map((node) => [node.path, node]))
  const reading = notes.length > 0 && graph.nodes.length === 0 && notes.every((note) => note.markdown == null)
  const lit = (path: string) => {
    if (!hover) return true
    if (path === hover) return true
    return neighbors.get(hover)?.has(path) ?? false
  }

  return (
    <div className="flex h-full min-h-0 w-full min-w-0 flex-col bg-white">
      <div className="flex shrink-0 flex-wrap items-center gap-2 px-4 pt-3">
        <p className="m-0 text-[12px] text-[#171717]">
          {openedPath ? t('knowledge.notes.graph_scope_note') : t('knowledge.notes.graph_scope_library')}
        </p>
        {openedPath ? null : (
          <button
            type="button"
            aria-pressed={orphans}
            onClick={() => setOrphans((current) => !current)}
            className={
              orphans
                ? 'h-[30px] rounded-[8px] bg-[#f2f3f5] px-2 text-[12px] text-[#171717]'
                : 'h-[30px] rounded-[8px] px-2 text-[12px] text-[#8a8a8a]'
            }>
            {t('knowledge.notes.graph_orphans')}
          </button>
        )}
        <div className="ml-auto flex items-center gap-1">
          <button type="button" className={zoomClass} onClick={() => zoomBy(svgRef.current, 1.2, viewRef, setView)}>
            {t('knowledge.notes.graph_zoom_in')}
          </button>
          <button type="button" className={zoomClass} onClick={() => zoomBy(svgRef.current, 1 / 1.2, viewRef, setView)}>
            {t('knowledge.notes.graph_zoom_out')}
          </button>
          <button type="button" className={zoomClass} onClick={() => setView(RESET_VIEW)}>
            {t('knowledge.notes.graph_reset')}
          </button>
        </div>
      </div>
      {coverage ? <p className="m-0 px-4 pt-2 text-[12px] text-[#8a8a8a]">{coverage}</p> : null}
      {openedPath ? null : (
        <p className="m-0 px-4 pt-2 text-[12px] text-[#8a8a8a]">{t('knowledge.notes.graph_hint')}</p>
      )}
      {reading ? (
        <p role="status" className="m-0 px-4 py-6 text-[13px] text-[#8a8a8a]">
          {t('knowledge.notes.graph_reading')}
        </p>
      ) : null}
      {!reading && graph.nodes.length === 0 ? (
        <p role="status" className="m-0 px-4 py-6 text-[13px] text-[#8a8a8a]">
          {t('knowledge.notes.graph_empty')}
        </p>
      ) : null}
      {openedPath && graph.nodes.length === 1 ? (
        <p className="m-0 px-4 pt-2 text-[12px] text-[#8a8a8a]">{t('knowledge.notes.graph_local_empty')}</p>
      ) : null}
      <nav aria-label={label} className="min-h-0 flex-1 px-4 pb-4">
        <svg
          ref={svgRef}
          viewBox="0 0 100 100"
          className="h-full w-full touch-none"
          onPointerDown={(event) => startPan(event, svgRef.current, viewRef, setView)}>
          <g transform={`translate(${view.x} ${view.y}) scale(${view.scale})`}>
            {graph.edges.map((edge) => {
              const source = at.get(edge.from)
              const target = at.get(edge.to)
              if (!source || !target) return null
              const on = lit(edge.from) && lit(edge.to)
              return (
                <line
                  key={`${edge.from}\0${edge.to}`}
                  x1={source.x}
                  y1={source.y}
                  x2={target.x}
                  y2={target.y}
                  stroke={on ? '#bdbdbd' : '#efefef'}
                  strokeWidth={on ? 0.45 : 0.3}
                />
              )
            })}
            {graph.nodes.map((node) => {
              const current = node.path === openedPath
              const on = lit(node.path)
              const showLabel = graphShowsLabel({
                count: graph.nodes.length,
                zoom: view.scale,
                hovered: hover === node.path,
                center: current
              })
              return (
                <g
                  key={node.path}
                  role="button"
                  tabIndex={0}
                  data-graph-node=""
                  aria-label={node.title}
                  aria-current={current ? 'page' : undefined}
                  className="cursor-pointer"
                  opacity={on ? 1 : 0.16}
                  onPointerEnter={() => setHover(node.path)}
                  onPointerLeave={() => setHover((current) => (current === node.path ? null : current))}
                  onClick={() => onOpen(node.path)}
                  onKeyDown={(event) => {
                    if (event.key === 'Enter' || event.key === ' ') {
                      event.preventDefault()
                      onOpen(node.path)
                    }
                  }}>
                  <circle
                    cx={node.x}
                    cy={node.y}
                    r={current ? 2.2 : 1.5}
                    fill={current || hover === node.path ? '#171717' : '#ffffff'}
                    stroke="#171717"
                    strokeWidth="0.4"
                  />
                  {showLabel ? (
                    <text
                      x={node.x}
                      y={node.y + (current ? 4.6 : 3.8)}
                      textAnchor="middle"
                      fontSize={hover === node.path ? 3 : graphLabelSize(graph.nodes.length)}
                      fill="#171717">
                      {hover === node.path ? node.title : graphLabel(node.title, graph.nodes.length)}
                    </text>
                  ) : null}
                  <title>{node.title}</title>
                </g>
              )
            })}
          </g>
        </svg>
      </nav>
    </div>
  )
}

const zoomClass = 'h-[30px] rounded-[8px] px-2 text-[12px] text-[#171717] hover:bg-[#f7f7f7]'

function zoomBy(
  svg: SVGSVGElement | null,
  factor: number,
  viewRef: { current: GraphView },
  setView: (value: GraphView) => void
) {
  if (!svg) {
    const current = viewRef.current
    const scale = clamp(current.scale * factor)
    setView({ ...current, scale })
    return
  }
  const rect = svg.getBoundingClientRect()
  zoomAt(svg, rect.left + rect.width / 2, rect.top + rect.height / 2, factor, viewRef, setView)
}

function zoomAt(
  svg: SVGSVGElement,
  clientX: number,
  clientY: number,
  factor: number,
  viewRef: { current: GraphView },
  setView: (value: GraphView) => void
) {
  const rect = svg.getBoundingClientRect()
  const width = rect.width || 1
  const height = rect.height || 1
  const px = ((clientX - rect.left) / width) * 100
  const py = ((clientY - rect.top) / height) * 100
  const current = viewRef.current
  const scale = clamp(current.scale * factor)
  const gx = (px - current.x) / current.scale
  const gy = (py - current.y) / current.scale
  setView({ scale, x: px - gx * scale, y: py - gy * scale })
}

function startPan(
  event: ReactPointerEvent<SVGSVGElement>,
  svg: SVGSVGElement | null,
  viewRef: { current: GraphView },
  setView: (value: GraphView) => void
) {
  if (!svg) return
  if ((event.target as Element).closest?.('[data-graph-node]')) return
  const origin = viewRef.current
  const startX = event.clientX
  const startY = event.clientY
  const target = event.currentTarget
  target.setPointerCapture(event.pointerId)
  const move = (next: PointerEvent) => {
    const rect = svg.getBoundingClientRect()
    const dx = ((next.clientX - startX) / (rect.width || 1)) * 100
    const dy = ((next.clientY - startY) / (rect.height || 1)) * 100
    setView({ ...origin, x: origin.x + dx, y: origin.y + dy })
  }
  const up = () => {
    target.removeEventListener('pointermove', move)
    target.removeEventListener('pointerup', up)
  }
  target.addEventListener('pointermove', move)
  target.addEventListener('pointerup', up)
}

function clamp(scale: number): number {
  return Math.min(3.5, Math.max(0.5, scale))
}
