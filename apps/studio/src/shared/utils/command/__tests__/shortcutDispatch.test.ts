import { readdirSync, readFileSync } from 'node:fs'
import { join } from 'node:path'
import { fileURLToPath, URL as NodeUrl } from 'node:url'

import { describe, expect, it } from 'vitest'

import {
  getCommandDefaultShortcutPreference,
  REGISTERED_KEYBINDINGS,
  resolveCommandByKeybinding
} from '@shared/utils/command'
import type { ShortcutBinding } from '@shared/utils/shortcut'

const WINDOW_LOCAL_ONLY = new Set([
  'app.window.show',
  'quick_assistant.toggle',
  'selection.capture_text',
  'selection.toggle'
])

const ACTIVE_CONTEXT = {
  'feature.screenshot.enabled': true,
  'webview.focused': false
}

function rendererSources(): string[] {
  const root = fileURLToPath(new NodeUrl('../../../../renderer', import.meta.url))
  const sources: string[] = []
  const visit = (directory: string) => {
    for (const entry of readdirSync(directory, { withFileTypes: true })) {
      if (entry.name === 'node_modules' || entry.name === '__tests__') continue
      const path = join(directory, entry.name)
      if (entry.isDirectory()) {
        visit(path)
        continue
      }
      if (!/\.(ts|tsx)$/.test(entry.name) || /\.(test|spec)\./.test(entry.name)) continue
      sources.push(readFileSync(path, 'utf8'))
    }
  }
  visit(root)
  return sources
}

function sourceHasHandler(sources: string[], id: string): boolean {
  const quoted = id.replace(/[.*+?^${}()|[\]\\]/g, '\\$&')
  const patterns = [
    new RegExp(`useCommandHandler\\(\\s*['"]${quoted}['"]`),
    new RegExp(`shortcut\\s*=\\s*['"]${quoted}['"]`),
    new RegExp(`shortcut\\s*=\\s*\\{[^}]*['"]${quoted}['"]`)
  ]
  return sources.some((source) => patterns.some((pattern) => pattern.test(source)))
}

describe('settings shortcut dispatch', () => {
  const sources = rendererSources()

  it('resolves every window shortcut from the renderer, except the four that need an OS hotkey', () => {
    for (const rule of REGISTERED_KEYBINDINGS) {
      if (WINDOW_LOCAL_ONLY.has(rule.command)) {
        expect(rule.scope).toBe('main')
        continue
      }

      const preference = getCommandDefaultShortcutPreference(rule.command, 'darwin')
      expect(preference?.binding.length, rule.command).toBeGreaterThan(0)
      expect(
        resolveCommandByKeybinding({
          binding: preference?.binding ?? [],
          context: ACTIVE_CONTEXT,
          platform: 'darwin',
          preferences: {
            [rule.command]: { binding: preference?.binding ?? [], enabled: true }
          },
          scope: 'renderer'
        })
      ).toBe(rule.command)

      if (rule.command === 'app.fullscreen.exit') {
        const runtime = sources.find((source) => source.includes("usePreference('shortcut.app.fullscreen.exit')"))
        expect(runtime).toContain("'window.is_full_screen'")
        expect(runtime).toContain("'window.set_full_screen'")
        continue
      }
      expect(sourceHasHandler(sources, rule.command), rule.command).toBe(true)
    }
  })

  it('leaves OS-global shortcuts off the renderer stream', () => {
    expect(
      resolveCommandByKeybinding({
        binding: ['CommandOrControl', 'E'],
        context: { 'feature.quick_assistant.enabled': true },
        platform: 'darwin',
        preferences: {
          'quick_assistant.toggle': { binding: ['CommandOrControl', 'E'], enabled: true }
        },
        scope: 'renderer'
      })
    ).toBeUndefined()
  })

  it('keeps the macOS close shortcut off other platforms and accepts every zoom spelling', () => {
    expect(
      resolveCommandByKeybinding({
        binding: ['CommandOrControl', 'Shift', 'W'],
        context: {},
        platform: 'win32',
        scope: 'renderer'
      })
    ).toBeUndefined()

    const zoom = (binding: ShortcutBinding) =>
      resolveCommandByKeybinding({
        binding,
        context: {},
        platform: 'darwin',
        scope: 'renderer'
      })

    expect(zoom(['CommandOrControl', '='])).toBe('app.zoom.in')
    expect(zoom(['CommandOrControl', 'Shift', '='])).toBe('app.zoom.in')
    expect(zoom(['CommandOrControl', 'numadd'])).toBe('app.zoom.in')
    expect(zoom(['CommandOrControl', '-'])).toBe('app.zoom.out')
    expect(zoom(['CommandOrControl', '0'])).toBe('app.zoom.reset')
  })
})
