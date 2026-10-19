// @vitest-environment jsdom
import '@testing-library/jest-dom/vitest'
import { readFileSync } from 'node:fs'
import { dirname, join } from 'node:path'
import { fileURLToPath } from 'node:url'

import { cleanup, fireEvent, render, screen, waitFor } from '@testing-library/react'
import { useState } from 'react'
import { afterEach, beforeAll, describe, expect, it, vi } from 'vitest'

import { DIALOG_UNMOUNT_DELAY_MS } from '../../../utils'
import { Dialog, DIALOG_CLOSE_DURATION_MS, DialogContent, DialogTitle } from '../dialog'
import { DialogPortalContainerProvider } from '../portal-container'
import { Select, SelectContent, SelectItem, SelectTrigger, SelectValue } from '../select'

beforeAll(() => {
  if (!HTMLElement.prototype.hasPointerCapture) {
    HTMLElement.prototype.hasPointerCapture = () => false
  }
  if (!HTMLElement.prototype.releasePointerCapture) {
    HTMLElement.prototype.releasePointerCapture = () => {}
  }
  if (!HTMLElement.prototype.setPointerCapture) {
    HTMLElement.prototype.setPointerCapture = () => {}
  }
  HTMLElement.prototype.scrollIntoView = () => {}
})
afterEach(() => {
  cleanup()
  vi.restoreAllMocks()
})
function DialogWithSelect({ onOpenChange }: { onOpenChange: (open: boolean) => void }) {
  const [selectOpen, setSelectOpen] = useState(false)

  return (
    <Dialog open onOpenChange={onOpenChange}>
      <DialogContent aria-describedby={undefined}>
        <DialogTitle>Configure item</DialogTitle>
        <input aria-label="Name" />
        <Select open={selectOpen} value="alpha" onOpenChange={setSelectOpen}>
          <SelectTrigger aria-label="Mode">
            <SelectValue />
          </SelectTrigger>
          <SelectContent>
            <SelectItem value="alpha">Alpha</SelectItem>
            <SelectItem value="beta">Beta</SelectItem>
          </SelectContent>
        </Select>
      </DialogContent>
    </Dialog>
  )
}

