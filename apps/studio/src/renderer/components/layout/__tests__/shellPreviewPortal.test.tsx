// @vitest-environment jsdom
import { DialogPortalContainerProvider, ImagePreviewDialog } from '@a3s-lab/ui'
import '@testing-library/jest-dom/vitest'
import { cleanup, render, screen } from '@testing-library/react'
import { afterEach, describe, expect, it, vi } from 'vitest'

import {
  ForegroundImagePreviewPortal,
  ImagePreviewDialogFrame,
  ShellPreviewPortalProvider
} from '../shellPreviewPortal'

const item = { alt: 'One', id: 'one', src: 'https://example.com/one.png' }

function Preview({ active }: { active: boolean }) {
  return (
    <ForegroundImagePreviewPortal active={active}>
      <ImagePreviewDialogFrame>
        <ImagePreviewDialog open items={[item]} onOpenChange={vi.fn()} />
      </ImagePreviewDialogFrame>
    </ForegroundImagePreviewPortal>
  )
}

describe('image preview shell portal', () => {
  afterEach(() => {
    cleanup()
    document.body.replaceChildren()
  })

  it('covers the shell while the tab is in front and returns to the tab when it is not', () => {
    const shell = document.createElement('div')
    const tab = document.createElement('div')
    document.body.append(shell, tab)

    const tree = (active: boolean) => (
      <ShellPreviewPortalProvider container={shell}>
        <DialogPortalContainerProvider container={tab}>
          <Preview active={active} />
        </DialogPortalContainerProvider>
      </ShellPreviewPortalProvider>
    )

    const { rerender } = render(tree(true))

    const preview = screen.getByTestId('image-preview-dialog')
    expect(shell.contains(preview)).toBe(true)
    expect(tab.contains(preview)).toBe(false)

    rerender(tree(false))

    const parked = screen.getByTestId('image-preview-dialog')
    expect(shell.contains(parked)).toBe(false)
    expect(tab.contains(parked)).toBe(true)
  })
})
