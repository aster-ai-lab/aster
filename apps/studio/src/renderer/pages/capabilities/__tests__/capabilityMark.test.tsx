import { render, screen, waitFor } from '@testing-library/react'
import { afterEach, describe, expect, it, vi } from 'vitest'

import type { CapabilityEntry } from '../capabilitiesData'
import { CapabilityMark } from '../capabilityMark'

vi.mock('@shared/studioRuntime', () => ({
  readStudioRuntime: () => ({ apiBaseUrl: 'http://127.0.0.1:4000', token: 'shell-token' })
}))

function entry(patch: Partial<CapabilityEntry> = {}): CapabilityEntry {
  return {
    slug: 'notion',
    kind: 'connector',
    category: 'office',
    name: 'Notion',
    description: '',
    color: '#111111',
    featured: false,
    installed: false,
    config: [],
    ...patch
  }
}

afterEach(() => {
  vi.unstubAllGlobals()
})

describe('CapabilityMark', () => {
  it('shows the catalog logo contained in the mark', async () => {
    vi.stubGlobal('fetch', vi.fn(async () => new Response(new Blob(['png'], { type: 'image/png' }))))
    vi.spyOn(URL, 'createObjectURL').mockReturnValue('blob:notion-logo')
    render(<CapabilityMark entry={entry({ iconUrl: '/api/capabilities/notion/icon?v=abc' })} />)

    const logo = await waitFor(() => {
      const image = document.querySelector('[data-ui="capabilities.mark"] img')
      expect(image).not.toBeNull()
      return image as HTMLImageElement
    })
    expect(logo).toHaveAttribute('src', 'blob:notion-logo')
    expect(logo).toHaveClass('object-contain')
    expect(logo.closest('[data-ui="capabilities.mark"]')).toHaveClass('size-10', 'bg-[#f4f5f7]')
    expect(logo.closest('[data-ui="capabilities.mark"]')?.querySelector('svg')).toBeNull()
    await waitFor(() =>
      expect(fetch).toHaveBeenCalledWith(
        'http://127.0.0.1:4000/api/capabilities/notion/icon?v=abc',
        expect.objectContaining({
          headers: expect.objectContaining({ authorization: 'Bearer shell-token' })
        })
      )
    )
  })

  it('uses the name initial when the catalog has no logo', () => {
    render(<CapabilityMark entry={entry({ iconUrl: undefined })} />)
    const mark = screen.getByText('N').closest('[data-ui="capabilities.mark"]')
    expect(mark).toHaveClass('size-10')
    expect(mark?.querySelector('img')).toBeNull()
    expect(mark?.querySelector('svg')).toBeNull()
  })
})
