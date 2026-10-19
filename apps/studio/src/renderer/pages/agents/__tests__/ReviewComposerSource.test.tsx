import { act, fireEvent, render, screen, waitFor } from '@testing-library/react'
import { afterEach, describe, expect, it, vi } from 'vitest'

import { ReviewComposerSource } from '../ReviewComposerSource'

function boot(result: unknown = null) {
  const select = vi.fn().mockResolvedValue(result)
  Object.assign(window.api.file, { select })
  const input = {
    getText: () => '使用智能体 合同审查：',
    insertText: vi.fn(),
    insertToken: vi.fn(),
    deleteTriggerRange: vi.fn(),
    focus: vi.fn()
  }
  const view = render(<ReviewComposerSource adapter={() => input} />)
  return { input, select, unmount: view.unmount }
}
afterEach(() => vi.restoreAllMocks())

describe('contract review composer source', () => {
  it('discards a late file choice after the composer has left this review', async () => {
    const { input, select, unmount } = boot()
    let complete!: (files: unknown) => void
    select.mockImplementationOnce(
      () =>
        new Promise((resolve) => {
          complete = resolve
        })
    )
    fireEvent.click(screen.getByRole('button', { name: '选择 Word 文件' }))
    unmount()
    await act(async () => complete([{ path: '/tmp/合同.docx', origin_name: '合同.docx', size: 123 }]))
    expect(input.insertToken).not.toHaveBeenCalled()
  })
  it('turns an explicit absolute path into a file chip without submitting', () => {
    const { input, select } = boot()
    fireEvent.change(screen.getByRole('textbox', { name: 'Word 文件路径' }), { target: { value: '/tmp/合同.docx' } })
    fireEvent.click(screen.getByRole('button', { name: '使用文件' }))
    expect(input.insertToken).toHaveBeenCalledWith(
      expect.objectContaining({ label: '合同.docx', promptText: '/tmp/合同.docx', kind: 'file' })
    )
    expect(input.insertText).toHaveBeenCalledWith('\n', { tokenizeVariables: false })
    expect(select).not.toHaveBeenCalled()
  })
  it.each(['relative.docx', '/tmp/合同.pdf'])('refuses invalid source %s and retains the input', (path) => {
    const { input } = boot()
    const field = screen.getByRole('textbox', { name: 'Word 文件路径' })
    fireEvent.change(field, { target: { value: path } })
    fireEvent.click(screen.getByRole('button', { name: '使用文件' }))
    expect(screen.getByRole('alert')).toBeInTheDocument()
    expect(field).toHaveValue(path)
    expect(input.insertToken).not.toHaveBeenCalled()
  })
  it.each([
    { origin_name: '合同.docx', name: '123-合同.docx' },
    { origin_name: '合同', name: '合同.docx' }
  ])('picks one bounded Word file using its metadata name: $origin_name', async (names) => {
    const { input, select } = boot([{ path: '/tmp/123-合同.docx', ...names, size: 123 }])
    fireEvent.click(screen.getByRole('button', { name: '选择 Word 文件' }))
    await waitFor(() => expect(input.insertToken).toHaveBeenCalledOnce())
    expect(select).toHaveBeenCalledWith(expect.objectContaining({ properties: ['openFile'] }))
    expect(input.insertToken).toHaveBeenCalledWith(
      expect.objectContaining({ label: '合同.docx', promptText: '/tmp/123-合同.docx' })
    )
  })
  it('keeps the draft on picker cancel and allows a second attempt', async () => {
    const { input, select } = boot()
    fireEvent.click(screen.getByRole('button', { name: '选择 Word 文件' }))
    await waitFor(() => expect(screen.getByRole('button', { name: '选择 Word 文件' })).toBeEnabled())
    expect(input.insertToken).not.toHaveBeenCalled()
    fireEvent.click(screen.getByRole('button', { name: '选择 Word 文件' }))
    await waitFor(() => expect(select).toHaveBeenCalledTimes(2))
  })
  it('refuses an oversized file before binding it to the composer', async () => {
    const { input } = boot([{ path: '/tmp/合同.docx', origin_name: '合同.docx', size: 8 * 1024 * 1024 + 1 }])
    fireEvent.click(screen.getByRole('button', { name: '选择 Word 文件' }))
    expect(await screen.findByRole('alert')).toHaveTextContent('8 MiB')
    expect(input.insertToken).not.toHaveBeenCalled()
  })
})
