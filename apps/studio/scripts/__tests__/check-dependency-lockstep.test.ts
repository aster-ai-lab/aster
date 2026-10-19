import { describe, expect, it } from 'vitest'

import { type InstalledPackage, lockstepErrors } from '../check-dependency-lockstep'

const aligned = {
  devDependencies: {
    '@tiptap/core': '3.31.3',
    '@tiptap/extension-list': '3.31.3',
    '@tiptap/y-tiptap': '^3.0.5'
  }
}

const workspace = {
  overrides: { '@tiptap/extension-bubble-menu': '3.31.3', 'prosemirror-view': '1.42.6' },
  patchedDependencies: {
    '@tiptap/extension-list@3.31.3': 'patches/@tiptap__extension-list@3.31.3.patch',
    'other@1.0.0': 'patches/other@1.0.0.patch'
  }
}

const list = (core: string | null): InstalledPackage => ({
  name: '@tiptap/extension-list',
  version: '3.31.3',
  peers: { '@tiptap/core': '3.31.3', '@tiptap/pm': '3.31.3' },
  resolvedPeers: { '@tiptap/core': core, '@tiptap/pm': '3.31.3' }
})

describe('lockstepErrors', () => {
  it('accepts one family version across manifest, overrides, patches and installed peers', () => {
    expect(lockstepErrors(aligned, workspace, [list('3.31.3')])).toEqual([])
  })

  it('rejects the half-landed upgrade that bumped only the patched members', () => {
    const manifest = {
      dependencies: { '@tiptap/extension-list': '3.31.3' },
      devDependencies: { '@tiptap/core': '3.26.1', '@tiptap/extension-list': '3.26.1' }
    }
    const errors = lockstepErrors(manifest, workspace, [list('3.26.1')])
    expect(errors).toContain('@tiptap/extension-list is declared in dependencies and devDependencies')
    expect(errors.some((error) => error.startsWith('@tiptap/* must share one version'))).toBe(true)
    expect(errors).toContain('@tiptap/extension-list@3.31.3 needs @tiptap/core@3.31.3 but resolves @tiptap/core@3.26.1')
  })

  it('rejects overrides and patches that point at another release', () => {
    const errors = lockstepErrors(
      aligned,
      {
        overrides: { '@tiptap/extension-bubble-menu': '3.26.1' },
        patchedDependencies: { '@tiptap/markdown@3.26.1': 'patches/@tiptap__markdown@3.26.1.patch' }
      },
      []
    )
    expect(errors).toEqual([
      'override @tiptap/extension-bubble-menu@3.26.1 differs from the @tiptap/* version 3.31.3',
      'patch @tiptap/markdown@3.26.1 targets 3.26.1, not the @tiptap/* version 3.31.3'
    ])
  })

  it('reports a family peer that is not installed', () => {
    expect(lockstepErrors(aligned, workspace, [list(null)])).toEqual([
      '@tiptap/extension-list@3.31.3 needs @tiptap/core@3.31.3, which is not installed'
    ])
  })

  it('rejects react and react-dom resolving to different releases from one caret range', () => {
    const manifest = { dependencies: { react: '^19.2.0', 'react-dom': '^19.2.0' } }
    const react = { name: 'react', version: '19.3.0', peers: {}, resolvedPeers: {} }
    const dom = {
      name: 'react-dom',
      version: '19.2.3',
      peers: { react: '^19.2.3' },
      resolvedPeers: { react: '19.3.0' }
    }
    expect(lockstepErrors(manifest, {}, [react, dom])).toEqual([
      'installed react/react-dom versions differ: react@19.3.0, react-dom@19.2.3'
    ])
    expect(lockstepErrors(manifest, {}, [react, { ...dom, version: '19.3.0' }])).toEqual([])
  })

  it('leaves independently versioned members and other packages alone', () => {
    const manifest = { devDependencies: { ...aligned.devDependencies, '@tiptap/y-tiptap': '^3.0.5', other: '2.0.0' } }
    expect(lockstepErrors(manifest, workspace, [])).toEqual([])
  })
})
