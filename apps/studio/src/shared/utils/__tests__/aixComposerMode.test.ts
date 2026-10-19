import { readFileSync } from 'node:fs'
import process from 'node:process'

import { describe, expect, it } from 'vitest'

import { aixWorkbenchCatalogueSchema } from '../../types/aixWorkbench'
import { STUDIO_EXTENSION_POINT_CONTRACTS } from '../../types/studioExtensionPoints.generated'

const mode = {
  id: 'build',
  contractVersion: 1,
  title: 'Build workcell',
  description: 'Construct a scene in the selected project',
  icon: 'boxes',
  agent: 'builder',
  editor: 'editor',
  scope: 'workspace',
  context: ['workspace', 'document'],
  order: 20
}
const item = {
  slug: 'probe-composer-mode',
  packageId: 'use/a3s/probe-composer-mode',
  generation: 1,
  revision: `sha256:${'a'.repeat(64)}`,
  containers: [],
  views: [
    {
      id: 'editor',
      title: 'Editor',
      description: '',
      icon: 'package',
      order: 100,
      container: null,
      point: 'studio.applets'
    }
  ],
  composerModes: [mode]
}
const catalogue = (pkg: unknown) => ({
  protocolVersion: 1,
  revision: '',
  extensionPoints: STUDIO_EXTENSION_POINT_CONTRACTS,
  packages: [pkg]
})

describe('AIX composer mode wire contract', () => {
  // Catch mode context/agent identity being stripped before a host can freeze a run selection.
  it('preserves explicit execution identity and context while accepting legacy packages', () => {
    const parsed = aixWorkbenchCatalogueSchema.parse(catalogue(item))
    expect(parsed.packages[0].composerModes?.[0]).toEqual(mode)
    const drafted = aixWorkbenchCatalogueSchema.parse(
      catalogue({ ...item, composerModes: [{ ...mode, keptNotice: 'draft' }] })
    )
    expect(drafted.packages[0].composerModes?.[0].keptNotice).toBe('draft')
    const legacy = { ...item, composerModes: undefined }
    expect(aixWorkbenchCatalogueSchema.parse(catalogue(legacy)).packages[0].composerModes).toBeUndefined()
  })

  // Catch untrusted metadata broadening privileges or resolving a mode into a foreign view.
  it('refuses incompatible scopes, contracts, duplicates, references and authority fields', () => {
    for (const changed of [
      { contractVersion: 2 },
      { scope: 'turn' },
      { scope: 'window' },
      { scope: 'unknown' },
      { title: ' ' },
      { title: 'x'.repeat(65) },
      { description: 'x'.repeat(241) },
      { agent: 'foreign/agent' },
      { editor: 'missing' },
      { context: ['workspace', 'workspace'] },
      { context: ['secrets'] },
      { context: 'workspace' },
      { order: 1.5 },
      { permissionMode: 'bypassPermissions' },
      { provider: 'foreign' },
      { point: 'studio.composer.actions' },
      { keptNotice: 'package' }
    ]) {
      expect(
        aixWorkbenchCatalogueSchema.safeParse(catalogue({ ...item, composerModes: [{ ...mode, ...changed }] })).success,
        JSON.stringify(changed)
      ).toBe(false)
    }
    expect(aixWorkbenchCatalogueSchema.safeParse(catalogue({ ...item, composerModes: [mode, mode] })).success).toBe(
      false
    )
    expect(
      aixWorkbenchCatalogueSchema.safeParse(
        catalogue({ ...item, composerModes: Array.from({ length: 33 }, (_, n) => ({ ...mode, id: `mode-${n}` })) })
      ).success
    ).toBe(false)
    expect(
      aixWorkbenchCatalogueSchema.safeParse(
        catalogue({ ...item, views: [{ ...item.views[0], point: 'studio.composer.actions' }] })
      ).success
    ).toBe(false)
  })

  const nativePath = process.env.A3S_CLOUD_TEST_AIX_MODE_CATALOGUE_INPUT
  // Catch drift between the real archive's native projection and the renderer's strict parser.
  it.skipIf(!nativePath)('consumes the catalogue emitted by native installation of the actual archive', () => {
    const native = aixWorkbenchCatalogueSchema.parse(JSON.parse(readFileSync(nativePath!, 'utf8')))
    const installed = native.packages.find((pkg) => pkg.packageId === 'use/a3s/probe-composer-mode')
    expect(installed?.composerModes?.map((entry) => entry.id)).toEqual(['build', 'inspect'])
    expect(installed?.composerModes?.[0].editor).toBe('editor')
    expect(installed?.composerModes?.[1].scope).toBe('session')
  })
})
