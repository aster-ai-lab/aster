import { afterEach, describe, expect, it, vi } from 'vitest'

import { launchableUiPlugin, openInstalledUiPlugin } from '@renderer/pages/plugins/pluginLaunch'
import { type AixWorkbenchCatalogue, aixWorkbenchCatalogueSchema } from '@shared/types/aixWorkbench'

import { aixAppletForPage, aixAppletViews, aixWindowViews, findAixApplet, findAixWindow } from '../aixApplets'
import { encodeAixWorkbenchRouteKey } from '../aixWorkbenchIdentity'

const revision = `sha256:${'a'.repeat(64)}`
const view = (id: string, point: string, container: string | null = null) => ({
  id,
  title: id,
  description: '',
  icon: 'bot',
  order: 1,
  point,
  container
})
const catalogue: AixWorkbenchCatalogue = aixWorkbenchCatalogueSchema.parse({
  protocolVersion: 1,
  revision,
  packages: [
    {
      slug: 'a3s-g1-arm',
      packageId: 'use/a3s/g1-arm',
      generation: 1,
      revision,
      containers: [],
      views: [view('workbench', 'studio.applets')]
    },
    {
      slug: 'a3s-lab-page',
      packageId: 'use/a3s/lab-page',
      generation: 1,
      revision,
      containers: [{ id: 'lab', title: 'Lab', icon: 'bot', order: 1, location: 'activitybar' }],
      views: [view('lab', 'studio.sidebar', 'lab'), view('editor', 'studio.editor')]
    }
  ]
})
const entry = (slug: string) => ({
  slug,
  name: '宇树 G1 人形机器人',
  surfaces: ['skill', 'agent', 'ui'],
  available: true
})

type Internals = { invoke?: (command: string, args?: Record<string, unknown>) => Promise<unknown> }

describe('AIX mini apps', () => {
  afterEach(() => {
    Object.assign(globalThis, { __TAURI_INTERNALS__: undefined })
  })

  it('finds only studio.applets views outside a view container', () => {
    expect(aixAppletViews(catalogue.packages[0]).map((item) => item.id)).toEqual(['workbench'])
    expect(aixAppletViews(catalogue.packages[1])).toEqual([])
    expect(findAixApplet(catalogue, 'a3s-lab-page')).toBeNull()
    expect(findAixApplet(catalogue, 'a3s-g1-arm', 'other')).toBeNull()
    const found = findAixApplet(catalogue, 'a3s-g1-arm')
    const key = encodeAixWorkbenchRouteKey({ package: catalogue.packages[0], kind: 'view', localId: 'workbench' })
    expect(found?.page).toBe(`aix.a3s-g1-arm/view.${key}`)
    expect(aixAppletForPage(catalogue, 'aix.a3s-g1-arm', `view.${key}`)?.view.id).toBe('workbench')
    expect(aixAppletForPage(catalogue, 'aix.a3s-lab-page', `view.${key}`)).toBeNull()
    expect(aixAppletForPage(catalogue, 'studio.mini_app', `view.${key}`)).toBeNull()
  })

  it('does not offer 打开 for a package whose only window is a homepage composer editor', () => {
    const mode = {
      id: 'slides',
      contractVersion: 1,
      title: '演示文稿',
      description: '在首页输入框里按主题生成演示文稿',
      icon: 'panels',
      agent: 'presenton',
      editor: 'workbench',
      scope: 'workspace',
      context: ['workspace'],
      order: 20
    }
    const presenton = aixWorkbenchCatalogueSchema.parse({
      protocolVersion: 1,
      revision,
      packages: [
        {
          slug: 'a3s-presenton',
          packageId: 'use/a3s/presenton',
          generation: 1,
          revision,
          containers: [],
          views: [view('workbench', 'studio.applets')],
          composerModes: [mode]
        },
        {
          ...catalogue.packages[1],
          composerModes: [{ ...mode, id: 'build', editor: 'editor', agent: 'builder' }]
        }
      ]
    })
    const installed = { ...entry('a3s-presenton'), surfaces: ['skill', 'agent', 'ui'] }
    expect(launchableUiPlugin(installed, true, presenton)).toBeNull()
    expect(findAixWindow(presenton, 'a3s-presenton', 'workbench')?.view.id).toBe('workbench')
    expect(launchableUiPlugin(entry('a3s-lab-page'), true, presenton)).toMatchObject({ view: { id: 'editor' } })
    expect(launchableUiPlugin(entry('a3s-g1-arm'), true, catalogue)).toMatchObject({ view: { id: 'workbench' } })
  })

  it('opens an installed AIX mini app in its own system window', async () => {
    const invoke = vi.fn<NonNullable<Internals['invoke']>>(() => Promise.resolve(null))
    Object.assign(globalThis, { __TAURI_INTERNALS__: { invoke } satisfies Internals })
    expect(launchableUiPlugin(entry('a3s-g1-arm'), false, catalogue)).toBeNull()
    expect(launchableUiPlugin(entry('a3s-lab-page'), true, catalogue)).toMatchObject({ view: { id: 'editor' } })
    expect(launchableUiPlugin(entry('a3s-g1-arm'), true)).toBeNull()
    expect(await openInstalledUiPlugin(entry('a3s-g1-arm'), true, catalogue)).toBe(true)
    expect(invoke).toHaveBeenCalledWith('open_aix_applet_window', {
      slug: 'a3s-g1-arm',
      view: 'workbench',
      title: '宇树 G1 人形机器人'
    })
  })

  it('opens verified editors and ActivityBar containers without admitting document or composer actions', async () => {
    const invoke = vi.fn<NonNullable<Internals['invoke']>>(() => Promise.resolve(0))
    Object.assign(globalThis, { __TAURI_INTERNALS__: { invoke } satisfies Internals })
    expect(await openInstalledUiPlugin(entry('a3s-lab-page'), true, catalogue)).toBe(true)
    expect(invoke).toHaveBeenLastCalledWith('open_aix_applet_window', {
      slug: 'a3s-lab-page',
      view: 'editor',
      title: entry('').name
    })
    const item = catalogue.packages[1]
    const mixed = {
      ...item,
      views: [
        ...item.views,
        { ...item.views[0], id: 'tree', presentation: 'tree' as const },
        { ...item.views[1], id: 'document', point: 'studio.editor.custom' as const },
        { ...item.views[1], id: 'action', point: 'studio.composer.actions' as const }
      ]
    }
    expect(aixWindowViews(mixed).map((candidate) => candidate.id)).toEqual(['lab', 'editor', 'tree'])
    expect(findAixWindow({ ...catalogue, packages: [mixed] }, item.slug, 'lab')?.view.id).toBe('lab')
    const key = encodeAixWorkbenchRouteKey({ package: item, kind: 'container', localId: 'lab' })
    expect(findAixWindow(catalogue, item.slug, 'lab')?.page).toBe(`aix.${item.slug}/page.${key}`)
    expect(findAixWindow(catalogue, 'uninstalled')).toBeNull()
  })

  it('reports a refused or shell-less window as not opened', async () => {
    expect(await openInstalledUiPlugin(entry('a3s-g1-arm'), true, catalogue)).toBe(false)
    Object.assign(globalThis, { __TAURI_INTERNALS__: { invoke: () => Promise.reject(new Error('refused')) } })
    expect(await openInstalledUiPlugin(entry('a3s-g1-arm'), true, catalogue)).toBe(false)
  })
})
