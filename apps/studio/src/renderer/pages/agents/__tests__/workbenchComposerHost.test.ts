import { describe, expect, it } from 'vitest'

import { findWorkbenchComposerScope } from '../workbenchComposerHost'

describe('findWorkbenchComposerScope', () => {
  it('finds the tray slot beside the anchor, not inside it', () => {
    document.body.innerHTML = `
      <div id="frame">
        <div data-ui="workbench.composer.anchor">
          <div id="host" data-ui="workbench.composer"></div>
        </div>
        <div id="scope" data-ui="workbench.composer.scope"></div>
      </div>
    `
    const host = document.getElementById('host')
    const scope = document.getElementById('scope')

    expect(host).toBeTruthy()
    expect(findWorkbenchComposerScope(host!)).toBe(scope)
    expect(host!.parentElement?.querySelector("[data-ui~='workbench.composer.scope']")).toBeNull()
  })
})
