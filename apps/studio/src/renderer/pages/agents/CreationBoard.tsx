import {
  ArrowLeft,
  Expand,
  Hand,
  Minus,
  MousePointer2,
  Pencil,
  RectangleHorizontal,
  Redo2,
  Square,
  StickyNote,
  Type,
  Undo2,
  ZoomIn,
  ZoomOut
} from 'lucide-react'
import { useEffect, useId, useRef, useState } from 'react'
import { useTranslation } from 'react-i18next'

import {
  boardFor,
  boundsOf,
  createNodeId,
  fitCamera,
  hitNode,
  placeNode,
  resizeNode,
  screenToWorld,
  STICKY_COLORS,
  writeBoard,
  zoomAt,
  type BoardDoc,
  type BoardNode,
  type BoardPoint,
  type CreateKind,
  type CreateTool,
  type ResizeCorner
} from './creationBoardModel'

import './creationBoard.css'

type HistorySnap = Pick<BoardDoc, 'title' | 'titleLiteral' | 'nodes'>

type Gesture =
  | { mode: 'move'; id: string; dx: number; dy: number; base: BoardDoc }
  | { mode: 'resize'; id: string; corner: ResizeCorner; base: BoardDoc; px: number; py: number }
  | { mode: 'pan'; cx: number; cy: number; panX: number; panY: number }
  | { mode: 'pen'; points: BoardPoint[] }
  | { mode: 'line'; x: number; y: number; points: BoardPoint[] }
  | { mode: 'place'; tool: 'sticky' | 'text' | 'shape' | 'frame'; cx: number; cy: number }

const TOOLS: Array<{ id: CreateTool; label: string; shortcut: string }> = [
  { id: 'select', label: 'workbench.create.select', shortcut: 'V' },
  { id: 'hand', label: 'workbench.create.hand', shortcut: 'H' },
  { id: 'sticky', label: 'workbench.create.sticky', shortcut: 'N' },
  { id: 'text', label: 'workbench.create.text', shortcut: 'T' },
  { id: 'shape', label: 'workbench.create.shape', shortcut: 'S' },
  { id: 'frame', label: 'workbench.create.frame', shortcut: 'F' },
  { id: 'pen', label: 'workbench.create.pen', shortcut: 'P' },
  { id: 'connector', label: 'workbench.create.line', shortcut: 'L' }
]

const KIND_LABEL: Record<CreateKind, string> = {
  center: 'workbench.create.kind.center',
  image: 'workbench.create.kind.image',
  ppt: 'workbench.create.kind.ppt',
  site: 'workbench.create.kind.site',
  video: 'workbench.create.kind.video'
}

function storageOrNull() {
  try {
    return globalThis.localStorage
  } catch {
    return null
  }
}

function snapOf(doc: BoardDoc): HistorySnap {
  return { title: doc.title, titleLiteral: doc.titleLiteral, nodes: doc.nodes }
}

function ToolIcon({ id }: { id: CreateTool }) {
  const props = { size: 16, strokeWidth: 1.7, 'aria-hidden': true as const }
  if (id === 'select') return <MousePointer2 {...props} />
  if (id === 'hand') return <Hand {...props} />
  if (id === 'sticky') return <StickyNote {...props} />
  if (id === 'text') return <Type {...props} />
  if (id === 'shape') return <Square {...props} />
  if (id === 'frame') return <RectangleHorizontal {...props} />
  if (id === 'pen') return <Pencil {...props} />
  return <Minus {...props} />
}

function nodeLabel(node: BoardNode, t: (key: string) => string) {
  if (node.literal || node.text === '') return node.text
  return t(node.text)
}

