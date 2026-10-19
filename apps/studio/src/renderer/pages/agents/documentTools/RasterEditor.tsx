import { SearchSelect } from '@a3s-lab/ui'
import { RotateCcw, RotateCw, Undo2 } from 'lucide-react'
import { useEffect, useMemo, useRef, useState } from 'react'
import { useTranslation } from 'react-i18next'

import { decodePng, encodePng } from '../pdfToolsPng'
import type { ToolOutput } from '../pdfToolsRun'
import {
  applyRasterEdit,
  ORIGINAL_RASTER_EDIT,
  parseCropMode,
  parseCropRatio,
  rectangleCorners,
  validateRasterEdit,
  type CropMode,
  type CropRatio,
  type RasterCorners,
  type RasterEdit,
  type RasterPoint
} from './rasterGeometry'

const P = 'workbench.feature.pdf.tools.workspace.geometry.'
const BUTTON =
  'inline-flex h-9 items-center justify-center gap-1.5 rounded-[8px] border border-[var(--spec-button-border,#e5e5e5)] bg-white px-3 text-[13px] hover:bg-[var(--spec-quiet-button-hover,#fafafa)] disabled:opacity-40 focus-visible:outline-2 focus-visible:outline-offset-2'
const FIELD =
  'h-9 w-full min-w-0 rounded-[8px] border border-[var(--spec-button-border,#e5e5e5)] px-2 text-[13px] focus-visible:outline-2'
const CORNER_KEYS = ['top_left', 'top_right', 'bottom_right', 'bottom_left'] as const
export type RasterEditorDraft = Readonly<{ edit: RasterEdit; mode: CropMode; ratio: CropRatio }>
export type RasterEditorHistory = Readonly<{
  present: RasterEditorDraft
  undo: readonly RasterEditorDraft[]
}>
export function rasterEditsEqual(a: RasterEdit, b: RasterEdit) {
  return (
    a.quarterTurns === b.quarterTurns &&
    a.straighten === b.straighten &&
    a.corners.every((point, index) => point.x === b.corners[index].x && point.y === b.corners[index].y)
  )
}
function ratioNumber(ratio: CropRatio) {
  return ratio === 'free' ? undefined : Number(ratio.split(':')[0]) / Number(ratio.split(':')[1])
}
function fitRatio(edit: RasterEdit, ratio: CropRatio, width: number, height: number): RasterEdit {
  const value = ratioNumber(ratio)
  if (!value) return edit
  const x = edit.corners[0].x,
    y = edit.corners[0].y
  const h = Math.min(edit.corners[2].y - y, ((edit.corners[2].x - x) * width) / (height * value))
  return { ...edit, corners: rectangleCorners(x, y, (h * height * value) / width, h) }
}
function movedCorner(
  draft: RasterEditorDraft,
  index: number,
  point: RasterPoint,
  width: number,
  height: number
): RasterEdit {
  if (draft.mode === 'document') {
    return {
      ...draft.edit,
      corners: draft.edit.corners.map((old, i) => (i === index ? point : old)) as unknown as RasterCorners
    }
  }
  const opposite = draft.edit.corners[(index + 2) % 4]
  const sx = index === 0 || index === 3 ? -1 : 1,
    sy = index === 0 || index === 1 ? -1 : 1
  let w = sx * (point.x - opposite.x),
    h = sy * (point.y - opposite.y)
  if (w <= 0 || h <= 0) return draft.edit
  const ratio = ratioNumber(draft.ratio)
  if (ratio) {
    h = Math.min(h, (w * width) / (height * ratio))
    w = (h * height * ratio) / width
  }
  return {
    ...draft.edit,
    corners: rectangleCorners(
      Math.min(opposite.x, opposite.x + sx * w),
      Math.min(opposite.y, opposite.y + sy * h),
      w,
      h
    )
  }
}

function useHistory(id: string, histories: Map<string, RasterEditorHistory>) {
  const [history, setHistory] = useState<RasterEditorHistory>(
    () =>
      histories.get(id) ?? {
        present: { edit: ORIGINAL_RASTER_EDIT, mode: 'rectangle', ratio: 'free' },
        undo: []
      }
  )
  const update = (draft: RasterEditorDraft, record = true) =>
    setHistory((old) => {
      const next = {
        present: draft,
        undo: record ? [...old.undo, old.present].slice(-32) : old.undo
      }
      histories.set(id, next)
      return next
    })
  const undo = () =>
    setHistory((old) => {
      const previous = old.undo.at(-1)
      if (!previous) return old
      const next = { present: previous, undo: old.undo.slice(0, -1) }
      histories.set(id, next)
      return next
    })
  return { history, update, undo }
}

