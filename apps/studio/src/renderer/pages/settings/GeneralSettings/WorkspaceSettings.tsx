import type { FC } from 'react'
import { useEffect, useRef, useState } from 'react'
import { useTranslation } from 'react-i18next'

import { Button } from '@a3s-lab/ui/components'
import { usePreference } from '@data/hooks/usePreference'
import {
  SettingDescription,
  SettingDivider,
  SettingGroup,
  SettingRow,
  SettingRowTitle,
  SettingTitle
} from '@renderer/components/SettingsPrimitives'
import { useTheme } from '@renderer/hooks/useTheme'
import { ipcApi } from '@renderer/ipc'
import { toast } from '@renderer/services/toast'

import {
  displayedWorkspacePath,
  expandWorkspacePath,
  normalizeWorkspacePath,
  workspacePreferenceValue
} from './appWorkspace'

export const WorkspaceSettings: FC = () => {
  const { t } = useTranslation()
  const { theme } = useTheme()
  const [stored, setStored] = usePreference('app.workspace.path')
  const [homePath, setHomePath] = useState<string | null>(null)
  const [busy, setBusy] = useState(false)
  const alive = useRef(true)
  const shown = displayedWorkspacePath(stored, homePath ?? '')
  const customized = typeof stored === 'string' && stored.trim().length > 0

  useEffect(() => {
    alive.current = true
    return () => {
      alive.current = false
    }
  }, [])

  useEffect(() => {
    let active = true
    void ipcApi
      .request('app.get_info')
      .then((info) => {
        if (active) setHomePath(info.homePath)
      })
      .catch(() => {
        if (active) setHomePath('')
      })
    return () => {
      active = false
    }
  }, [])

  const savePath = async (raw: string) => {
    const expanded = expandWorkspacePath(raw, homePath ?? '')
    if (normalizeWorkspacePath(expanded) === normalizeWorkspacePath(shown)) return
    const next = workspacePreferenceValue(expanded, homePath ?? '')
    if (next === null) {
      toast.error(t('settings.workspace.invalid'))
      return
    }
    await setStored(next)
  }

  const chooseFolder = async () => {
    setBusy(true)
    try {
      const selected = await window.api.file.selectFolder({
        title: t('settings.workspace.folder'),
        defaultPath: shown || undefined,
        properties: ['openDirectory', 'createDirectory']
      })
      if (!selected) return
      await savePath(selected)
    } catch {
      toast.error(t('settings.workspace.select_failed'))
    } finally {
      if (alive.current) setBusy(false)
    }
  }

  return (
    <SettingGroup theme={theme} data-ui="settings.workspace">
      <SettingTitle>{t('settings.workspace.title')}</SettingTitle>
      <SettingDescription>{t('settings.workspace.description')}</SettingDescription>
      <SettingDivider />
      <SettingRow id="setting-general-workspace-folder" className="scroll-mt-6">
        <SettingRowTitle>{t('settings.workspace.folder')}</SettingRowTitle>
        <div className="flex min-w-0 items-center justify-end gap-2">
          <span className="min-w-0 truncate text-sm leading-4.5 text-foreground select-text" title={shown}>
            {shown}
          </span>
          <div className="flex shrink-0 gap-2">
            {customized ? (
              <Button variant="outline" size="sm" disabled={busy} onClick={() => void setStored('')}>
                {t('settings.workspace.reset')}
              </Button>
            ) : null}
            <Button variant="outline" size="sm" disabled={busy || homePath === null} onClick={() => void chooseFolder()}>
              {t('settings.workspace.change')}
            </Button>
          </div>
        </div>
      </SettingRow>
    </SettingGroup>
  )
}
