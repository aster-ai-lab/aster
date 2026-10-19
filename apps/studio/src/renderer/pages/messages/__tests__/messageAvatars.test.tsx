import { render, screen } from '@testing-library/react'
import { describe, expect, it } from 'vitest'

import { avatarText, GroupAvatar, PersonAvatar } from '../messageAvatars'

describe('avatarText', () => {
  it('uses nickname initials and falls back to the last four phone digits', () => {
    expect(avatarText('周予安')).toBe('周予')
    expect(avatarText('Roy Lin')).toBe('RL')
    expect(avatarText('', '13800138000')).toBe('8000')
    expect(avatarText('13800138000')).toBe('8000')
    expect(avatarText('  +86 138-0013-8000  ')).toBe('8000')
    expect(avatarText('')).toBe('')
  })
})

describe('PersonAvatar', () => {
  it('draws nickname text when no photo is set', () => {
    render(<PersonAvatar name="周予安" />)
    const mark = screen.getByRole('img', { name: '周予安' })
    expect(mark).toHaveAttribute('data-kind', 'text')
    expect(mark).toHaveTextContent('周予')
  })

  it('draws the saved photo instead of initials', () => {
    render(<PersonAvatar name="林知夏" avatar="data:image/webp;base64,AA" />)
    const mark = screen.getByRole('img', { name: '林知夏' })
    expect(mark).toHaveAttribute('data-kind', 'image')
    expect(mark.querySelector('img')).toHaveAttribute('src', 'data:image/webp;base64,AA')
    expect(mark).not.toHaveTextContent('林知')
  })

  it('keeps a file photo instead of redrawing initials', () => {
    render(<PersonAvatar name="林知夏" avatar="file:///Users/me/avatar.webp" />)
    const mark = screen.getByRole('img', { name: '林知夏' })
    expect(mark).toHaveAttribute('data-kind', 'image')
    expect(mark.querySelector('img')).toHaveAttribute('src', 'file:///Users/me/avatar.webp')
  })

  it('draws a composed svg photo instead of the phone tail', () => {
    render(<PersonAvatar name="" phone="18770221825" avatar="data:image/svg+xml;base64,QQ" />)
    const mark = screen.getByRole('img', { name: '18770221825' })
    expect(mark).toHaveAttribute('data-kind', 'image')
    expect(mark.querySelector('img')).toHaveAttribute('src', 'data:image/svg+xml;base64,QQ')
    expect(mark).not.toHaveTextContent('1825')
  })

  it('uses the phone when the nickname is empty', () => {
    render(<PersonAvatar name="" phone="13912345678" />)
    expect(screen.getByRole('img', { name: '13912345678' })).toHaveTextContent('5678')
  })
})

function tileBox(tile: Element) {
  const node = tile as HTMLElement
  return {
    x: Number(node.style.left.replace('px', '')),
    y: Number(node.style.top.replace('px', '')),
    size: Number(node.style.width.replace('px', ''))
  }
}

function tilesOf() {
  return [...document.querySelectorAll('[data-ui="messages.group-face"]')].map(tileBox)
}

function separated(tiles: ReturnType<typeof tilesOf>) {
  return tiles.every((left, index) =>
    tiles.slice(index + 1).every((right) => {
      return (
        left.x + left.size <= right.x ||
        right.x + right.size <= left.x ||
        left.y + left.size <= right.y ||
        right.y + right.size <= left.y
      )
    })
  )
}

describe('GroupAvatar', () => {
  it('paints each member as text inside the composite', () => {
    render(
      <GroupAvatar
        faces={[
          { kind: 'person', name: '周予安', colorKey: 'a' },
          { kind: 'person', name: '', phone: '13912345678', colorKey: 'b' }
        ]}
      />
    )
    const grid = document.querySelector('[data-ui="messages.group-avatar"]')
    expect(grid).toHaveAttribute('data-faces', '2')
    expect(grid).toHaveTextContent('周予')
    expect(grid).toHaveTextContent('5678')
    const tiles = tilesOf()
    expect(separated(tiles)).toBe(true)
    expect(tiles[0]?.y).toBe(tiles[1]?.y)
    expect(tiles[0]?.y).toBeGreaterThan(0)
  })

  it('centers the top face of three and keeps four faces in a 2 by 2 grid', () => {
    const people = ['周予安', 'Roy Lin', '林书豪', 'Leon']
    const { rerender } = render(
      <GroupAvatar faces={people.slice(0, 3).map((name) => ({ kind: 'person' as const, name }))} />
    )
    const three = tilesOf()
    expect(three).toHaveLength(3)
    expect(separated(three)).toBe(true)
    expect(three[0]?.x).toBeGreaterThan(three[1]?.x ?? 0)
    expect(three[1]?.y).toBe(three[2]?.y)

    rerender(<GroupAvatar faces={people.map((name) => ({ kind: 'person' as const, name }))} />)
    const four = tilesOf()
    expect(four).toHaveLength(4)
    expect(separated(four)).toBe(true)
    expect(new Set(four.map((tile) => tile.x)).size).toBe(2)
    expect(new Set(four.map((tile) => tile.y)).size).toBe(2)
  })
})