export function RasterEditor({
  id,
  source,
  histories,
  busy,
  batch,
  committed,
  accepted,
  onApply,
  onDraft
}: {
  id: string
  source: ToolOutput
  histories: Map<string, RasterEditorHistory>
  busy: boolean
  batch: boolean
  committed: RasterEdit
  accepted: boolean
  onApply: (edit: RasterEdit, anglesToSelected: boolean) => Promise<void>
  onDraft: (edit: RasterEdit) => void
}) {
  const { t } = useTranslation()
  const { history, update, undo } = useHistory(id, histories)
  const { edit, mode, ratio } = history.present
  useEffect(() => {
    onDraft(edit)
  }, [edit, onDraft])
  const width = source.width ?? 1,
    height = source.height ?? 1
  const [zoom, setZoom] = useState(100)
  const [anglesToSelected, setAnglesToSelected] = useState(false)
  const [applying, setApplying] = useState(false)
  const [preview, setPreview] = useState<{ url: string; width: number; height: number } | null>(null)
  const [failure, setFailure] = useState<string>()
  const sourceUrl = useMemo(
    () => globalThis.URL.createObjectURL(new globalThis.Blob([source.bytes.slice()], { type: source.mediaType })),
    [source.bytes, source.mediaType]
  )
  const plane = useRef<InstanceType<typeof globalThis.HTMLDivElement>>(null)
  const drag = useRef<number | null>(null)
  useEffect(() => () => globalThis.URL.revokeObjectURL(sourceUrl), [sourceUrl])
  useEffect(() => {
    const controller = new globalThis.AbortController()
    let url: string | undefined
    setPreview(null)
    setFailure(undefined)
    void (async () => {
      try {
        const raster = await applyRasterEdit(await decodePng(source.bytes), edit, controller.signal)
        const bytes = await encodePng(raster)
        if (controller.signal.aborted) return
        url = globalThis.URL.createObjectURL(new globalThis.Blob([bytes.slice()], { type: 'image/png' }))
        setPreview({ url, width: raster.width, height: raster.height })
      } catch (error) {
        if (!controller.signal.aborted)
          setFailure(error instanceof Error && error.message === 'raster-edit-size' ? 'too_large' : 'invalid')
      }
    })()
    return () => {
      controller.abort()
      if (url) globalThis.URL.revokeObjectURL(url)
    }
  }, [edit, source.bytes])
  const locked = busy || applying
  const change = (next: Partial<RasterEdit>, record = true) =>
    update({ ...history.present, edit: { ...edit, ...next } }, record)
  const rotate = (delta: number) => change({ quarterTurns: (edit.quarterTurns + delta + 4) % 4 })
  const corners = edit.corners.map((point) => `${point.x},${point.y}`).join(' ')
  const apply = async () => {
    if (locked || !preview || failure) return
    setApplying(true)
    try {
      await onApply(validateRasterEdit(edit), anglesToSelected)
    } catch {
      setFailure('apply_failed')
    } finally {
      setApplying(false)
    }
  }
  const numericRect = (field: string, value: number) => {
    const rectangle = {
      x: edit.corners[0].x * width,
      y: edit.corners[0].y * height,
      width: (edit.corners[2].x - edit.corners[0].x) * width,
      height: (edit.corners[2].y - edit.corners[0].y) * height
    }
    if (!Number.isFinite(value)) return
    const next = { ...rectangle, [field]: value }
    const fixed = ratioNumber(ratio)
    if (fixed && field === 'width') next.height = value / fixed
    if (fixed && field === 'height') next.width = value * fixed
    change({
      corners: rectangleCorners(next.x / width, next.y / height, next.width / width, next.height / height)
    })
  }
  return (
    <section
      data-ui="document-tools.geometry"
      aria-label={t(P + 'editor')}
      onKeyDown={(event) => {
        if (locked || (event.target instanceof globalThis.Element && event.target.closest('input,textarea,select')))
          return
        if ((event.ctrlKey || event.metaKey) && event.key.toLowerCase() === 'z') {
          event.preventDefault()
          undo()
        }
      }}
      className="flex flex-col gap-4">
      <div className="flex flex-wrap items-center gap-2">
        <button type="button" className={BUTTON} disabled={locked} onClick={() => rotate(-1)}>
          <RotateCcw size={16} strokeWidth={1.7} aria-hidden />
          {t(P + 'left')}
        </button>
        <button type="button" className={BUTTON} disabled={locked} onClick={() => rotate(1)}>
          <RotateCw size={16} strokeWidth={1.7} aria-hidden />
          {t(P + 'right')}
        </button>
        <button type="button" className={BUTTON} disabled={locked || !history.undo.length} onClick={undo}>
          <Undo2 size={16} strokeWidth={1.7} aria-hidden />
          {t(P + 'undo')}
        </button>
        <button
          type="button"
          className={BUTTON}
          disabled={locked}
          onClick={() => update({ edit: ORIGINAL_RASTER_EDIT, mode: 'rectangle', ratio: 'free' })}>
          {t(P + 'restore')}
        </button>
        <span role="status" className="text-[12px]">
          {t(P + 'angle', { angle: edit.quarterTurns * 90 })}
        </span>
      </div>
      <fieldset disabled={locked} className="grid gap-3 text-[13px] sm:grid-cols-3">
        <legend className="sr-only">{t(P + 'settings')}</legend>
        <label className="flex flex-col gap-1.5">
          {t(P + 'crop_mode')}
          <SearchSelect
            searchable={false}
            allowClear={false}
            aria-label={t(P + 'crop_mode')}
            value={mode}
            options={[
              { value: 'rectangle', label: t(P + 'rectangle') },
              { value: 'document', label: t(P + 'document') }
            ]}
            onChange={(raw) => {
              const next = parseCropMode(raw)
              update({
                edit: { ...edit, corners: ORIGINAL_RASTER_EDIT.corners },
                mode: next,
                ratio: 'free'
              })
            }}
          />
        </label>
        {mode === 'rectangle' ? (
          <label className="flex flex-col gap-1.5">
            {t(P + 'ratio')}
            <SearchSelect
              searchable={false}
              allowClear={false}
              aria-label={t(P + 'ratio')}
              value={ratio}
              options={['free', '1:1', '4:3', '16:9'].map((value) => ({
                value: value,
                label: value === 'free' ? t(P + 'free') : value
              }))}
              onChange={(raw) => {
                const next = parseCropRatio(raw)
                update({
                  ...history.present,
                  ratio: next,
                  edit: fitRatio(edit, next, width, height)
                })
              }}
            />
          </label>
        ) : (
          <p className="text-[12px]">{t(P + 'perspective_hint')}</p>
        )}
        <label className="flex flex-col gap-1.5">
          {t(P + 'straighten')}
          <input
            className={FIELD}
            type="number"
            min={-45}
            max={45}
            step={0.1}
            value={edit.straighten}
            onChange={(event) => {
              const value = event.target.valueAsNumber
              if (Number.isFinite(value)) change({ straighten: value })
            }}
          />
        </label>
      </fieldset>
      <p className="text-[12px] text-[var(--spec-body,#3f3f3f)]">{t(P + 'coordinates_hint')}</p>
      <label className="flex items-center gap-3 text-[13px]">
        {t(P + 'zoom')}
        <input
          type="range"
          min={50}
          max={200}
          step={10}
          value={zoom}
          onChange={(event) => setZoom(Number(event.target.value))}
        />
        <span>{zoom}%</span>
      </label>
      <div className="grid min-w-0 gap-3 sm:grid-cols-2">
        <figure className="min-w-0">
          <figcaption className="mb-2 text-[13px]">{t(P + 'original')}</figcaption>
          <div className="overflow-auto rounded-[8px] border border-[var(--spec-card-border,#ececec)] p-3">
            <div ref={plane} className="relative" style={{ width: `${zoom}%` }}>
              <img src={sourceUrl} alt={t(P + 'source_image')} className="block w-full" draggable={false} />
              <svg
                className="pointer-events-none absolute inset-0 h-full w-full"
                viewBox="0 0 1 1"
                preserveAspectRatio="none"
                aria-hidden>
                <path
                  d={`M0,0H1V1H0Z M${edit.corners.map((p) => `${p.x},${p.y}`).join('L')}Z`}
                  fill="black"
                  fillOpacity="0.45"
                  fillRule="evenodd"
                />
                <polygon
                  points={corners}
                  fill="none"
                  stroke="white"
                  strokeWidth="2"
                  vectorEffect="non-scaling-stroke"
                />
              </svg>
              {edit.corners.map((point, index) => (
                <button
                  key={CORNER_KEYS[index]}
                  type="button"
                  disabled={locked}
                  aria-label={t(P + 'corner.' + CORNER_KEYS[index])}
                  data-geometry-corner={index}
                  title={`${Math.round(point.x * width)}, ${Math.round(point.y * height)}`}
                  className="absolute flex size-9 touch-none items-center justify-center rounded-[6px] bg-transparent focus-visible:outline-2"
                  style={{
                    left: `${point.x * 100}%`,
                    top: `${point.y * 100}%`,
                    transform: 'translate(-50%,-50%)'
                  }}
                  onPointerDown={(event) => {
                    if (locked) return
                    drag.current = index
                    update(history.present)
                    event.currentTarget.setPointerCapture?.(event.pointerId)
                  }}
                  onPointerMove={(event) => {
                    if (drag.current !== index || locked) return
                    const bounds = plane.current?.getBoundingClientRect()
                    if (!bounds || !bounds.width || !bounds.height) return
                    const next = {
                      x: Math.max(0, Math.min(1, (event.clientX - bounds.left) / bounds.width)),
                      y: Math.max(0, Math.min(1, (event.clientY - bounds.top) / bounds.height))
                    }
                    change(movedCorner(history.present, index, next, width, height), false)
                  }}
                  onPointerUp={() => {
                    drag.current = null
                  }}
                  onPointerCancel={() => {
                    drag.current = null
                  }}
                  onKeyDown={(event) => {
                    const directions: Record<string, RasterPoint> = {
                      ArrowLeft: { x: -1, y: 0 },
                      ArrowRight: { x: 1, y: 0 },
                      ArrowUp: { x: 0, y: -1 },
                      ArrowDown: { x: 0, y: 1 }
                    }
                    const direction = directions[event.key]
                    if (!direction || locked) return
                    event.preventDefault()
                    const step = event.shiftKey ? 10 : 1
                    change(
                      movedCorner(
                        history.present,
                        index,
                        {
                          x: Math.max(0, Math.min(1, point.x + (direction.x * step) / width)),
                          y: Math.max(0, Math.min(1, point.y + (direction.y * step) / height))
                        },
                        width,
                        height
                      )
                    )
                  }}>
                  <span className="size-3 rounded-[3px] border border-black bg-white" aria-hidden />
                </button>
              ))}
            </div>
          </div>
        </figure>
        <figure className="min-w-0">
          <figcaption className="mb-2 text-[13px]">{t(P + 'result')}</figcaption>
          <div
            className="overflow-auto rounded-[8px] border border-[var(--spec-card-border,#ececec)] p-3"
            data-ui="document-tools.geometry-preview"
            data-state={failure ? 'failed' : preview ? 'ready' : 'running'}>
            {preview ? (
              <>
                <img src={preview.url} alt={t(P + 'result_image')} className="block" style={{ width: `${zoom}%` }} />
                <p className="mt-2 text-[12px]" data-ui="document-tools.geometry-dimensions">
                  {preview.width} × {preview.height} px
                </p>
              </>
            ) : (
              <p role="status" className="text-[13px]">
                {t(P + 'previewing')}
              </p>
            )}
          </div>
        </figure>
      </div>
      <details className="rounded-[8px] border border-[var(--spec-card-border,#ececec)] p-3">
        <summary className="cursor-pointer text-[13px]">{t(P + 'numeric')}</summary>
        <fieldset className="mt-3 grid grid-cols-2 gap-3 sm:grid-cols-4" disabled={locked}>
          <legend className="sr-only">{t(P + 'numeric')}</legend>
          {mode === 'rectangle'
            ? Object.entries({
                x: edit.corners[0].x * width,
                y: edit.corners[0].y * height,
                width: (edit.corners[2].x - edit.corners[0].x) * width,
                height: (edit.corners[2].y - edit.corners[0].y) * height
              }).map(([field, value]) => (
                <label key={field} className="flex flex-col gap-1.5 text-[13px]">
                  {t(P + 'rect.' + field)}
                  <input
                    className={FIELD}
                    type="number"
                    step={1}
                    value={Math.round(value * 100) / 100}
                    min={field === 'width' || field === 'height' ? 1 : 0}
                    max={field === 'x' || field === 'width' ? width : height}
                    aria-invalid={Boolean(failure)}
                    onChange={(event) => numericRect(field, event.target.valueAsNumber)}
                  />
                </label>
              ))
            : edit.corners.flatMap((point, index) =>
                ['x', 'y'].map((axis) => (
                  <label key={`${index}-${axis}`} className="flex flex-col gap-1.5 text-[13px]">
                    {t(P + 'corner.' + CORNER_KEYS[index])} {axis.toUpperCase()}
                    <input
                      className={FIELD}
                      type="number"
                      step={1}
                      min={0}
                      max={axis === 'x' ? width : height}
                      value={Math.round(point[axis as 'x' | 'y'] * (axis === 'x' ? width : height) * 100) / 100}
                      aria-invalid={Boolean(failure)}
                      onChange={(event) => {
                        const value = event.target.valueAsNumber
                        if (!Number.isFinite(value)) return
                        change({
                          corners: edit.corners.map((p, i) =>
                            i === index ? { ...p, [axis]: value / (axis === 'x' ? width : height) } : p
                          ) as unknown as RasterCorners
                        })
                      }}
                    />
                  </label>
                ))
              )}
        </fieldset>
      </details>
      {batch ? (
        <label className="flex min-h-9 items-center gap-2 text-[13px]">
          <input
            type="checkbox"
            checked={anglesToSelected}
            disabled={locked}
            onChange={(event) => setAnglesToSelected(event.target.checked)}
          />
          {t(P + 'selected_angles')}
        </label>
      ) : null}
      {failure ? (
        <p role="alert" className="text-[13px]">
          {t(P + failure)}
        </p>
      ) : null}
      <button
        type="button"
        className={`${BUTTON} w-fit !bg-[var(--spec-ink,#171717)] !text-white`}
        disabled={
          locked || !preview || Boolean(failure) || (accepted && rasterEditsEqual(edit, committed) && !anglesToSelected)
        }
        onClick={() => void apply()}>
        {t(P + (applying ? 'applying' : 'apply'))}
      </button>
    </section>
  )
}

