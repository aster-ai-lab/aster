import { describe, expect, it } from 'vitest'

import {
  getQuickPanelBodyVerticalSpace,
  getQuickPanelHeights,
  MENTION_PANEL_WIDTH,
  placeQuickPanel,
  QUICK_PANEL_ITEM_HEIGHT
} from '../heights'

const ITEM = QUICK_PANEL_ITEM_HEIGHT
const DEFAULT_CHROME = 98
const READONLY_CHROME = 50

const base = {
  isVisible: true,
  collapsed: false,
  readOnly: false,
  pageSize: 7,
  itemCount: 10,
  availableHeight: null as number | null,
  fill: false
}

describe('getQuickPanelHeights', () => {
  it('accounts for the 34px rendered row plus its one-pixel inter-row gap', () => {
    expect(QUICK_PANEL_ITEM_HEIGHT).toBe(35)
  })

  it('sums the rendered body padding and border widths', () => {
    const style = {
      paddingTop: '5px',
      paddingBottom: '5px',
      borderTopWidth: '0.5px',
      borderBottomWidth: '0.5px'
    } as CSSStyleDeclaration

    expect(getQuickPanelBodyVerticalSpace(style)).toBe(11)
  })

  describe('default (docked / non-fill): fixed height, ignores availableHeight', () => {
    it('uses the fixed ideal height when availableHeight is null', () => {
      const { panelMaxHeight, listHeight } = getQuickPanelHeights(base)
      expect(panelMaxHeight).toBe(base.pageSize * ITEM + DEFAULT_CHROME)
      expect(listHeight).toBe(Math.min(base.pageSize, base.itemCount) * ITEM)
    })

    it('still uses the fixed height even when availableHeight is small (docked has no top clipping)', () => {
      const { panelMaxHeight, listHeight } = getQuickPanelHeights({ ...base, availableHeight: 120 })
      expect(panelMaxHeight).toBe(base.pageSize * ITEM + DEFAULT_CHROME)
      expect(listHeight).toBe(base.pageSize * ITEM)
    })

    it('caps the list height to the actual item count', () => {
      expect(getQuickPanelHeights({ ...base, itemCount: 2 }).listHeight).toBe(2 * ITEM)
    })

    it('uses the read-only chrome height', () => {
      expect(getQuickPanelHeights({ ...base, readOnly: true }).panelMaxHeight).toBe(
        base.pageSize * ITEM + READONLY_CHROME
      )
    })

    it('keeps a measured read-only footer visible when search results collapse', () => {
      const measuredChrome = 82

      expect(
        getQuickPanelHeights({
          ...base,
          readOnly: true,
          collapsed: true,
          chromeHeight: measuredChrome
        })
      ).toEqual({ panelMaxHeight: measuredChrome, listHeight: 0 })
    })

    it('includes the rendered empty state when search results collapse', () => {
      const measuredChrome = 82
      const emptyStateHeight = 48

      expect(
        getQuickPanelHeights({
          ...base,
          readOnly: true,
          collapsed: true,
          chromeHeight: measuredChrome,
          emptyStateHeight
        })
      ).toEqual({ panelMaxHeight: measuredChrome + emptyStateHeight, listHeight: 0 })
    })
  })

  describe('fill (welcome / placement=home): panel prefers content height until it exceeds the available space', () => {
    it('uses content height instead of filling the available space for few items', () => {
      const available = 400
      const { panelMaxHeight, listHeight } = getQuickPanelHeights({
        ...base,
        fill: true,
        itemCount: 2,
        availableHeight: available
      })
      expect(panelMaxHeight).toBe(DEFAULT_CHROME + 2 * ITEM)
      expect(panelMaxHeight).not.toBe(available)
      expect(listHeight).toBe(2 * ITEM)
    })

    it('uses content height when pageSize-capped content still fits', () => {
      const available = 400
      const { panelMaxHeight, listHeight } = getQuickPanelHeights({
        ...base,
        fill: true,
        itemCount: 50,
        availableHeight: available
      })
      expect(panelMaxHeight).toBe(DEFAULT_CHROME + base.pageSize * ITEM)
      expect(panelMaxHeight).not.toBe(available)
      expect(listHeight).toBe(base.pageSize * ITEM)
    })

    it('caps the panel at the available height and shrinks the list when content overflows', () => {
      const available = DEFAULT_CHROME + 3 * ITEM
      const { panelMaxHeight, listHeight } = getQuickPanelHeights({
        ...base,
        fill: true,
        itemCount: 50,
        availableHeight: available
      })
      expect(panelMaxHeight).toBe(available)
      expect(listHeight).toBe(3 * ITEM)
    })

    it('uses measured chrome height when capping a home panel', () => {
      const measuredChrome = 42
      const available = measuredChrome + 3 * ITEM
      const { panelMaxHeight, listHeight } = getQuickPanelHeights({
        ...base,
        fill: true,
        itemCount: 50,
        availableHeight: available,
        chromeHeight: measuredChrome
      })
      expect(panelMaxHeight).toBe(available)
      expect(listHeight).toBe(3 * ITEM)
    })

    it('never shrinks the panel below one row of chrome when the available space is tiny', () => {
      expect(getQuickPanelHeights({ ...base, fill: true, itemCount: 1, availableHeight: 10 }).panelMaxHeight).toBe(
        DEFAULT_CHROME + ITEM
      )
    })

    it('stays compact (chrome only) when collapsed even in fill mode', () => {
      const { panelMaxHeight, listHeight } = getQuickPanelHeights({
        ...base,
        fill: true,
        collapsed: true,
        availableHeight: 400
      })
      expect(panelMaxHeight).toBe(DEFAULT_CHROME)
      expect(listHeight).toBe(0)
    })
  })

  it('returns zeros when the panel is hidden', () => {
    expect(getQuickPanelHeights({ ...base, isVisible: false })).toEqual({ panelMaxHeight: 0, listHeight: 0 })
  })
})