describe('Dialog primitive', () => {
  it('uses the 12px dialog panel and lets a square override replace it', () => {
    render(
      <>
        <Dialog open>
          <DialogContent aria-describedby={undefined}>
            <DialogTitle>Default panel</DialogTitle>
          </DialogContent>
        </Dialog>
        <Dialog open>
          <DialogContent aria-describedby={undefined} className="rounded-none">
            <DialogTitle>Square panel</DialogTitle>
          </DialogContent>
        </Dialog>
      </>
    )

    const panel = screen.getByText('Default panel').closest('[data-slot="dialog-content"]')
    const square = screen.getByText('Square panel').closest('[data-slot="dialog-content"]')
    expect(panel).toHaveClass('rounded-[12px]')
    expect(panel).not.toHaveClass('rounded-3xl')
    expect(square).toHaveClass('rounded-none')
    expect(square).not.toHaveClass('rounded-[12px]')
  })

  it('keeps nested dialogs in the page dialog portal instead of the parent dialog content', () => {
    const pagePortalContainer = document.createElement('div')
    document.body.appendChild(pagePortalContainer)

    render(
      <DialogPortalContainerProvider container={pagePortalContainer}>
        <Dialog open>
          <DialogContent aria-describedby={undefined}>
            <DialogTitle>Outer dialog</DialogTitle>
            <Dialog open>
              <DialogContent aria-describedby={undefined}>
                <DialogTitle>Inner dialog</DialogTitle>
              </DialogContent>
            </Dialog>
          </DialogContent>
        </Dialog>
      </DialogPortalContainerProvider>
    )

    const outerContent = screen.getByText('Outer dialog').closest('[data-slot="dialog-content"]')
    const innerContent = screen.getByText('Inner dialog').closest('[data-slot="dialog-content"]')

    expect(outerContent?.parentElement).toBe(pagePortalContainer)
    expect(innerContent?.parentElement).toBe(pagePortalContainer)
    expect(innerContent?.parentElement).not.toBe(outerContent)

    pagePortalContainer.remove()
  })

  it('keeps the declared close and imperative-unmount durations synchronized', () => {
    render(
      <Dialog open>
        <DialogContent aria-describedby={undefined}>
          <DialogTitle>Rename item</DialogTitle>
        </DialogContent>
      </Dialog>
    )

    const content = document.querySelector('[data-slot="dialog-content"]')
    const motionCss = readFileSync(
      join(dirname(fileURLToPath(import.meta.url)), '../../../styles/dialog-motion.css'),
      'utf8'
    )
    const dialogSource = readFileSync(join(dirname(fileURLToPath(import.meta.url)), '../dialog.tsx'), 'utf8')
    expect(content).not.toBeNull()
    expect(motionCss).toContain(`a3s-dialog-out ${DIALOG_CLOSE_DURATION_MS}ms`)
    expect(motionCss).toContain('a3s-dialog-in 160ms')
    expect(motionCss).not.toMatch(/transform:|slide-in-from-bottom|filter:/)
    expect(dialogSource).not.toMatch(/slide-in-from-bottom|slide-out-to-bottom|animate-in|zoom-in-/)
    expect(DIALOG_CLOSE_DURATION_MS).toBe(200)
    expect(DIALOG_UNMOUNT_DELAY_MS).toBe(DIALOG_CLOSE_DURATION_MS)
  })

  it('stays in place and fades with scale instead of traveling from the bottom', () => {
    render(
      <Dialog open>
        <DialogContent aria-describedby={undefined}>
          <DialogTitle>Rename item</DialogTitle>
        </DialogContent>
      </Dialog>
    )

    const overlay = document.querySelector('[data-slot="dialog-overlay"]')
    const content = document.querySelector('[data-slot="dialog-content"]')

    expect(overlay).toHaveClass('motion-reduce:animate-none')
    expect(content).toHaveClass('motion-reduce:animate-none')
    expect(content?.className ?? '').not.toMatch(/slide-in-from-bottom|slide-out-to-bottom|zoom-in-|animate-in/)
    expect(overlay?.className ?? '').not.toMatch(/slide-in-from-bottom|animate-in/)
  })

  it('keeps a requested directional motion in place', () => {
    render(
      <Dialog open>
        <DialogContent motion="directional" aria-describedby={undefined}>
          <DialogTitle>Confirm action</DialogTitle>
        </DialogContent>
      </Dialog>
    )

    const content = document.querySelector('[data-slot="dialog-content"]')

    expect(content).toHaveAttribute('data-dialog-motion', 'directional')
    expect(content?.className ?? '').not.toMatch(/slide-in-from-bottom|slide-out-to-bottom/)
  })

  it('marks dialog content as no-drag so it stays clickable over titlebar drag regions', () => {
    render(
      <Dialog open>
        <DialogContent aria-describedby={undefined}>
          <DialogTitle>Rename item</DialogTitle>
        </DialogContent>
      </Dialog>
    )

    const content = document.querySelector('[data-slot="dialog-content"]')
    expect(content).toHaveClass('[-webkit-app-region:no-drag]')
  })

  it('keeps content mounted while its close animation is active', () => {
    const originalGetComputedStyle = window.getComputedStyle
    vi.spyOn(window, 'getComputedStyle').mockImplementation((element) => {
      const styles = originalGetComputedStyle(element)
      return new Proxy(styles, {
        get(target, property, receiver) {
          if (property === 'animationName') {
            return element.getAttribute('data-state') === 'closed' ? 'exit' : 'enter'
          }
          if (property === 'display') return 'grid'
          return Reflect.get(target, property, receiver)
        }
      })
    })

    const { rerender } = render(
      <Dialog open>
        <DialogContent aria-describedby={undefined}>
          <DialogTitle>Animated dialog</DialogTitle>
        </DialogContent>
      </Dialog>
    )

    const content = screen.getByText('Animated dialog').closest('[data-slot="dialog-content"]')
    expect(content).toBeInTheDocument()

    rerender(
      <Dialog open={false}>
        <DialogContent aria-describedby={undefined}>
          <DialogTitle>Animated dialog</DialogTitle>
        </DialogContent>
      </Dialog>
    )

    expect(content).toBeInTheDocument()
    expect(content).toHaveAttribute('data-state', 'closed')
  })

  it('stops pointerdown events inside content from reaching React ancestors', () => {
    const handleAncestorPointerDown = vi.fn()

    render(
      <div onPointerDown={handleAncestorPointerDown}>
        <Dialog open>
          <DialogContent aria-describedby={undefined}>
            <DialogTitle>Rename item</DialogTitle>
            <input aria-label="Name" />
          </DialogContent>
        </Dialog>
      </div>
    )

    fireEvent.pointerDown(screen.getByLabelText('Name'))

    expect(handleAncestorPointerDown).not.toHaveBeenCalled()
  })

  it('stops pointerdown events on the overlay from reaching React ancestors', () => {
    const handleAncestorPointerDown = vi.fn()

    render(
      <div onPointerDown={handleAncestorPointerDown}>
        <Dialog open>
          <DialogContent aria-describedby={undefined}>
            <DialogTitle>Rename item</DialogTitle>
          </DialogContent>
        </Dialog>
      </div>
    )

    const overlay = document.querySelector('[data-slot="dialog-overlay"]')
    expect(overlay).toBeInTheDocument()

    fireEvent.pointerDown(overlay!)

    expect(handleAncestorPointerDown).not.toHaveBeenCalled()
  })

  it('lets pointerdown events outside a dialog reach React ancestors', () => {
    const handleAncestorPointerDown = vi.fn()

    render(
      <div onPointerDown={handleAncestorPointerDown}>
        <button type="button">Outside</button>
      </div>
    )

    fireEvent.pointerDown(screen.getByText('Outside'))

    expect(handleAncestorPointerDown).toHaveBeenCalledTimes(1)
  })

  it('preserves pointerdown handlers inside the dialog content', () => {
    const handleAncestorPointerDown = vi.fn()
    const handleContentPointerDown = vi.fn()
    const handleButtonPointerDown = vi.fn()

    render(
      <div onPointerDown={handleAncestorPointerDown}>
        <Dialog open>
          <DialogContent aria-describedby={undefined} onPointerDown={handleContentPointerDown}>
            <DialogTitle>Rename item</DialogTitle>
            <button type="button" onPointerDown={handleButtonPointerDown}>
              Inside
            </button>
          </DialogContent>
        </Dialog>
      </div>
    )

    fireEvent.pointerDown(screen.getByText('Inside'))

    expect(handleButtonPointerDown).toHaveBeenCalledTimes(1)
    expect(handleContentPointerDown).toHaveBeenCalledTimes(1)
    expect(handleAncestorPointerDown).not.toHaveBeenCalled()
  })

  it('closes on overlay click by default', () => {
    const handleOpenChange = vi.fn()

    render(
      <Dialog open onOpenChange={handleOpenChange}>
        <DialogContent aria-describedby={undefined}>
          <DialogTitle>Rename item</DialogTitle>
        </DialogContent>
      </Dialog>
    )

    const overlay = document.querySelector('[data-slot="dialog-overlay"]')
    expect(overlay).toBeInTheDocument()

    fireEvent.click(overlay!)

    expect(handleOpenChange).toHaveBeenCalledWith(false)
  })

  it('does not close when overlay click close is disabled', () => {
    const handleOpenChange = vi.fn()

    render(
      <Dialog open onOpenChange={handleOpenChange}>
        <DialogContent aria-describedby={undefined} closeOnOverlayClick={false}>
          <DialogTitle>Rename item</DialogTitle>
        </DialogContent>
      </Dialog>
    )

    const overlay = document.querySelector('[data-slot="dialog-overlay"]')
    expect(overlay).toBeInTheDocument()

    fireEvent.click(overlay!)

    expect(handleOpenChange).not.toHaveBeenCalled()
  })

  it('keeps the overlay mounted when overlay click close is toggled', () => {
    const handleOpenChange = vi.fn()
    const { rerender } = render(
      <Dialog open onOpenChange={handleOpenChange}>
        <DialogContent aria-describedby={undefined} closeOnOverlayClick>
          <DialogTitle>Create item</DialogTitle>
        </DialogContent>
      </Dialog>
    )

    const overlay = document.querySelector('[data-slot="dialog-overlay"]')
    expect(overlay).toBeInTheDocument()

    rerender(
      <Dialog open onOpenChange={handleOpenChange}>
        <DialogContent aria-describedby={undefined} closeOnOverlayClick={false}>
          <DialogTitle>Create item</DialogTitle>
        </DialogContent>
      </Dialog>
    )

    expect(document.querySelector('[data-slot="dialog-overlay"]')).toBe(overlay)
    fireEvent.click(overlay!)
    expect(handleOpenChange).not.toHaveBeenCalled()

    rerender(
      <Dialog open onOpenChange={handleOpenChange}>
        <DialogContent aria-describedby={undefined} closeOnOverlayClick>
          <DialogTitle>Create item</DialogTitle>
        </DialogContent>
      </Dialog>
    )

    expect(document.querySelector('[data-slot="dialog-overlay"]')).toBe(overlay)
    fireEvent.click(overlay!)
    expect(handleOpenChange).toHaveBeenCalledWith(false)
  })

  it('keeps the dialog open when dismissing an in-dialog select from another dialog field', async () => {
    const handleOpenChange = vi.fn()

    render(<DialogWithSelect onOpenChange={handleOpenChange} />)

    fireEvent.pointerDown(screen.getByRole('combobox', { name: 'Mode' }))
    fireEvent.click(screen.getByRole('combobox', { name: 'Mode' }))

    expect(await screen.findByRole('option', { name: 'Beta' })).toBeInTheDocument()
    await waitFor(() => expect(screen.getByRole('dialog')).not.toHaveStyle({ pointerEvents: 'none' }))

    fireEvent.pointerDown(screen.getByLabelText('Name'))
    fireEvent.click(screen.getByLabelText('Name'))

    await waitFor(() => expect(screen.queryByRole('option', { name: 'Beta' })).not.toBeInTheDocument())
    expect(handleOpenChange).not.toHaveBeenCalledWith(false)
  })

  it('names the close control Close by default', () => {
    render(
      <Dialog open>
        <DialogContent aria-describedby={undefined}>
          <DialogTitle>Rename item</DialogTitle>
        </DialogContent>
      </Dialog>
    )

    const close = screen.getByRole('button', { name: 'Close' })
    expect(close).toBeInTheDocument()
    expect(close).toHaveAttribute('aria-label', 'Close')
    expect(close).toHaveAttribute('data-variant', 'ghost')
    expect(close.querySelector('svg')).toBeInTheDocument()
    expect(close.className).toContain('absolute')
    expect(close.className).toContain('size-6')
    expect(close.className).not.toContain('bg-neutral-900')
    expect(close.closest('[data-slot="dialog-content"]')?.className).toContain('pr-12')
    expect(close.textContent).not.toContain('Close')
  })

  it('reserves right padding for the close icon except on a bleed dialog', () => {
    const { rerender } = render(
      <Dialog open>
        <DialogContent aria-describedby={undefined} className="gap-0 p-0">
          <DialogTitle>Bleed</DialogTitle>
        </DialogContent>
      </Dialog>
    )
    expect(screen.getByRole('button', { name: 'Close' }).closest('[data-slot="dialog-content"]')?.className).not.toContain(
      'pr-12'
    )

    rerender(
      <Dialog open>
        <DialogContent aria-describedby={undefined} className="!p-0">
          <DialogTitle>Bleed</DialogTitle>
        </DialogContent>
      </Dialog>
    )
    expect(screen.getByRole('button', { name: 'Close' }).closest('[data-slot="dialog-content"]')?.className).not.toContain(
      'pr-12'
    )

    rerender(
      <Dialog open>
        <DialogContent aria-describedby={undefined} className="p-0.5 sm:p-0">
          <DialogTitle>Inset</DialogTitle>
        </DialogContent>
      </Dialog>
    )
    expect(screen.getByRole('button', { name: 'Close' }).closest('[data-slot="dialog-content"]')?.className).toContain(
      'pr-12'
    )

    rerender(
      <Dialog open>
        <DialogContent aria-describedby={undefined} showCloseButton={false}>
          <DialogTitle>Confirm</DialogTitle>
        </DialogContent>
      </Dialog>
    )
    expect(document.querySelector('[data-slot="dialog-content"]')?.className).not.toContain('pr-12')
  })

  it('uses a caller-provided closeLabel as the close control name', () => {
    render(
      <Dialog open>
        <DialogContent aria-describedby={undefined} closeLabel="关闭">
          <DialogTitle>频道日志</DialogTitle>
        </DialogContent>
      </Dialog>
    )

    const close = screen.getByRole('button', { name: '关闭' })
    expect(close).toHaveAttribute('aria-label', '关闭')
    expect(close.querySelector('svg')).toBeInTheDocument()
    expect(close.textContent).not.toContain('关闭')
    expect(screen.queryByRole('button', { name: 'Close' })).not.toBeInTheDocument()
  })
})
