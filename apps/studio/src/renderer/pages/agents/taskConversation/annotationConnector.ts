export interface ReviewAnnotationLink {
  id: string
  d: string
  startX: number
  startY: number
  endX: number
  endY: number
  active: boolean
}

/** Smooth curve from a document quote leftward onto the finding card. */
export function reviewAnnotationConnector(
  card: DOMRect,
  mark: DOMRect
): Pick<ReviewAnnotationLink, 'd' | 'startX' | 'startY' | 'endX' | 'endY'> {
  const startX = mark.left
  const startY = mark.top + mark.height / 2
  const endX = card.right
  const endY = card.top + Math.min(card.height / 2, 22)
  const pull = Math.max(48, Math.abs(startX - endX) * 0.45)
  const leaveX = startX - pull
  const arriveX = endX + pull
  return {
    startX,
    startY,
    endX,
    endY,
    d: `M ${startX} ${startY} C ${leaveX} ${startY}, ${arriveX} ${endY}, ${endX} ${endY}`
  }
}

const HEADER_SELECTOR = '[data-workspace-multitab] > header, [data-workspace-multitab] .work-office-ribbon'

/** Surfaces the curve must not paint over: workspace chrome, the settings sheet, and the composer. */
const OCCLUDER_SELECTOR = `${HEADER_SELECTOR}, [data-ui="settings.overlay"], [data-ui="workbench.composer"], [data-composer-dock-surface], [data-composer-inputbar]`

/** Tab bar plus the office ribbon. The curve must not paint over this chrome. */
export function workspaceHeaderRect(root: ParentNode): DOMRect | null {
  return unionClientRects([...root.querySelectorAll(HEADER_SELECTOR)].map((node) => node.getBoundingClientRect()))
}

/** Each rect is a hole in the annotation layer. */
export function annotationOccluders(root: ParentNode): DOMRect[] {
  return [...root.querySelectorAll(OCCLUDER_SELECTOR)]
    .map((node) => node.getBoundingClientRect())
    .filter((rect) => rect.width > 0 && rect.height > 0)
}

export function unionClientRects(rects: DOMRect[]): DOMRect | null {
  const visible = rects.filter((rect) => rect.width > 0 && rect.height > 0)
  if (visible.length === 0) return null
  const left = Math.min(...visible.map((rect) => rect.left))
  const top = Math.min(...visible.map((rect) => rect.top))
  const right = Math.max(...visible.map((rect) => rect.right))
  const bottom = Math.max(...visible.map((rect) => rect.bottom))
  return new DOMRect(left, top, right - left, bottom - top)
}

export function measureReviewAnnotationLinks(root: ParentNode, activeId: string | null): ReviewAnnotationLink[] {
  const links: ReviewAnnotationLink[] = []
  for (const card of root.querySelectorAll<HTMLElement>('[data-chat-review-finding-id]')) {
    const id = card.dataset.chatReviewFindingId
    if (!id) continue
    const marks = [
      ...root.querySelectorAll<HTMLElement>(`[data-review-office-document] [data-comment-id="${cssEscape(id)}"]`)
    ]
    let markRect: DOMRect | null = null
    for (const mark of marks) {
      const rect = visibleMarkRect(mark)
      if (!rect) continue
      if (!markRect || rect.width * rect.height > markRect.width * markRect.height) markRect = rect
    }
    const cardRect = card.getBoundingClientRect()
    if (!markRect || !isVisible(cardRect, scrollFrame(card))) continue
    links.push({ id, active: id === activeId, ...reviewAnnotationConnector(cardRect, markRect) })
  }
  return links
}

function visibleMarkRect(mark: HTMLElement): DOMRect | null {
  const editable = mark.closest('.work-document-editable')
  if (!editable || !isShown(mark)) return null
  const frame = editable.getBoundingClientRect()
  const rects = [...mark.getClientRects()].filter(
    (rect) => rect.width >= 8 && rect.height >= 8 && rect.height <= 40 && isVisible(rect, frame)
  )
  if (rects.length === 0) return null
  return rects.reduce((top, rect) => (rect.top < top.top ? rect : top))
}

function isShown(node: Element): boolean {
  const view = node.ownerDocument.defaultView
  if (!view) return false
  let current: Element | null = node
  while (current) {
    const style = view.getComputedStyle(current)
    if (style.display === 'none' || style.visibility === 'hidden' || Number(style.opacity) === 0) return false
    current = current.parentElement
  }
  return true
}

function isVisible(rect: DOMRect, frame: DOMRect): boolean {
  return rect.bottom > frame.top && rect.top < frame.bottom && rect.right > frame.left && rect.left < frame.right
}

function scrollFrame(node: Element): DOMRect {
  const view = node.ownerDocument.defaultView
  let current = node.parentElement
  while (current && view && current !== node.ownerDocument.body) {
    const style = view.getComputedStyle(current)
    if (
      (style.overflowY === 'auto' || style.overflowY === 'scroll') &&
      current.scrollHeight > current.clientHeight + 1
    ) {
      return current.getBoundingClientRect()
    }
    current = current.parentElement
  }
  return new DOMRect(0, 0, view?.innerWidth ?? 0, view?.innerHeight ?? 0)
}

function cssEscape(value: string): string {
  return typeof CSS !== 'undefined' && typeof CSS.escape === 'function'
    ? CSS.escape(value)
    : value.replace(/["\\]/g, '\\$&')
}