describe('placeQuickPanel', () => {
  const boundary = { left: 0, top: 0, right: 800, bottom: 900 }
  const viewport = { viewportWidth: 800, viewportHeight: 900 }

  it('matches the input width and opens above when that side fits', () => {
    const frame = placeQuickPanel({
      anchor: { left: 120, top: 500, right: 520, bottom: 640 },
      boundary,
      ...viewport,
      desiredHeight: 200
    })

    expect(frame.placement).toBe('top')
    expect(frame.left).toBe(120)
    expect(frame.width).toBe(400)
    expect(frame.availableHeight).toBe(500 - 4 - 8)
    expect(frame.left + frame.width).toBeLessThanOrEqual(boundary.right - 8)
  })

  it('shifts and shrinks the box that would cross the frame', () => {
    const left = placeQuickPanel({
      anchor: { left: -20, top: 200, right: 180, bottom: 280 },
      boundary: { left: 0, top: 0, right: 400, bottom: 600 },
      viewportWidth: 400,
      viewportHeight: 600,
      desiredHeight: 80
    })
    expect(left.left).toBe(8)
    expect(left.width).toBe(172)
    expect(left.left + left.width).toBeLessThanOrEqual(392)

    const right = placeQuickPanel({
      anchor: { left: 300, top: 200, right: 500, bottom: 280 },
      boundary: { left: 0, top: 0, right: 400, bottom: 600 },
      viewportWidth: 400,
      viewportHeight: 600,
      desiredHeight: 80
    })
    expect(right.left).toBe(300)
    expect(right.width).toBe(92)
    expect(right.left + right.width).toBe(392)
    expect(right.width).toBeLessThanOrEqual(200)
  })

  it('hugs a caret and keeps a mention menu inside the frame', () => {
    const frame = placeQuickPanel({
      anchor: { left: 180, top: 640, right: 180, bottom: 660 },
      boundary,
      ...viewport,
      desiredHeight: 120,
      preferredWidth: MENTION_PANEL_WIDTH
    })
    expect(frame.placement).toBe('top')
    expect(frame.left).toBe(180)
    expect(frame.width).toBe(MENTION_PANEL_WIDTH)
    expect(frame.bottom).toBe(900 - (640 - 4))

    const shifted = placeQuickPanel({
      anchor: { left: 700, top: 640, right: 700, bottom: 660 },
      boundary,
      ...viewport,
      desiredHeight: 120,
      preferredWidth: MENTION_PANEL_WIDTH
    })
    expect(shifted.left).toBe(800 - 8 - MENTION_PANEL_WIDTH)
    expect(shifted.width).toBe(MENTION_PANEL_WIDTH)
  })

  it('opens below the input when only that side fits', () => {
    const frame = placeQuickPanel({
      anchor: { left: 40, top: 20, right: 440, bottom: 80 },
      boundary,
      ...viewport,
      desiredHeight: 400
    })

    expect(frame.placement).toBe('bottom')
    expect(frame.top).toBe(84)
    expect(frame.availableHeight).toBe(900 - 8 - 80 - 4)
  })
})