export function CreationBoard({ kind, onClose }: { kind: CreateKind; onClose: () => void }) {
  const { t } = useTranslation()
  const markerId = useId().replace(/:/g, '')
  const canvasRef = useRef<HTMLDivElement>(null)
  const [doc, setDoc] = useState<BoardDoc>(() => boardFor(storageOrNull(), kind))
  const [tool, setTool] = useState<CreateTool>('select')
  const [selectedId, setSelectedId] = useState<string | null>(null)
  const [editingId, setEditingId] = useState<string | null>(null)
  const [spaceDown, setSpaceDown] = useState(false)
  const [panning, setPanning] = useState(false)
  const [draft, setDraft] = useState<BoardNode | null>(null)
  const [pastCount, setPastCount] = useState(0)
  const [futureCount, setFutureCount] = useState(0)
  const docRef = useRef(doc)
  const toolRef = useRef(tool)
  const spaceRef = useRef(false)
  const gestureRef = useRef<Gesture | null>(null)
  const pastRef = useRef<HistorySnap[]>([])
  const futureRef = useRef<HistorySnap[]>([])
  const editRecorded = useRef(false)
  docRef.current = doc
  toolRef.current = tool

  useEffect(() => {
    writeBoard(storageOrNull(), kind, doc)
  }, [doc, kind])

  useEffect(() => {
    const node = canvasRef.current
    if (!node || typeof ResizeObserver === 'undefined') return
    let fitted = false
    const observer = new ResizeObserver(() => {
      if (fitted) return
      const rect = node.getBoundingClientRect()
      const next = fitCamera(docRef.current.nodes, rect.width, rect.height)
      if (!next) return
      fitted = true
      setDoc((current) => ({ ...current, camera: next }))
    })
    observer.observe(node)
    return () => observer.disconnect()
  }, [])

  function publish(next: BoardDoc) {
    docRef.current = next
    setDoc(next)
  }

  function remember(base: BoardDoc) {
    pastRef.current.push(structuredClone(snapOf(base)))
    if (pastRef.current.length > 50) pastRef.current.shift()
    futureRef.current = []
    setPastCount(pastRef.current.length)
    setFutureCount(0)
  }

  function commit(next: BoardDoc, base: BoardDoc) {
    remember(base)
    publish(next)
  }

  function undo() {
    const previous = pastRef.current.pop()
    if (!previous) return
    futureRef.current.push(snapOf(docRef.current))
    publish({ ...docRef.current, ...structuredClone(previous) })
    setSelectedId(null)
    setEditingId(null)
    setPastCount(pastRef.current.length)
    setFutureCount(futureRef.current.length)
  }

  function redo() {
    const nextSnap = futureRef.current.pop()
    if (!nextSnap) return
    pastRef.current.push(snapOf(docRef.current))
    publish({ ...docRef.current, ...structuredClone(nextSnap) })
    setSelectedId(null)
    setEditingId(null)
    setPastCount(pastRef.current.length)
    setFutureCount(futureRef.current.length)
  }

  function localOf(event: { clientX: number; clientY: number }) {
    const rect = canvasRef.current?.getBoundingClientRect()
    return { x: event.clientX - (rect?.left ?? 0), y: event.clientY - (rect?.top ?? 0) }
  }

  function worldOf(event: { clientX: number; clientY: number }) {
    const local = localOf(event)
    return screenToWorld(docRef.current.camera, local.x, local.y)
  }

  useEffect(() => {
    const onKeyDown = (event: KeyboardEvent) => {
      const target = event.target
      const typing = target instanceof HTMLInputElement || target instanceof HTMLTextAreaElement
      const key = event.key.toLowerCase()
      if ((event.metaKey || event.ctrlKey) && key === 'z' && !event.altKey) {
        event.preventDefault()
        if (event.shiftKey) redo()
        else undo()
        return
      }
      if (typing) return
      if (event.code === 'Space') {
        event.preventDefault()
        spaceRef.current = true
        setSpaceDown(true)
        return
      }
      if (event.key === 'Escape') {
        setDraft(null)
        gestureRef.current = null
        if (editingId) {
          setEditingId(null)
          return
        }
        if (toolRef.current !== 'select') {
          setTool('select')
          return
        }
        setSelectedId(null)
        return
      }
      if ((event.key === 'Backspace' || event.key === 'Delete') && selectedId) {
        event.preventDefault()
        const current = docRef.current
        commit({ ...current, nodes: current.nodes.filter((node) => node.id !== selectedId) }, current)
        setSelectedId(null)
        setEditingId(null)
        return
      }
      const next = TOOLS.find((item) => item.shortcut.toLowerCase() === key)
      if (next && !event.metaKey && !event.ctrlKey && !event.altKey) setTool(next.id)
    }
    const onKeyUp = (event: KeyboardEvent) => {
      if (event.code !== 'Space') return
      spaceRef.current = false
      setSpaceDown(false)
    }
    window.addEventListener('keydown', onKeyDown)
    window.addEventListener('keyup', onKeyUp)
    return () => {
      window.removeEventListener('keydown', onKeyDown)
      window.removeEventListener('keyup', onKeyUp)
    }
  }, [editingId, selectedId])

  useEffect(() => {
    const node = canvasRef.current
    if (!node) return
    const onWheel = (event: WheelEvent) => {
      event.preventDefault()
      const local = localOf(event)
      const camera = docRef.current.camera
      const next =
        event.ctrlKey || event.metaKey
          ? zoomAt(camera, local.x, local.y, camera.zoom * Math.exp(-event.deltaY * 0.0016))
          : { ...camera, panX: camera.panX - event.deltaX, panY: camera.panY - event.deltaY }
      publish({ ...docRef.current, camera: next })
    }
    node.addEventListener('wheel', onWheel, { passive: false })
    return () => node.removeEventListener('wheel', onWheel)
  }, [])

  function zoomBy(factor: number) {
    const rect = canvasRef.current?.getBoundingClientRect()
    const camera = docRef.current.camera
    publish({
      ...docRef.current,
      camera: zoomAt(camera, (rect?.width ?? 0) / 2, (rect?.height ?? 0) / 2, camera.zoom * factor)
    })
  }

  function fit() {
    const rect = canvasRef.current?.getBoundingClientRect()
    const next = fitCamera(docRef.current.nodes, rect?.width ?? 0, rect?.height ?? 0)
    if (!next) return
    publish({ ...docRef.current, camera: next })
  }

  function patchNode(id: string, recipe: (node: BoardNode) => BoardNode, base = docRef.current) {
    return { ...base, nodes: base.nodes.map((node) => (node.id === id ? recipe(node) : node)) }
  }

  function onCanvasPointerDown(event: React.PointerEvent<HTMLDivElement>) {
    if (event.button !== 0 && event.button !== 1) return
    const currentTool = toolRef.current
    const pan = currentTool === 'hand' || spaceRef.current || event.button === 1
    if (pan) {
      gestureRef.current = {
        mode: 'pan',
        cx: event.clientX,
        cy: event.clientY,
        panX: docRef.current.camera.panX,
        panY: docRef.current.camera.panY
      }
      setPanning(true)
      event.currentTarget.setPointerCapture?.(event.pointerId)
      return
    }
    const world = worldOf(event)
    const hit = hitNode(docRef.current.nodes, world.x, world.y)
    if (currentTool === 'select') {
      setSelectedId(hit?.id ?? null)
      setEditingId(null)
      return
    }
    if (currentTool === 'pen') {
      gestureRef.current = { mode: 'pen', points: [[world.x, world.y]] }
      event.currentTarget.setPointerCapture?.(event.pointerId)
      return
    }
    if (currentTool === 'connector') {
      gestureRef.current = { mode: 'line', x: world.x, y: world.y, points: [[world.x, world.y]] }
      event.currentTarget.setPointerCapture?.(event.pointerId)
      return
    }
    if (currentTool === 'sticky' || currentTool === 'text' || currentTool === 'shape' || currentTool === 'frame') {
      gestureRef.current = { mode: 'place', tool: currentTool, cx: event.clientX, cy: event.clientY }
      event.currentTarget.setPointerCapture?.(event.pointerId)
    }
  }

  function onNodePointerDown(event: React.PointerEvent<HTMLDivElement>, id: string) {
    if (toolRef.current !== 'select' || spaceRef.current || event.button !== 0) return
    const target = event.target
    if (target instanceof HTMLElement && target.closest('textarea, input')) return
    event.stopPropagation()
    const node = docRef.current.nodes.find((item) => item.id === id)
    if (!node) return
    const world = worldOf(event)
    setSelectedId(id)
    setEditingId(null)
    gestureRef.current = { mode: 'move', id, dx: node.x - world.x, dy: node.y - world.y, base: docRef.current }
    canvasRef.current?.setPointerCapture?.(event.pointerId)
  }

  function onResizePointerDown(event: React.PointerEvent<HTMLButtonElement>, id: string, corner: ResizeCorner) {
    event.stopPropagation()
    const world = worldOf(event)
    gestureRef.current = { mode: 'resize', id, corner, base: docRef.current, px: world.x, py: world.y }
    setSelectedId(id)
    canvasRef.current?.setPointerCapture?.(event.pointerId)
  }

  function onPointerMove(event: React.PointerEvent<HTMLDivElement>) {
    const gesture = gestureRef.current
    if (!gesture) return
    if (gesture.mode === 'pan') {
      publish({
        ...docRef.current,
        camera: {
          ...docRef.current.camera,
          panX: gesture.panX + event.clientX - gesture.cx,
          panY: gesture.panY + event.clientY - gesture.cy
        }
      })
      return
    }
    const world = worldOf(event)
    if (gesture.mode === 'move') {
      publish(
        patchNode(gesture.id, (node) => ({ ...node, x: world.x + gesture.dx, y: world.y + gesture.dy }), gesture.base)
      )
      return
    }
    if (gesture.mode === 'resize') {
      const origin = gesture.base.nodes.find((node) => node.id === gesture.id)
      if (!origin) return
      const next = resizeNode(origin, gesture.corner, world.x - gesture.px, world.y - gesture.py)
      publish(patchNode(gesture.id, (node) => ({ ...node, ...next }), gesture.base))
      return
    }
    if (gesture.mode === 'pen') {
      const last = gesture.points[gesture.points.length - 1]
      if (!last || Math.hypot(world.x - last[0], world.y - last[1]) > 2) gesture.points.push([world.x, world.y])
      const box = boundsOf(gesture.points)
      setDraft({
        id: 'draft-pen',
        kind: 'pen',
        ...box,
        text: '',
        literal: true,
        fill: 'transparent'
      })
      return
    }
    if (gesture.mode === 'line') {
      gesture.points = [
        [gesture.x, gesture.y],
        [world.x, world.y]
      ]
      const box = boundsOf(gesture.points)
      setDraft({
        id: 'draft-line',
        kind: 'connector',
        ...box,
        text: '',
        literal: true,
        fill: 'transparent'
      })
    }
  }

  function finishGesture(event: React.PointerEvent<HTMLDivElement>) {
    const gesture = gestureRef.current
    gestureRef.current = null
    setPanning(false)
    setDraft(null)
    if (!gesture) return
    const current = docRef.current
    if (gesture.mode === 'move' || gesture.mode === 'resize') {
      const changed = JSON.stringify(current.nodes) !== JSON.stringify(gesture.base.nodes)
      if (changed) remember(gesture.base)
      return
    }
    if (gesture.mode === 'pen' && gesture.points.length > 1) {
      const box = boundsOf(gesture.points)
      commit(
        {
          ...current,
          nodes: [
            ...current.nodes,
            { id: createNodeId(), kind: 'pen', ...box, text: '', literal: true, fill: 'transparent' }
          ]
        },
        current
      )
      return
    }
    if (gesture.mode === 'line' && gesture.points.length === 2) {
      const [start, end] = gesture.points
      if (start && end && Math.hypot(end[0] - start[0], end[1] - start[1]) > 8) {
        const box = boundsOf(gesture.points)
        commit(
          {
            ...current,
            nodes: [
              ...current.nodes,
              {
                id: createNodeId(),
                kind: 'connector',
                ...box,
                text: '',
                literal: true,
                fill: 'transparent'
              }
            ]
          },
          current
        )
      }
      return
    }
    if (gesture.mode === 'place' && Math.hypot(event.clientX - gesture.cx, event.clientY - gesture.cy) < 6) {
      const world = worldOf(event)
      const placed = placeNode(gesture.tool, world.x, world.y)
      commit({ ...current, nodes: [...current.nodes, placed] }, current)
      setSelectedId(placed.id)
      if (placed.kind === 'sticky' || placed.kind === 'text') setEditingId(placed.id)
    }
  }

  function onDoubleClick(event: React.MouseEvent<HTMLDivElement>) {
    if (toolRef.current !== 'select' || spaceRef.current) return
    const target = event.target
    if (target instanceof HTMLElement && target.closest('[data-node-id]')) return
    const world = worldOf(event)
    if (hitNode(docRef.current.nodes, world.x, world.y)) return
    const placed = placeNode('sticky', world.x, world.y)
    commit({ ...docRef.current, nodes: [...docRef.current.nodes, placed] }, docRef.current)
    setSelectedId(placed.id)
    setEditingId(placed.id)
  }

  function changeText(id: string, text: string) {
    const current = docRef.current
    if (!editRecorded.current) {
      remember(current)
      editRecorded.current = true
    }
    publish(patchNode(id, (node) => ({ ...node, text, literal: true }), docRef.current))
  }

  const camera = doc.camera
  const selected = doc.nodes.find((node) => node.id === selectedId) ?? null
  const dot = 22 * camera.zoom
  const title = doc.titleLiteral ? doc.title : t(doc.title)

  return (
    <section
      data-ui="workbench.create.board"
      data-kind={kind}
      data-zoom={camera.zoom.toFixed(2)}
      aria-label={t('workbench.feature.create')}
      className="absolute inset-0 z-40 flex flex-col bg-[#f3f4f6] text-[#171717]">
      <header className="flex h-12 shrink-0 items-center gap-2 border-b border-[#efefef] bg-white px-3">
        <button
          type="button"
          aria-label={t('workbench.create.back')}
          onClick={onClose}
          className="grid size-8 place-items-center rounded-[8px] text-[#3f3f3f] hover:bg-[#f2f3f5] hover:text-[#171717]">
          <ArrowLeft size={16} strokeWidth={1.7} />
        </button>
        <button
          type="button"
          aria-label={t('workbench.create.undo')}
          disabled={pastCount === 0}
          onClick={undo}
          className="grid size-8 place-items-center rounded-[8px] text-[#3f3f3f] hover:bg-[#f2f3f5] disabled:opacity-40">
          <Undo2 size={16} strokeWidth={1.7} />
        </button>
        <button
          type="button"
          aria-label={t('workbench.create.redo')}
          disabled={futureCount === 0}
          onClick={redo}
          className="grid size-8 place-items-center rounded-[8px] text-[#3f3f3f] hover:bg-[#f2f3f5] disabled:opacity-40">
          <Redo2 size={16} strokeWidth={1.7} />
        </button>
        <input
          aria-label={t('workbench.create.title')}
          value={title}
          onChange={(event) => publish({ ...docRef.current, title: event.target.value, titleLiteral: true })}
          className="ml-1 h-8 min-w-0 flex-1 rounded-[8px] bg-transparent px-2 text-[14px] font-medium text-[#171717] outline-none focus:bg-[#f5f5f5]"
        />
        {selected?.kind === 'sticky' ? (
          <div
            role="group"
            aria-label={t('workbench.create.color')}
            data-ui="workbench.create.colors"
            className="flex items-center gap-1">
            {STICKY_COLORS.map((swatch) => (
              <button
                key={swatch.id}
                type="button"
                aria-label={t(`workbench.create.swatch.${swatch.id}`)}
                data-swatch={swatch.id}
                aria-pressed={selected.fill === swatch.value}
                onClick={() => {
                  const current = docRef.current
                  commit(
                    patchNode(selected.id, (node) => ({ ...node, fill: swatch.value }), current),
                    current
                  )
                }}
                className="size-5 rounded-full ring-1 ring-black/10 ring-inset aria-pressed:ring-2 aria-pressed:ring-[#171717]"
                style={{ background: swatch.value }}
              />
            ))}
          </div>
        ) : null}
        <span className="rounded-full bg-[#f5f5f5] px-2 py-1 text-[12px] text-[#8a8a8a]">{t(KIND_LABEL[kind])}</span>
      </header>
      <div className="relative min-h-0 flex-1">
        <div
          ref={canvasRef}
          data-ui="workbench.create.canvas"
          data-tool={tool}
          data-space={spaceDown ? 'true' : 'false'}
          data-panning={panning ? 'true' : 'false'}
          tabIndex={0}
          className="create-board-canvas absolute inset-0 outline-none"
          style={{ backgroundSize: `${dot}px ${dot}px`, backgroundPosition: `${camera.panX}px ${camera.panY}px` }}
          onPointerDown={onCanvasPointerDown}
          onPointerMove={onPointerMove}
          onPointerUp={finishGesture}
          onDoubleClick={onDoubleClick}>
          <div
            className="pointer-events-none absolute top-0 left-0"
            style={{
              transform: `translate(${camera.panX}px, ${camera.panY}px) scale(${camera.zoom})`,
              transformOrigin: '0 0'
            }}>
            <svg width="0" height="0" className="absolute">
              <defs>
                <marker id={markerId} markerWidth="8" markerHeight="8" refX="6" refY="3" orient="auto">
                  <path d="M0,0 L8,3 L0,6 Z" fill="#8a8a8a" />
                </marker>
              </defs>
            </svg>
            {[...doc.nodes, ...(draft ? [draft] : [])].map((node) => (
              <BoardObject
                key={node.id}
                node={node}
                zoom={camera.zoom}
                markerId={markerId}
                selected={node.id === selectedId}
                editing={node.id === editingId}
                label={nodeLabel(node, t)}
                hint={t('workbench.create.sticky_hint')}
                onPointerDown={(event) => onNodePointerDown(event, node.id)}
                onResize={(event, corner) => onResizePointerDown(event, node.id, corner)}
                onEdit={() => {
                  setSelectedId(node.id)
                  setEditingId(node.id)
                }}
                onText={(text) => changeText(node.id, text)}
                onBlur={() => {
                  editRecorded.current = false
                  setEditingId((current) => (current === node.id ? null : current))
                }}
              />
            ))}
          </div>
          {doc.nodes.length === 0 ? (
            <p
              data-ui="workbench.create.empty"
              className="pointer-events-none absolute top-1/2 left-1/2 max-w-sm -translate-x-1/2 -translate-y-1/2 text-center text-[13px] leading-6 text-[#8a8a8a]">
              {t('workbench.create.empty')}
            </p>
          ) : null}
        </div>
        <div
          role="toolbar"
          aria-label={t('workbench.feature.create')}
          className="absolute top-1/2 left-3 z-10 flex -translate-y-1/2 flex-col gap-0.5 rounded-[16px] border border-[#efefef] bg-white p-1.5 shadow-[0_8px_24px_rgba(23,23,23,0.08)]">
          {TOOLS.map((item) => (
            <button
              key={item.id}
              type="button"
              aria-label={t(item.label)}
              aria-keyshortcuts={item.shortcut}
              aria-pressed={tool === item.id}
              title={`${t(item.label)} (${item.shortcut})`}
              onClick={() => setTool(item.id)}
              className="grid size-9 place-items-center rounded-[8px] text-[#3f3f3f] hover:bg-[#f7f7f7] aria-pressed:bg-[#f2f3f5] aria-pressed:text-[#171717]">
              <ToolIcon id={item.id} />
            </button>
          ))}
        </div>
        <div className="absolute right-4 bottom-4 z-10 flex items-center gap-0.5 rounded-[16px] border border-[#efefef] bg-white p-1 shadow-[0_8px_24px_rgba(23,23,23,0.08)]">
          <button
            type="button"
            aria-label={t('workbench.create.zoom_out')}
            onClick={() => zoomBy(1 / 1.15)}
            className="grid size-8 place-items-center rounded-[8px] text-[#3f3f3f] hover:bg-[#f2f3f5]">
            <ZoomOut size={16} strokeWidth={1.7} />
          </button>
          <span className="w-12 text-center text-[12px] text-[#3f3f3f]">{Math.round(camera.zoom * 100)}%</span>
          <button
            type="button"
            aria-label={t('workbench.create.zoom_in')}
            onClick={() => zoomBy(1.15)}
            className="grid size-8 place-items-center rounded-[8px] text-[#3f3f3f] hover:bg-[#f2f3f5]">
            <ZoomIn size={16} strokeWidth={1.7} />
          </button>
          <button
            type="button"
            aria-label={t('workbench.create.fit')}
            onClick={fit}
            className="grid size-8 place-items-center rounded-[8px] text-[#3f3f3f] hover:bg-[#f2f3f5]">
            <Expand size={16} strokeWidth={1.7} />
          </button>
        </div>
      </div>
    </section>
  )
}

