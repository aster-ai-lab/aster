import { beforeEach, describe, expect, it } from 'vitest'

import {
  HOME_PLUGIN_PIN_LIMIT,
  pinHomePlugin,
  readHomePluginPins,
  reconcileHomePluginPins,
  unpinHomePlugin,
  writeHomePluginPins
} from '../homePins'

describe('home plugin pins', () => {
  beforeEach(() => {
    localStorage.clear()
  })

  it('keeps an empty home until the member pins a plugin', () => {
    expect(readHomePluginPins('member-a')).toEqual([])
    expect(pinHomePlugin('member-a', 'home-writing')).toEqual(['home-writing'])
    expect(readHomePluginPins('member-b')).toEqual([])
    expect(readHomePluginPins('member-a')).toEqual(['home-writing'])
  })

  it('stops at six plugins and drops a pin when the plugin is removed', () => {
    const slugs = ['a', 'b', 'c', 'd', 'e', 'f', 'g'].map((item) => `plugin-${item}`)
    for (const slug of slugs) pinHomePlugin('member-a', slug)
    expect(readHomePluginPins('member-a')).toEqual(slugs.slice(0, HOME_PLUGIN_PIN_LIMIT))
    unpinHomePlugin('member-a', 'plugin-a')
    expect(readHomePluginPins('member-a')).toEqual(['plugin-b', 'plugin-c', 'plugin-d', 'plugin-e', 'plugin-f'])
    expect(pinHomePlugin('member-a', 'plugin-g')).toEqual([
      'plugin-b',
      'plugin-c',
      'plugin-d',
      'plugin-e',
      'plugin-f',
      'plugin-g'
    ])
  })

  it('ignores a slug the catalog would refuse', () => {
    writeHomePluginPins('member-a', ['OK', 'home-writing', 'home-writing'])
    expect(readHomePluginPins('member-a')).toEqual(['home-writing'])
  })

  it('drops a pin when the materialized package has no home cards', () => {
    writeHomePluginPins('member-a', ['home-writing', 'a3s-finance', 'quiet-tool'])
    expect(
      reconcileHomePluginPins('member-a', [
        { slug: 'a3s-finance' },
        { slug: 'home-writing', home: [{ id: 'notice' }] },
        { slug: 'quiet-tool', home: [] }
      ])
    ).toEqual(['home-writing'])
    expect(readHomePluginPins('member-a')).toEqual(['home-writing'])
  })
})
