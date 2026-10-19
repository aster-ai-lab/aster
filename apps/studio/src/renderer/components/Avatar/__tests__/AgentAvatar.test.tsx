import { fireEvent, render, waitFor } from '@testing-library/react'
import { afterEach, describe, expect, it, vi } from 'vitest'

import { AgentAvatar, agentAvatarRadius } from '../AgentAvatar'
import { agentAvatarElement } from '../agentAvatarDom'
import { readAgentAvatar } from '../agentAvatarValue'

vi.mock('@shared/studioRuntime', () => ({
  readStudioRuntime: () => ({ apiBaseUrl: 'http://127.0.0.1:4000', token: 'shell-token' })
}))

const CATALOG_ICON = '/v1/api/org/capabilities/a3s-review/icon?v=abc'

afterEach(() => {
  vi.unstubAllGlobals()
  vi.restoreAllMocks()
})

function stubIconFetch() {
  const fetchIcon = vi.fn(async () => new Response('<svg/>', { headers: { 'content-type': 'image/svg+xml' } }))
  vi.stubGlobal('fetch', fetchIcon)
  return fetchIcon
}

describe('readAgentAvatar', () => {
  it('reads each stored form into one kind', () => {
    expect(readAgentAvatar('⚖️')).toEqual({ kind: 'glyph', glyph: '⚖️' })
    expect(readAgentAvatar('👨‍👩‍👧')).toEqual({ kind: 'glyph', glyph: '👨‍👩‍👧' })
    expect(readAgentAvatar('文')).toEqual({ kind: 'glyph', glyph: '文' })
    expect(readAgentAvatar('/static/image/legal-expert.jpg')).toEqual({
      kind: 'image',
      src: '/static/image/legal-expert.jpg'
    })
    expect(readAgentAvatar('data:image/png;base64,AAAA')).toEqual({ kind: 'image', src: 'data:image/png;base64,AAAA' })
    expect(readAgentAvatar('https://cdn.example/a.png')).toEqual({ kind: 'image', src: 'https://cdn.example/a.png' })
    expect(readAgentAvatar(CATALOG_ICON)).toEqual({
      kind: 'catalog-icon',
      path: '/api/capabilities/a3s-review/icon?v=abc'
    })
  })

  it('refuses a string that is neither a glyph nor an image address', () => {
    for (const raw of [
      '',
      '   ',
      undefined,
      null,
      'legal expert',
      'assets/a.png',
      '//evil.example/a.png',
      'C:\\a.png'
    ]) {
      expect(readAgentAvatar(raw)).toBeNull()
    }
  })
})

describe('AgentAvatar', () => {
  it('loads a catalog icon through the authenticated API and never prints the path', async () => {
    const fetchIcon = stubIconFetch()
    vi.spyOn(URL, 'createObjectURL').mockReturnValue('blob:review-icon')
    const view = render(<AgentAvatar avatar={CATALOG_ICON} name="法务专家" size={36} />)
    expect(view.container.textContent).toBe('法')
    const image = await waitFor(() => {
      const found = view.container.querySelector('img')
      expect(found).not.toBeNull()
      return found as HTMLImageElement
    })
    expect(image).toHaveAttribute('src', 'blob:review-icon')
    expect(fetchIcon).toHaveBeenCalledWith(
      'http://127.0.0.1:4000/api/capabilities/a3s-review/icon?v=abc',
      expect.objectContaining({ headers: expect.objectContaining({ authorization: 'Bearer shell-token' }) })
    )
    expect(view.container.textContent).not.toContain('/api/')
  })

  it('falls back to the name initial when the image fails', () => {
    const view = render(<AgentAvatar avatar="/static/image/missing.jpg" name="writer" size={24} />)
    fireEvent.error(view.container.querySelector('img') as HTMLImageElement)
    expect(view.container.querySelector('img')).toBeNull()
    expect(view.container.textContent).toBe('W')
  })

  it('draws a glyph once on a rounded tile, or bare in a plain row', () => {
    const tile = render(<AgentAvatar avatar="⚖️" size={32} />)
    const mark = tile.container.querySelector('[data-avatar="glyph"]') as HTMLElement
    expect(mark.textContent).toBe('⚖️')
    expect(mark).toHaveClass('bg-[#f2f3f5]')
    expect(mark.style.borderRadius).toBe('8px')
    expect(tile.container.querySelector('[data-slot="emoji-icon"]')).toBeNull()
    tile.unmount()
    const plain = render(<AgentAvatar avatar="⚖️" size={16} glyphStyle="plain" />)
    const bare = plain.container.querySelector('[data-avatar="glyph"]') as HTMLElement
    expect(bare.textContent).toBe('⚖️')
    expect(bare).not.toHaveClass('bg-[#f2f3f5]')
  })

  it('rounds the mark by the DESIGN.md radius for its size', () => {
    expect([14, 16, 32, 40, 48].map(agentAvatarRadius)).toEqual([4, 4, 8, 10, 12])
  })

  it('draws the default glyph when there is neither an avatar nor a name', () => {
    const view = render(<AgentAvatar avatar="not an avatar" size={24} glyphStyle="plain" />)
    expect(view.container.textContent).toBe('🤖')
  })

  it('paints an image as a background on editor surfaces', () => {
    const view = render(<AgentAvatar avatar="/static/image/a.jpg" size={14} imagePaint="background" />)
    expect(view.container.querySelector('img')).toBeNull()
    expect(view.container.querySelector('[data-avatar="image"]')?.getAttribute('style')).toContain(
      'url("/static/image/a.jpg")'
    )
  })
})

describe('agentAvatarElement', () => {
  it('paints a catalog icon after it loads and shows the initial until then', async () => {
    stubIconFetch()
    const mark = agentAvatarElement(CATALOG_ICON, '法务专家')
    expect(mark.textContent).toBe('法')
    await waitFor(() => expect(mark.style.backgroundImage).toContain('data:image/svg+xml;base64,'))
    expect(mark.textContent).toBe('')
  })

  it('paints a glyph as text and an image as a background', () => {
    expect(agentAvatarElement('⚖️').textContent).toBe('⚖️')
    const image = agentAvatarElement('/static/image/a.jpg')
    expect(image.textContent).toBe('')
    expect(image.style.backgroundImage).toContain('/static/image/a.jpg')
  })
})
