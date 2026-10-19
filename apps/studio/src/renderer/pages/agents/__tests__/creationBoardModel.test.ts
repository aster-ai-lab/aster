import { beforeEach, describe, expect, it } from 'vitest'

import {
  CREATE_BOARD_STORAGE_KEY,
  boardFor,
  freshBoard,
  hitNode,
  resizeNode,
  screenToWorld,
  seedNodes,
  writeBoard,
  zoomAt
} from '../creationBoardModel'

describe('creation board model', () => {
  beforeEach(() => {
    localStorage.removeItem(CREATE_BOARD_STORAGE_KEY)
  })

  it('seeds a storyboard for each entry and leaves the center empty', () => {
    expect(seedNodes('center')).toHaveLength(0)
    expect(seedNodes('ppt').filter((node) => node.kind === 'frame')).toHaveLength(4)
    expect(seedNodes('site').some((node) => node.text === 'workbench.create.seed.site.hero')).toBe(true)
    expect(seedNodes('video').filter((node) => node.kind === 'frame')).toHaveLength(4)
    expect(seedNodes('image').filter((node) => node.kind === 'shape')).toHaveLength(3)
  })

  it('keeps the cursor point fixed while zooming', () => {
    const camera = { panX: 20, panY: -10, zoom: 1 }
    const next = zoomAt(camera, 180, 90, 2)
    expect(screenToWorld(next, 180, 90)).toEqual(screenToWorld(camera, 180, 90))
    expect(zoomAt(camera, 0, 0, 10).zoom).toBe(2.5)
  })

  it('hits the topmost object and resizes from the corner', () => {
    const nodes = freshBoard('ppt').nodes
    const cover = nodes.find((node) => node.id === 'ppt-1')
    expect(cover).toBeTruthy()
    expect(hitNode(nodes, 28, 70)?.id).toBe('ppt-note')
    expect(resizeNode(cover!, 'se', 40, -200)).toMatchObject({ w: cover!.w + 40, h: 88 })
  })

  it('remembers an edited board and ignores a corrupt store', () => {
    const board = freshBoard('center')
    board.nodes = seedNodes('ppt').slice(0, 1)
    board.title = '命名画布'
    board.titleLiteral = true
    writeBoard(localStorage, 'center', board)
    expect(boardFor(localStorage, 'center').title).toBe('命名画布')
    expect(boardFor(localStorage, 'ppt').nodes.filter((node) => node.kind === 'frame')).toHaveLength(4)

    localStorage.setItem(CREATE_BOARD_STORAGE_KEY, '{')
    expect(boardFor(localStorage, 'ppt').nodes.filter((node) => node.kind === 'frame')).toHaveLength(4)
  })
})
