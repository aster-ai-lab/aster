import { realpathSync } from 'node:fs'
import { resolve } from 'node:path'

import { describe, expect, it } from 'vitest'

import { flowDesignerResolveAlias } from '../flowDesignerAlias'

describe('shared Flow designer sources', () => {
  it('resolves every Flow entry to the workspace engine authoring package', () => {
    const appRoot = resolve(import.meta.dirname, '../..')
    const sourceRoot = realpathSync(resolve(appRoot, '../../crates/flow/packages/ui/src'))
    const entries = Object.entries(flowDesignerResolveAlias(appRoot)).filter(([name]) =>
      name.startsWith('@a3s-lab/flow-ui')
    )

    expect(entries.length).toBeGreaterThan(0)
    for (const [, path] of entries) {
      expect(realpathSync(path).startsWith(`${sourceRoot}/`)).toBe(true)
    }
  })
})
