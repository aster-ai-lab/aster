import { act, render, screen } from '@testing-library/react'
import { afterEach, beforeEach, expect, it, vi } from 'vitest'

import { PaperloomPdf } from '../PaperloomPdf'

const mocks = vi.hoisted(() => ({ getDocument: vi.fn(), renderPage: vi.fn() }))
vi.mock('pdfjs-dist', () => ({
  getDocument: mocks.getDocument,
  GlobalWorkerOptions: {},
  TextLayer: class {
    render() {
      return Promise.resolve()
    }
    cancel() {}
  }
}))
vi.mock('pdfjs-dist/build/pdf.worker.mjs?url', () => ({ default: 'fixture-worker' }))
vi.mock('@renderer/components/FilePreview', () => ({
  PdfjsBundledCMapReaderFactory: class {},
  PdfjsBundledStandardFontDataFactory: class {}
}))
vi.mock('../PaperloomClient', () => ({ usePaperloomClient: () => ({}) }))

function drawTask() {
  let finish!: () => void
  let fail!: (error: Error) => void
  const promise = new Promise<void>((resolve, reject) => {
    finish = resolve
    fail = reject
  })
  return {
    promise,
    finish,
    cancel: vi.fn(() => fail(new Error('cancelled'))),
    onContinue: undefined as undefined | ((resume: () => void) => void)
  }
}

beforeEach(() => {
  vi.useFakeTimers()
  vi.spyOn(HTMLElement.prototype, 'clientWidth', 'get').mockReturnValue(640)
  vi.stubGlobal(
    'ResizeObserver',
    class {
      observe() {}
      disconnect() {}
    }
  )
  mocks.getDocument.mockImplementation(() => ({
    promise: Promise.resolve({
      numPages: 2,
      getPage: vi.fn(async () => ({
        getTextContent: async () => ({}),
        getViewport: ({ scale }: { scale: number }) => ({ width: 600 * scale, height: 800 * scale, scale }),
        render: mocks.renderPage
      }))
    }),
    destroy: async () => {}
  }))
})

afterEach(() => {
  vi.useRealTimers()
  vi.restoreAllMocks()
  vi.clearAllMocks()
  vi.unstubAllGlobals()
})

it('leaves the loading status when animation frames never run', async () => {
  const ready = vi.fn()
  window.requestAnimationFrame = () => 0
  mocks.getDocument.mockImplementationOnce(() => ({
    promise: Promise.resolve({
      numPages: 2,
      getPage: async () => ({
        getTextContent: async () => ({ items: [{ str: 'PaperLoom regression - Page 1' }] }),
        getViewport: ({ scale }: { scale: number }) => ({ width: 600 * scale, height: 800 * scale, scale }),
        render: mocks.renderPage
      })
    }),
    destroy: async () => {}
  }))
  mocks.renderPage.mockImplementation(() => {
    const task = drawTask()
    queueMicrotask(() => {
      task.onContinue?.(() => task.finish())
    })
    return task
  })
  render(<PaperloomPdf path="paper.pdf" source={new Uint8Array([1])} page={1} zoom={1} label="原文" onReady={ready} />)
  await act(async () => {})
  expect(screen.queryByRole('status')).toBeNull()
  expect(screen.getByLabelText('PaperLoom regression - Page 1')).toBeInTheDocument()
  expect(ready).toHaveBeenCalledTimes(1)
})

it('does not start reading text from a page that resolves after closing', async () => {
  const pdfPage = { getTextContent: vi.fn() }
  let finish!: (page: typeof pdfPage) => void
  mocks.getDocument.mockReturnValueOnce({
    promise: Promise.resolve({
      numPages: 1,
      getPage: () =>
        new Promise((resolve) => {
          finish = resolve
        })
    }),
    destroy: async () => {}
  })
  const ready = vi.fn()
  const { unmount } = render(
    <PaperloomPdf path="paper.pdf" source={new Uint8Array([1])} page={1} zoom={1} label="原文" onReady={ready} />
  )
  await act(async () => {})
  unmount()
  await act(async () => {
    finish(pdfPage)
  })
  expect(pdfPage.getTextContent).not.toHaveBeenCalled()
  expect(mocks.renderPage).not.toHaveBeenCalled()
  expect(ready).not.toHaveBeenCalled()
})

it('cancels a pending continuation when the source changes and only acknowledges its replacement', async () => {
  const oldTask = drawTask()
  const nextTask = drawTask()
  mocks.renderPage.mockReturnValueOnce(oldTask).mockReturnValueOnce(nextTask)
  const ready = vi.fn()
  const { rerender, unmount } = render(
    <PaperloomPdf path="paper.pdf" source={new Uint8Array([1])} page={1} zoom={1} label="原文" onReady={ready} />
  )
  await act(async () => {})
  expect(mocks.renderPage).toHaveBeenCalledTimes(1)
  expect(ready).not.toHaveBeenCalled()
  const staleResume = vi.fn()
  oldTask.onContinue?.(staleResume)
  rerender(<PaperloomPdf path="next.pdf" source={new Uint8Array([2])} page={1} zoom={1} label="原文" onReady={ready} />)
  await act(async () => {
    await vi.runAllTimersAsync()
  })
  expect(oldTask.cancel).toHaveBeenCalledTimes(1)
  expect(staleResume).not.toHaveBeenCalled()
  expect(mocks.renderPage).toHaveBeenCalledTimes(2)
  expect(ready).not.toHaveBeenCalled()
  const resume = vi.fn()
  nextTask.onContinue?.(resume)
  await act(async () => {
    await vi.runAllTimersAsync()
  })
  expect(resume).toHaveBeenCalledTimes(1)
  await act(async () => {
    nextTask.finish()
  })
  expect(ready).toHaveBeenCalledTimes(1)
  const afterClose = vi.fn()
  nextTask.onContinue?.(afterClose)
  unmount()
  await act(async () => {
    await vi.runAllTimersAsync()
  })
  expect(afterClose).not.toHaveBeenCalled()
})
