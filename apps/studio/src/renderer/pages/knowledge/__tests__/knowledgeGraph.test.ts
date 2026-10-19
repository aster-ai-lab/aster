import { describe, expect, it } from 'vitest'

import { graphLabel, graphShowsLabel, noteGraph } from '../knowledgeGraph'
import { noteDisplayName } from '../noteReading'

const notes = [
  { name: '甲.md', path: '/vault/甲.md', markdown: '见 [[乙]] 和 [[还没有]]' },
  { name: '乙.md', path: '/vault/乙.md', markdown: '乙的正文' },
  { name: '丙.md', path: '/vault/丙.md', markdown: '单独的一篇' },
  { name: '丁.md', path: '/vault/丁.md', markdown: null }
]

describe('note graph', () => {
  it('draws resolved links and leaves orphans and unread notes off the default graph', () => {
    const graph = noteGraph(notes)
    expect(graph.nodes.map((node) => node.title)).toEqual(['甲', '乙'])
    expect(graph.edges).toEqual([{ from: '/vault/甲.md', to: '/vault/乙.md' }])
    const first = graph.nodes[0]
    const second = graph.nodes[1]
    expect(first && second && Math.hypot(first.x - second.x, first.y - second.y)).toBeGreaterThan(20)
    expect(graph.nodes.every((node) => node.x >= 12 && node.x <= 88 && node.y >= 12 && node.y <= 88)).toBe(true)
    expect(noteGraph(notes)).toEqual(graph)
  })

  it('keeps the open note and its neighbors, and can show an orphan on request', () => {
    const local = noteGraph(notes, { centerPath: '/vault/甲.md' })
    expect(local.nodes.map((node) => node.title).sort()).toEqual(['乙', '甲'])
    const withOrphans = noteGraph(notes, { orphans: true })
    expect(withOrphans.nodes.map((node) => node.title)).toEqual(['甲', '乙', '丙'])
    expect(withOrphans.nodes.find((node) => node.title === '丁')).toBeUndefined()
  })

  it('shows a label when the graph is sparse, zoomed, hovered, or the open note', () => {
    expect(graphShowsLabel({ count: 40, zoom: 1, hovered: false, center: false })).toBe(false)
    expect(graphShowsLabel({ count: 40, zoom: 1.6, hovered: false, center: false })).toBe(true)
    expect(graphShowsLabel({ count: 40, zoom: 1, hovered: true, center: false })).toBe(true)
    expect(graphShowsLabel({ count: 8, zoom: 1, hovered: false, center: false })).toBe(true)
    expect(graphLabel('政务服务培训会会议通知草稿', 40)).toBe('政务服务培…')
    expect(graphLabel('甲', 40)).toBe('甲')
  })

  it('uses a frontmatter title on a node and the file stem when that title is empty', () => {
    expect(noteDisplayName('甲.md', '---\ntitle: "家常食谱"\n---\n正文')).toBe('家常食谱')
    expect(noteDisplayName('甲.md', '---\ntitle: ""\n---\n正文')).toBe('甲')
    expect(noteDisplayName('甲.md', null)).toBe('甲')
    const graph = noteGraph([
      { name: '甲.md', path: '/vault/甲.md', markdown: '---\ntitle: "家常食谱"\n---\n见 [[乙]]' },
      { name: '乙.md', path: '/vault/乙.md', markdown: '乙的正文' }
    ])
    expect(graph.nodes.map((node) => node.title).sort()).toEqual(['乙', '家常食谱'])
  })
})
