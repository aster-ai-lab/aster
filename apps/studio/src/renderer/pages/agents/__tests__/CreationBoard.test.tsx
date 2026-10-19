import { fireEvent, render, screen } from '@testing-library/react'
import { beforeEach, describe, expect, it, vi } from 'vitest'

vi.mock('react-i18next', () => ({
  initReactI18next: { type: '3rdParty', init: vi.fn() },
  useTranslation: () => ({
    t: (key: string) => key
  })
}))

import { CreationBoard } from '../CreationBoard'
import { CREATE_BOARD_STORAGE_KEY } from '../creationBoardModel'

function renderBoard(kind: 'center' | 'ppt' | 'site' | 'video' | 'image' = 'center') {
  const onClose = vi.fn()
  const view = render(<CreationBoard kind={kind} onClose={onClose} />)
  const canvas = document.querySelector('[data-ui="workbench.create.canvas"]') as HTMLDivElement
  canvas.getBoundingClientRect = () => ({
    x: 0,
    y: 0,
    left: 0,
    top: 0,
    right: 800,
    bottom: 600,
    width: 800,
    height: 600,
    toJSON() {
      return {}
    }
  })
  return { ...view, onClose, canvas }
}

function pointer(target: Element, type: 'pointerDown' | 'pointerMove' | 'pointerUp', x: number, y: number) {
  fireEvent[type](target, { clientX: x, clientY: y, button: 0, pointerId: 1 })
}

class TestPointerEvent extends MouseEvent {
  pointerId: number

  constructor(type: string, init: MouseEventInit & { pointerId?: number } = {}) {
    super(type, init)
    this.pointerId = init.pointerId ?? 1
  }
}

describe('CreationBoard', () => {
  beforeEach(() => {
    localStorage.removeItem(CREATE_BOARD_STORAGE_KEY)
    if (typeof window.PointerEvent !== 'function') {
      Object.defineProperty(window, 'PointerEvent', { configurable: true, writable: true, value: TestPointerEvent })
    }
    HTMLElement.prototype.setPointerCapture ??= () => {}
    HTMLElement.prototype.releasePointerCapture ??= () => {}
  })

  it('opens an empty free board and places, edits, recolors, and undoes a note', () => {
    const { canvas, unmount } = renderBoard('center')
    expect(document.querySelector('[data-ui="workbench.create.empty"]')).toHaveTextContent('workbench.create.empty')
    expect(screen.getByRole('button', { name: 'workbench.create.select' })).toHaveAttribute('aria-pressed', 'true')

    fireEvent.click(screen.getByRole('button', { name: 'workbench.create.sticky' }))
    pointer(canvas, 'pointerDown', 400, 300)
    pointer(canvas, 'pointerUp', 400, 300)

    const field = screen.getByRole('textbox', { name: 'workbench.create.sticky_hint' })
    fireEvent.change(field, { target: { value: '开场一句' } })
    fireEvent.blur(field)
    expect(document.querySelector('[data-node-kind="sticky"]')).toHaveTextContent('开场一句')

    fireEvent.click(screen.getByRole('button', { name: 'workbench.create.swatch.blue' }))
    expect(document.querySelector('[data-node-kind="sticky"]')).toHaveStyle({ backgroundColor: '#d9e8ff' })
    fireEvent.click(screen.getByRole('button', { name: 'workbench.create.undo' }))
    expect(document.querySelector('[data-node-kind="sticky"]')).toHaveStyle({ backgroundColor: '#fff4c2' })
    fireEvent.click(screen.getByRole('button', { name: 'workbench.create.undo' }))
    expect(document.querySelector('[data-node-kind="sticky"]')).not.toHaveTextContent('开场一句')

    unmount()
    renderBoard('center')
    const kept = document.querySelector('[data-node-kind="sticky"]')
    expect(kept).toBeTruthy()
    expect(kept).not.toHaveTextContent('开场一句')
  })

  it('drags a note, deletes it, and draws a line', () => {
    const { canvas } = renderBoard('center')
    fireEvent.click(screen.getByRole('button', { name: 'workbench.create.sticky' }))
    pointer(canvas, 'pointerDown', 400, 300)
    pointer(canvas, 'pointerUp', 400, 300)
    fireEvent.click(screen.getByRole('button', { name: 'workbench.create.select' }))

    const before = Number(document.querySelector('[data-node-kind="sticky"]')?.getAttribute('data-x'))
    const note = document.querySelector('[data-node-kind="sticky"]') as HTMLElement
    pointer(note, 'pointerDown', 400, 300)
    pointer(canvas, 'pointerMove', 460, 320)
    pointer(canvas, 'pointerUp', 460, 320)
    expect(Number(document.querySelector('[data-node-kind="sticky"]')?.getAttribute('data-x'))).toBe(before + 60)

    fireEvent.keyDown(canvas, { key: 'Delete' })
    expect(document.querySelector('[data-node-kind="sticky"]')).toBeNull()

    fireEvent.click(screen.getByRole('button', { name: 'workbench.create.line' }))
    pointer(canvas, 'pointerDown', 120, 120)
    pointer(canvas, 'pointerMove', 280, 200)
    pointer(canvas, 'pointerUp', 280, 200)
    expect(document.querySelector('[data-node-kind="connector"]')).toBeTruthy()
    fireEvent.click(screen.getByRole('button', { name: 'workbench.create.zoom_in' }))
    expect(document.querySelector('[data-ui="workbench.create.board"]')).toHaveAttribute('data-zoom', '1.15')
  })

  it('starts a deck, a page, a storyboard, and a picture from the matching entry', () => {
    const { unmount, onClose } = renderBoard('ppt')
    expect(document.querySelectorAll('[data-node-kind="frame"]')).toHaveLength(4)
    expect(document.querySelector('[data-ui="workbench.create.board"]')).toHaveAttribute('data-kind', 'ppt')
    fireEvent.click(screen.getByRole('button', { name: 'workbench.create.back' }))
    expect(onClose).toHaveBeenCalledOnce()
    unmount()

    const site = renderBoard('site')
    expect(screen.getByText('workbench.create.seed.site.hero')).toBeTruthy()
    site.unmount()
    const video = renderBoard('video')
    expect(document.querySelectorAll('[data-node-kind="frame"]')).toHaveLength(4)
    video.unmount()
    renderBoard('image')
    expect(document.querySelectorAll('[data-node-kind="shape"]')).toHaveLength(3)
  })
})
