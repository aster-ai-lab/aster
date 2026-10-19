import { type ReactNode, useLayoutEffect, useState } from 'react'
import { createPortal } from 'react-dom'

/** Places an action in the settings title row when that row is mounted. */
export function SettingsHeaderAction({ children }: { children: ReactNode }) {
  const [host, setHost] = useState<HTMLElement | null>(null)
  useLayoutEffect(() => {
    setHost(document.querySelector<HTMLElement>('[data-ui="settings.header-action"]'))
  }, [])
  if (!host) return <>{children}</>
  return createPortal(children, host)
}
