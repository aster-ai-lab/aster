import { describe, expect, it } from 'vitest'

import {
  defaultAppWorkspacePath,
  displayedWorkspacePath,
  knowledgeCorpusAllowed,
  expandWorkspacePath,
  fileMentionRoot,
  workspacePreferenceValue
} from '../appWorkspace'

describe('app workspace path', () => {
  it('joins the home directory with .a3s', () => {
    expect(defaultAppWorkspacePath('/Users/roylin')).toBe('/Users/roylin/.a3s')
    expect(defaultAppWorkspacePath('/Users/roylin/')).toBe('/Users/roylin/.a3s')
    expect(defaultAppWorkspacePath('C:\\Users\\roylin')).toBe('C:\\Users\\roylin\\.a3s')
  })

  it('shows the default directory when nothing is stored', () => {
    expect(displayedWorkspacePath('', '/Users/roylin')).toBe('/Users/roylin/.a3s')
    expect(displayedWorkspacePath(null, '/Users/roylin')).toBe('/Users/roylin/.a3s')
    expect(displayedWorkspacePath('   ', '/Users/roylin')).toBe('/Users/roylin/.a3s')
  })

  it('shows a stored absolute directory', () => {
    expect(displayedWorkspacePath('/tmp/work/', '/Users/roylin')).toBe('/tmp/work')
    expect(displayedWorkspacePath('D:\\work', 'C:\\Users\\roylin')).toBe('D:\\work')
  })

  it('stores an empty value when the chosen folder is the default', () => {
    expect(workspacePreferenceValue('/Users/roylin/.a3s/', '/Users/roylin')).toBe('')
    expect(workspacePreferenceValue('C:\\Users\\roylin\\.a3s', 'C:\\Users\\roylin')).toBe('')
  })

  it('stores another absolute folder and rejects a relative path or a filesystem root', () => {
    expect(workspacePreferenceValue('/Users/roylin/Work', '/Users/roylin')).toBe('/Users/roylin/Work')
    expect(workspacePreferenceValue('Work', '/Users/roylin')).toBeNull()
    expect(workspacePreferenceValue('/', '/Users/roylin')).toBeNull()
    expect(workspacePreferenceValue('C:\\', 'C:\\Users\\roylin')).toBeNull()
    expect(workspacePreferenceValue('C:', 'C:\\Users\\roylin')).toBeNull()
  })

  it('expands a home-relative folder', () => {
    expect(expandWorkspacePath('~/Work', '/Users/roylin')).toBe('/Users/roylin/Work')
    expect(expandWorkspacePath('~', '/Users/roylin/')).toBe('/Users/roylin')
    expect(workspacePreferenceValue(expandWorkspacePath('~/Work', '/Users/roylin'), '/Users/roylin')).toBe(
      '/Users/roylin/Work'
    )
  })

  it('lists the selected project, and otherwise the app workspace', () => {
    expect(fileMentionRoot('/tmp/project/', '/Users/roylin/.a3s')).toBe('/tmp/project')
    expect(fileMentionRoot('  ', '/Users/roylin/.a3s/')).toBe('/Users/roylin/.a3s')
    expect(fileMentionRoot(null, '')).toBe('')
  })

  it('compiles a chosen workspace and refuses the home directory', () => {
    expect(knowledgeCorpusAllowed('/Users/roylin/.a3s', '/Users/roylin')).toBe(true)
    expect(knowledgeCorpusAllowed('/tmp/work', '/Users/roylin')).toBe(true)
    expect(knowledgeCorpusAllowed('/Users/roylin', '/Users/roylin')).toBe(false)
    expect(knowledgeCorpusAllowed('/Users/roylin/', '/Users/roylin')).toBe(false)
    expect(knowledgeCorpusAllowed('/', '/Users/roylin')).toBe(false)
    expect(knowledgeCorpusAllowed('/tmp/work', '')).toBe(false)
  })

  it('leaves the default empty when the home directory is missing', () => {
    expect(defaultAppWorkspacePath('')).toBe('')
    expect(defaultAppWorkspacePath('/')).toBe('')
    expect(displayedWorkspacePath('', '')).toBe('')
  })
})
