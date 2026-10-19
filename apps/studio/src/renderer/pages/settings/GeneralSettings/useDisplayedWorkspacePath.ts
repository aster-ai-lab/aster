import { useEffect, useState } from 'react'

import { usePreference } from '@renderer/data/hooks/usePreference'
import { ipcApi } from '@renderer/ipc'
import { readStudioHomePath } from '@shared/studioRuntime'

import { displayedWorkspacePath } from './appWorkspace'

/** Settings workspace the documents tree and the composer `@` panel share. */
export function useDisplayedWorkspacePath(): string {
  const [stored] = usePreference('app.workspace.path')
  const [homePath, setHomePath] = useState(() => readStudioHomePath())

  useEffect(() => {
    if (homePath) return
    let active = true
    void ipcApi
      .request('app.get_info')
      .then((info) => {
        if (active && info && typeof info.homePath === 'string' && info.homePath) setHomePath(info.homePath)
      })
      .catch(() => undefined)
    return () => {
      active = false
    }
  }, [homePath])

  return displayedWorkspacePath(typeof stored === 'string' ? stored : '', homePath)
}
