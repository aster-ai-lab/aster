import { describe, expect, it } from 'vitest'

import { displayedFileDirectory } from '../FilePreviewPageHeader'

describe('displayedFileDirectory', () => {
  it('shortens folders under the home directory', () => {
    expect(displayedFileDirectory('/Users/me/code/a3s/Cargo.toml', '/Users/me')).toBe('~/code/a3s')
  })

  it('keeps folders outside the home directory and lookalike prefixes intact', () => {
    expect(displayedFileDirectory('/opt/data/report.pdf', '/Users/me')).toBe('/opt/data')
    expect(displayedFileDirectory('/Users/meg/notes.md', '/Users/me')).toBe('/Users/meg')
  })
})
