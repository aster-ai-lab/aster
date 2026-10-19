import { createContext, useContext } from 'react'

/** Closes the settings overlay and returns to the page underneath. Null outside the shell. */
export const SettingsLeaveContext = createContext<(() => void) | null>(null)

export function useSettingsLeave(): () => void {
  return useContext(SettingsLeaveContext) ?? (() => undefined)
}
