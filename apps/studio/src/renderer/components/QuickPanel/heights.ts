/** Visible height of one QuickPanel row in pixels. */
export const QUICK_PANEL_ROW_HEIGHT = 34

/** Row height plus the one-pixel bottom gap used by the list. */
export const QUICK_PANEL_ITEM_HEIGHT = QUICK_PANEL_ROW_HEIGHT + 1

/** Safe gap between the panel and the frame edge it opens toward, in pixels. */
export const QUICK_PANEL_SAFE_MARGIN = 8

/** Gap between the panel and the input it is anchored to. */
export const QUICK_PANEL_ANCHOR_GAP = 4

/** Width of an `@` menu. It hugs the caret instead of stretching across the input. */
export const MENTION_PANEL_WIDTH = 320

export interface QuickPanelRect {
  left: number
  top: number
  right: number
  bottom: number
}

export interface QuickPanelPlacement {
  placement: 'top' | 'bottom'
  /** Height the panel may use on the chosen side before it would leave the boundary. */
  availableHeight: number
  left: number
  width: number
  /** Viewport Y when the panel opens below the anchor. */
  top: number
  /** CSS `bottom` when the panel opens above, so its bottom edge stays on the anchor. */
  bottom: number
}

/**
 * Pins a mention panel to the input and keeps the box inside the boundary.
 * Width matches the input and never grows past it. The side with room wins;
 * the box shifts inward when the input meets the frame edge.
 */
export function placeQuickPanel(input: {
  anchor: QuickPanelRect
  boundary: QuickPanelRect
  viewportWidth: number
  viewportHeight: number
  desiredHeight: number
  margin?: number
  gap?: number
  /** Used when the anchor is a caret. The box stays this wide and shifts to stay inside the frame. */
  preferredWidth?: number
}): QuickPanelPlacement {
  const margin = input.margin ?? QUICK_PANEL_SAFE_MARGIN
  const gap = input.gap ?? QUICK_PANEL_ANCHOR_GAP
  const boundLeft = Math.max(input.boundary.left, 0) + margin
  const boundRight = Math.min(input.boundary.right, input.viewportWidth) - margin
  const boundTop = Math.max(input.boundary.top, 0) + margin
  const boundBottom = Math.min(input.boundary.bottom, input.viewportHeight) - margin

  const anchorWidth = Math.max(0, input.anchor.right - input.anchor.left)
  const caretMenu = typeof input.preferredWidth === 'number' && input.preferredWidth > anchorWidth
  let left = input.anchor.left
  let width = caretMenu ? input.preferredWidth! : anchorWidth
  if (boundRight > boundLeft) {
    if (caretMenu && left + width > boundRight) left = Math.max(boundLeft, boundRight - width)
    if (left < boundLeft) {
      width -= boundLeft - left
      left = boundLeft
    }
    if (left + width > boundRight) width = boundRight - left
    width = Math.max(0, caretMenu ? width : Math.min(width, anchorWidth))
  }

  const above = Math.max(0, input.anchor.top - gap - boundTop)
  const below = Math.max(0, boundBottom - input.anchor.bottom - gap)
  const placement = above >= input.desiredHeight || above >= below ? 'top' : 'bottom'
  return {
    placement,
    availableHeight: placement === 'top' ? above : below,
    left,
    width,
    top: input.anchor.bottom + gap,
    bottom: Math.max(0, input.viewportHeight - (input.anchor.top - gap))
  }
}

/** Default non-list panel chrome height: footer, header, and padding. */
const READONLY_CHROME_HEIGHT = 50
export const QUICK_PANEL_DEFAULT_CHROME_HEIGHT = 98
const DEFAULT_CHROME_HEIGHT = QUICK_PANEL_DEFAULT_CHROME_HEIGHT

/** Reads the rendered body padding and border instead of assuming a device-pixel rounding mode. */
export function getQuickPanelBodyVerticalSpace(style: CSSStyleDeclaration): number {
  return [style.paddingTop, style.paddingBottom, style.borderTopWidth, style.borderBottomWidth].reduce(
    (total, value) => total + (Number.parseFloat(value) || 0),
    0
  )
}

export interface QuickPanelHeightOptions {
  isVisible: boolean
  collapsed: boolean
  readOnly: boolean
  pageSize: number
  itemCount: number
  /** Available height cap above the input; only used for fill/home placement. */
  availableHeight: number | null
  /** Home placement is capped by available height; other placements keep the fixed height. */
  fill?: boolean
  /** Runtime-measured footer plus body chrome height for home/fill; docked/readOnly use defaults. */
  chromeHeight?: number
  /** Runtime-measured empty-state height when the searchable list has no matches. */
  emptyStateHeight?: number
}

export interface QuickPanelHeights {
  /** Outer panel maxHeight; also used as explicit body height when home content overflows. */
  panelMaxHeight: number
  /** Virtual list scroller size: fits content, or shrinks for internal scrolling when fill space is tight. */
  listHeight: number
}

/**
 * Calculates QuickPanel panel and list heights.
 *
 * - fill/home: use content height while it fits; cap to available frame height and scroll the list when it overflows.
 * - default/docked: keep the original fixed height and ignore availableHeight.
 */
export function getQuickPanelHeights({
  isVisible,
  collapsed,
  readOnly,
  pageSize,
  itemCount,
  availableHeight,
  fill = false,
  chromeHeight: measuredChromeHeight,
  emptyStateHeight = 0
}: QuickPanelHeightOptions): QuickPanelHeights {
  const defaultChromeHeight = readOnly ? READONLY_CHROME_HEIGHT : DEFAULT_CHROME_HEIGHT
  const chromeHeight = (fill || readOnly) && measuredChromeHeight != null ? measuredChromeHeight : defaultChromeHeight

  if (!isVisible) return { panelMaxHeight: 0, listHeight: 0 }
  if (collapsed) return { panelMaxHeight: chromeHeight + emptyStateHeight, listHeight: 0 }

  const listContentHeight = Math.min(pageSize, itemCount) * QUICK_PANEL_ITEM_HEIGHT
  const contentHeight = chromeHeight + listContentHeight

  if (fill && availableHeight != null) {
    const minimumPanelHeight = chromeHeight + (itemCount > 0 ? QUICK_PANEL_ITEM_HEIGHT : 0)
    const panelMaxHeight = Math.max(minimumPanelHeight, Math.min(contentHeight, availableHeight))
    const listHeight = Math.min(listContentHeight, Math.max(0, panelMaxHeight - chromeHeight))
    return { panelMaxHeight, listHeight }
  }

  return {
    panelMaxHeight: pageSize * QUICK_PANEL_ITEM_HEIGHT + chromeHeight,
    listHeight: listContentHeight
  }
}
