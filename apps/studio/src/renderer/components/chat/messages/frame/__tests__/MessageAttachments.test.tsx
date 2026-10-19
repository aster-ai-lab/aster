import { render, screen } from '@testing-library/react'
import { beforeEach, describe, expect, it, vi } from 'vitest'

import type { AbsoluteFilePath } from '@shared/types/file'

import MessageAttachments from '../MessageAttachments'

const { mockUseQuery, mockPreviewFile, mockOpenFile } = vi.hoisted(() => ({
  mockUseQuery: vi.fn(),
  mockPreviewFile: vi.fn(),
  mockOpenFile: vi.fn()
}))

vi.mock('@data/hooks/useDataApi', () => ({
  useQuery: mockUseQuery
}))

vi.mock('react-i18next', () => ({
  useTranslation: () => ({ t: (key: string) => key })
}))

vi.mock('../../MessageListProvider', () => ({
  useOptionalMessageListActions: () => ({ previewFile: mockPreviewFile, openFile: mockOpenFile }),
  useOptionalMessageListUi: () => undefined
}))

const entryHandle = { kind: 'entry', entryId: '019606a0-0000-7000-8000-000000000001' } as const

function iconType(): string | null {
  return globalThis.document.querySelector('[data-file-icon]')?.getAttribute('data-file-icon') ?? null
}

describe('MessageAttachments', () => {
  beforeEach(() => {
    vi.clearAllMocks()
    mockUseQuery.mockReturnValue({ data: undefined })
  })

  // The file part carries no size; without the entry lookup the card reads "0.00 KB".
  it('shows the size the entry row reports', () => {
    mockUseQuery.mockReturnValue({
      data: { id: entryHandle.entryId, origin: 'internal', name: 'report.pdf', ext: '.pdf', size: 2048 }
    })

    render(<MessageAttachments handle={entryHandle} name="report.pdf" ext=".pdf" createdAt="2026-01-01" />)

    expect(screen.getByText('2K')).toBeInTheDocument()
    expect(iconType()).toBe('pdf')
  })

  it('shows the document icon rather than a fabricated zero size', () => {
    render(
      <MessageAttachments
        handle={{ kind: 'path', path: '/tmp/report.pdf' as AbsoluteFilePath }}
        name="report.pdf"
        ext=".pdf"
        createdAt="2026-01-01"
      />
    )

    expect(iconType()).toBe('pdf')
    expect(screen.queryByText(/0\.00 KB/)).toBeNull()
    expect(screen.getByText('PDF').closest('[data-file-icon]')).not.toBeNull()
  })

  it('uses the message document icon for each file type', () => {
    const { rerender } = render(
      <MessageAttachments handle={entryHandle} name="预算.xlsx" ext=".xlsx" createdAt="2026-01-01" />
    )
    expect(iconType()).toBe('excel')
    expect(globalThis.document.querySelector('[data-file-icon] svg')).toHaveClass('h-9', 'w-8')

    rerender(<MessageAttachments handle={entryHandle} name="说明.docx" ext=".docx" createdAt="2026-01-01" />)
    expect(iconType()).toBe('word')

    rerender(<MessageAttachments handle={entryHandle} name="纪要.txt" ext=".txt" createdAt="2026-01-01" />)
    expect(iconType()).toBe('text')

    rerender(<MessageAttachments handle={entryHandle} name="周报.pptx" ext=".pptx" createdAt="2026-01-01" />)
    expect(iconType()).toBe('slides')
  })

  it('hands the untouched handle to the preview action', async () => {
    render(<MessageAttachments handle={entryHandle} name="report.pdf" ext=".pdf" createdAt="2026-01-01" />)

    screen.getByRole('button', { name: 'report.pdf' }).click()

    expect(mockPreviewFile).toHaveBeenCalledWith(expect.objectContaining({ handle: entryHandle }))
    expect(mockOpenFile).not.toHaveBeenCalled()
  })
})