export function PdfRotationEditor({
  id,
  histories,
  busy,
  onApply,
  onDraft
}: {
  id: string
  histories: Map<string, RasterEditorHistory>
  busy: boolean
  onApply: (edit: RasterEdit, anglesToSelected: boolean) => Promise<void>
  onDraft: (edit: RasterEdit) => void
}) {
  const { t } = useTranslation()
  const { history, update, undo } = useHistory(id, histories)
  useEffect(() => {
    onDraft(history.present.edit)
  }, [history.present.edit, onDraft])
  const [failed, setFailed] = useState(false)
  const rotate = (delta: number) =>
    update({
      ...history.present,
      edit: {
        ...ORIGINAL_RASTER_EDIT,
        quarterTurns: (history.present.edit.quarterTurns + delta + 4) % 4
      }
    })
  return (
    <section className="flex flex-col gap-3" aria-label={t(P + 'editor')}>
      <p className="text-[13px]">{t(P + 'pdf_rotation_only')}</p>
      <div className="flex flex-wrap items-center gap-2">
        <button className={BUTTON} type="button" disabled={busy} onClick={() => rotate(-1)}>
          {t(P + 'left')}
        </button>
        <button className={BUTTON} type="button" disabled={busy} onClick={() => rotate(1)}>
          {t(P + 'right')}
        </button>
        <button className={BUTTON} type="button" disabled={busy || !history.undo.length} onClick={undo}>
          {t(P + 'undo')}
        </button>
        <button
          className={BUTTON}
          type="button"
          disabled={busy}
          onClick={() => update({ edit: ORIGINAL_RASTER_EDIT, mode: 'rectangle', ratio: 'free' })}>
          {t(P + 'restore')}
        </button>
        <span role="status" className="text-[12px]">
          {t(P + 'angle', { angle: history.present.edit.quarterTurns * 90 })}
        </span>
        <button
          className={BUTTON}
          type="button"
          disabled={busy}
          onClick={() => {
            setFailed(false)
            void onApply(history.present.edit, false).catch(() => setFailed(true))
          }}>
          {t(P + 'apply')}
        </button>
      </div>
      {failed ? <p role="alert">{t(P + 'apply_failed')}</p> : null}
    </section>
  )
}