function BoardObject({
  node,
  zoom,
  markerId,
  selected,
  editing,
  label,
  hint,
  onPointerDown,
  onResize,
  onEdit,
  onText,
  onBlur
}: {
  node: BoardNode
  zoom: number
  markerId: string
  selected: boolean
  editing: boolean
  label: string
  hint: string
  onPointerDown: (event: React.PointerEvent<HTMLDivElement>) => void
  onResize: (event: React.PointerEvent<HTMLButtonElement>, corner: ResizeCorner) => void
  onEdit: () => void
  onText: (text: string) => void
  onBlur: () => void
}) {
  const ring = selected ? `0 0 0 ${1.5 / zoom}px #171717` : undefined
  const common = 'pointer-events-auto absolute select-none'
  if (node.kind === 'pen' || node.kind === 'connector') {
    const [start, end] = node.points
    return (
      <div
        data-node-id={node.id}
        data-node-kind={node.kind}
        data-x={Math.round(node.x)}
        data-y={Math.round(node.y)}
        className={common}
        style={{ left: node.x, top: node.y, width: node.w, height: node.h }}
        onPointerDown={onPointerDown}
        onDoubleClick={(event) => event.stopPropagation()}>
        <svg className="absolute inset-0 overflow-visible" width={node.w} height={node.h}>
          {node.kind === 'pen' ? (
            <polyline
              points={node.points.map((point) => point.join(',')).join(' ')}
              fill="none"
              stroke="#171717"
              strokeWidth={2.2 / zoom}
              strokeLinecap="round"
              strokeLinejoin="round"
            />
          ) : start && end ? (
            <line
              x1={start[0]}
              y1={start[1]}
              x2={end[0]}
              y2={end[1]}
              stroke="#8a8a8a"
              strokeWidth={2 / zoom}
              markerEnd={`url(#${markerId})`}
            />
          ) : null}
        </svg>
      </div>
    )
  }

  const resizable = selected && (node.kind === 'sticky' || node.kind === 'shape' || node.kind === 'frame')
  return (
    <div
      data-node-id={node.id}
      data-node-kind={node.kind}
      data-x={Math.round(node.x)}
      data-y={Math.round(node.y)}
      className={common}
      style={{
        left: node.x,
        top: node.y,
        width: node.w,
        height: node.h,
        background: node.kind === 'frame' ? 'transparent' : node.fill,
        borderRadius: node.kind === 'text' ? 0 : 12,
        boxShadow: node.kind === 'sticky' && !selected ? '0 8px 20px rgba(23,23,23,0.08)' : ring,
        border: node.kind === 'frame' ? '1.5px solid #dedede' : node.kind === 'shape' ? '1px solid #e5e5e5' : undefined
      }}
      onPointerDown={onPointerDown}
      onDoubleClick={(event) => {
        event.stopPropagation()
        onEdit()
      }}>
      {node.kind === 'frame' ? (
        editing ? (
          <input
            autoFocus
            aria-label={hint}
            value={label}
            onChange={(event) => onText(event.target.value)}
            onBlur={onBlur}
            onPointerDown={(event) => event.stopPropagation()}
            className="absolute right-0 bottom-full left-0 mb-1 h-6 bg-transparent text-[12px] text-[#171717] outline-none"
          />
        ) : (
          <div className="absolute right-0 bottom-full left-0 truncate pb-1 text-[12px] leading-4 text-[#8a8a8a]">
            {label}
          </div>
        )
      ) : editing ? (
        <textarea
          autoFocus
          aria-label={hint}
          value={label}
          onChange={(event) => onText(event.target.value)}
          onBlur={onBlur}
          onPointerDown={(event) => event.stopPropagation()}
          className="h-full w-full resize-none bg-transparent p-3 text-[14px] leading-5 text-[#171717] outline-none select-text"
        />
      ) : (
        <div
          className={
            node.kind === 'shape'
              ? 'flex h-full items-center justify-center px-3 text-center text-[14px] text-[#171717]'
              : 'p-3 text-[14px] leading-5 break-words whitespace-pre-wrap text-[#171717]'
          }>
          {label === '' && node.kind === 'sticky' ? <span className="text-[#8a8a8a]">{hint}</span> : label}
        </div>
      )}
      {resizable
        ? (['nw', 'ne', 'se', 'sw'] as const).map((corner) => (
            <button
              key={corner}
              type="button"
              tabIndex={-1}
              aria-label={corner}
              data-resize={corner}
              onPointerDown={(event) => onResize(event, corner)}
              className="absolute size-2.5 rounded-[2px] border border-[#171717] bg-white"
              style={{
                left: corner === 'nw' || corner === 'sw' ? 0 : '100%',
                top: corner === 'nw' || corner === 'ne' ? 0 : '100%',
                transform: `translate(-50%, -50%) scale(${1 / zoom})`
              }}
            />
          ))
        : null}
    </div>
  )
}
