import { readFileSync } from 'node:fs'
import { dirname, join } from 'node:path'
import { fileURLToPath } from 'node:url'

import { describe, expect, it } from 'vitest'

const studio = join(dirname(fileURLToPath(import.meta.url)), '../../')

const centered = [
  'drive/DrivePanel.tsx',
  'drive/SaveFindingDialog.tsx',
  'pages/agents/WorkScheduledTasks.tsx',
  'pages/agents/WorkbenchHome.tsx'
]

const anchored = [
  'components/account/AccountCenterMenu.tsx',
  'pages/agents/WorkbenchWorkspaceChrome.tsx',
  'pages/messages/MessageEmojiPanel.tsx',
  'components/workInbox/WorkInboxPopup.tsx'
]

const enterSurfaces = [
  'pages/agents/AppletPermissionDialog.tsx',
  'pages/mail/MailTeamPicker.tsx',
  'pages/messages/HistoryDialog.tsx',
  'pages/settings/EvolutionSettings/HarnessFigure.tsx',
  'pages/settings/EvolutionSettings/HarnessFlowDesigner.tsx'
]

describe('hand-drawn dialog motion', () => {
  it('keeps centered dialogs on the resting scrim and surface', () => {
    for (const path of centered) {
      const source = readFileSync(join(studio, path), 'utf8')
      expect(source, path).toContain('a3s-dialog-scrim')
      expect(source, path).toContain('a3s-dialog-surface')
      expect(source, path).toContain('useRestingDialog')
      expect(source, path).not.toMatch(/slide-in-from-|animate-in/)
    }
  })

  it('fades in-page dialogs in place and keeps them for the leave', () => {
    for (const path of enterSurfaces) {
      const source = readFileSync(join(studio, path), 'utf8')
      expect(source, path).toContain('a3s-dialog-surface')
      expect(source, path).toContain('useRestingDialog')
      expect(source, path).toContain('data-state')
      expect(source, path).not.toMatch(/slide-in-from-|animate-in/)
    }
  })

  it('grows the applet launcher on the scale property', () => {
    const css = readFileSync(join(studio, 'pages/agents/workbenchComposer.css'), 'utf8')
    const start = css.indexOf('@keyframes workbench-launchpad-in')
    const end = css.indexOf('@media (prefers-reduced-motion: reduce)', start)
    const body = css.slice(start, end)
    expect(start).toBeGreaterThan(-1)
    expect(body).not.toMatch(/transform:/)
    expect(body).toContain('scale: 0.98')
    expect(body).not.toContain('scale: 0.62')
    expect(body).not.toContain('scale: 0.86')
  })

  it('rises anchored panels with the shared popup class and keeps the leave', () => {
    for (const path of anchored) {
      const source = readFileSync(join(studio, path), 'utf8')
      expect(source, path).toContain('a3s-popup-rise')
      expect(source, path).toContain('useRestingDialog')
      expect(source, path).toContain('data-state')
      expect(source, path).not.toMatch(/slide-in-from-|animate-in/)
    }
  })
})
