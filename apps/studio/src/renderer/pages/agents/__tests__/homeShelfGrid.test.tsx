import { fireEvent, render } from '@testing-library/react'
import { beforeEach, describe, expect, it } from 'vitest'

import { publishAixCatalogue } from '@renderer/services/plugins/aixCatalogueStore'
import type { AixWorkbenchCatalogue } from '@shared/types/aixWorkbench'

import { readHomePluginPins, writeHomePluginPins } from '../homePins'
import { WorkbenchFeatureStrip } from '../WorkbenchFeatureStrip'

const REVISION = `sha256:${'ab'.repeat(32)}`

function catalogue(): AixWorkbenchCatalogue {
  const pack = (slug: string, home: AixWorkbenchCatalogue['packages'][number]['home']) => ({
    slug,
    packageId: `use/a3s/${slug}`,
    generation: 1,
    revision: REVISION,
    containers: [],
    views: [],
    home
  })
  return {
    protocolVersion: 1,
    revision: '',
    packages: [
      pack('home-writing', [
        {
          id: 'notice',
          shelf: 'write',
          title: '通知',
          description: '事项',
          order: 10,
          action: 'writing',
          writingType: 'notice',
          tag: '通知'
        }
      ]),
      pack('home-create', [
        {
          id: 'ppt',
          shelf: 'create',
          title: '幻灯片',
          description: '',
          order: 10,
          action: 'unavailable',
          cover: 'ppt'
        }
      ])
    ]
  }
}

describe('home shelf grid', () => {
  beforeEach(() => {
    localStorage.clear()
    writeHomePluginPins(null, ['home-writing', 'home-create'])
    publishAixCatalogue(catalogue())
  })

  it('removes a shelf from the right-click menu', () => {
    render(<WorkbenchFeatureStrip onCommitDraft={() => {}} />)
    const shelf = document.querySelector('[data-shelf="home-writing"]') as HTMLElement
    expect(shelf.querySelector('[data-ui="workbench.home.layout.move"]')).toBeNull()
    expect(shelf.querySelector('button[data-ui="workbench.home.pin.remove"]')).toBeNull()
    fireEvent.contextMenu(shelf)
    const remove = document.querySelector('[data-ui="workbench.home.pin.remove"]')
    expect(remove?.querySelector('svg')).toBeTruthy()
    fireEvent.click(remove!)
    expect(document.querySelector('[data-shelf="home-writing"]')).toBeNull()
    expect(readHomePluginPins(null)).toEqual(['home-create'])
  })

  it('keeps installed plugins off the home until they are pinned', () => {
    writeHomePluginPins(null, [])
    render(<WorkbenchFeatureStrip onCommitDraft={() => {}} />)
    expect(document.querySelector('[data-ui="workbench.features"]')).toBeNull()
    expect(document.querySelector('[data-feature="write"]')).toBeNull()
  })

  it('drops a pinned plugin that has no home cards', () => {
    const current = catalogue()
    publishAixCatalogue({
      ...current,
      packages: [
        ...current.packages,
        {
          slug: 'a3s-finance',
          packageId: 'use/a3s/finance',
          generation: 1,
          revision: REVISION,
          containers: [],
          views: [{ id: 'chart', title: '金融分析', point: 'studio.sidebar', order: 40 }],
          home: undefined
        }
      ]
    })
    writeHomePluginPins(null, ['home-writing', 'a3s-finance'])
    render(<WorkbenchFeatureStrip onCommitDraft={() => {}} />)
    expect(document.querySelector('[data-shelf="a3s-finance"]')).toBeNull()
    expect(document.querySelector('[data-feature="launch"]')).toBeNull()
    expect(document.querySelector('[data-shelf="home-writing"]')).toBeTruthy()
    expect(readHomePluginPins(null)).toEqual(['home-writing'])
  })
})
