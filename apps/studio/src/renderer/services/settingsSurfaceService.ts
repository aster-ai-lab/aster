import type { CommandId } from '@shared/utils/command'

const SETTINGS_SURFACE_COMMANDS: ReadonlySet<CommandId> = new Set([
  'app.settings.open',
  'app.zoom.in',
  'app.zoom.out',
  'app.zoom.reset',
  'app.window.close',
  'screenshot.capture',
  'tab.close',
  'tab.next',
  'tab.prev'
])

/** Settings overlays suspend commands owned by the mounted page underneath. */
export class SettingsSurfaceService {
  private active = false

  setActive(active: boolean): void {
    this.active = active
  }

  isActive(): boolean {
    return this.active
  }

  isCommand(command: CommandId): boolean {
    return SETTINGS_SURFACE_COMMANDS.has(command)
  }
}

export const settingsSurfaceService = new SettingsSurfaceService()
export const setSettingsSurfaceActive = (active: boolean): void => settingsSurfaceService.setActive(active)
export const isSettingsSurfaceActive = (): boolean => settingsSurfaceService.isActive()
export const isSettingsSurfaceCommand = (command: CommandId): boolean => settingsSurfaceService.isCommand(command)
