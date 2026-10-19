import { describe, expect, it } from 'vitest'

import { aixWorkbenchCatalogueSchema } from '../aixWorkbench'
import { STUDIO_EXTENSION_POINT_CONTRACTS } from '../studioExtensionPoints.generated'

const catalogue = {
  protocolVersion: 1,
  revision: '',
  packages: [],
  extensionPoints: STUDIO_EXTENSION_POINT_CONTRACTS
}

describe('native Studio capability preflight', () => {
  it('accepts the canonical directory and legacy append-only catalogues', () => {
    expect(aixWorkbenchCatalogueSchema.safeParse(catalogue).success).toBe(true)
    expect(aixWorkbenchCatalogueSchema.safeParse({ protocolVersion: 1, revision: '', packages: [] }).success).toBe(true)
  })

  it('refuses a host directory that advertises unsupported replacements, scopes or contracts', () => {
    for (const changed of [
      { ...STUDIO_EXTENSION_POINT_CONTRACTS.sidebar, modes: ['append', 'replace'] },
      { ...STUDIO_EXTENSION_POINT_CONTRACTS.sidebar, scopes: ['subtask'] },
      { ...STUDIO_EXTENSION_POINT_CONTRACTS.sidebar, contractVersion: 2 }
    ]) {
      expect(
        aixWorkbenchCatalogueSchema.safeParse({
          ...catalogue,
          extensionPoints: { ...STUDIO_EXTENSION_POINT_CONTRACTS, sidebar: changed }
        }).success
      ).toBe(false)
    }
  })
})
