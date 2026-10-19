import { describe, expect, it } from 'vitest'

import { PLUGIN_PAGE_SIZE, pluginCatalogEntry, pluginQuery, pluginTypeKey } from '../pluginStoreData'

describe('plugin store catalog', () => {
  it('asks the member catalog for plugins', () => {
    expect(pluginQuery({ view: 'market', category: 'all', q: '' })).toEqual({
      kind: 'plugin',
      pageSize: PLUGIN_PAGE_SIZE
    })
    expect(pluginQuery({ view: 'market', category: 'featured', q: '' })).toEqual({
      kind: 'plugin',
      pageSize: PLUGIN_PAGE_SIZE,
      featured: true
    })
    expect(pluginQuery({ view: 'market', category: 'code', q: ' 审查 ' })).toEqual({
      kind: 'plugin',
      pageSize: PLUGIN_PAGE_SIZE,
      category: 'code',
      q: '审查'
    })
    expect(pluginQuery({ view: 'installed', category: 'featured', q: '' })).toEqual({
      kind: 'plugin',
      pageSize: PLUGIN_PAGE_SIZE,
      featured: true,
      installed: true
    })
  })

  it('includes composer agent extensions without requiring a window', () => {
    expect(pluginCatalogEntry({ surfaces: ['ui'] })).toBe(true)
    expect(pluginCatalogEntry({ surfaces: ['skill', 'agent', 'ui'] })).toBe(true)
    expect(pluginCatalogEntry({ surfaces: ['skill', 'agent'] })).toBe(true)
    expect(pluginCatalogEntry({ surfaces: ['skill'] })).toBe(true)
    expect(pluginCatalogEntry({ surfaces: ['mcp'] })).toBe(true)
    expect(pluginCatalogEntry({ surfaces: ['agent'] })).toBe(true)
    expect(pluginCatalogEntry({ surfaces: ['mcp', 'agent'] })).toBe(true)
    expect(pluginCatalogEntry({ surfaces: [] })).toBe(false)
    expect(pluginCatalogEntry({ builtin: true, surfaces: ['ui'] })).toBe(false)
  })

  it('labels an applet contract as a mini app and other ui packages as plugins', () => {
    expect(pluginTypeKey(['ui'], { permissions: ['user'] })).toBe('plugins.type.applet')
    expect(pluginTypeKey(['skill', 'ui'])).toBe('plugins.type.plugin')
    expect(pluginTypeKey(['ui'])).toBe('plugins.type.plugin')
    expect(pluginTypeKey(['tool'])).toBe('plugins.type.tool')
    expect(pluginTypeKey(['plugin'])).toBe('plugins.type.plugin')
    expect(pluginTypeKey(['agent'])).toBe('plugins.type.agent')
    expect(pluginTypeKey(['mcp', 'agent'])).toBe('plugins.type.agent')
    expect(pluginTypeKey(['skill', 'agent', 'ui'])).toBe('plugins.type.plugin')
    expect(pluginTypeKey(['skill', 'agent'])).toBe('plugins.type.agent')
    expect(pluginTypeKey([])).toBeNull()
  })
})
