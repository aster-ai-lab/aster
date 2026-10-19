import { Button, SearchSelect } from '@a3s-lab/ui'
import { useEffect, useMemo, useState } from 'react'
import { useTranslation } from 'react-i18next'

import {
  SettingDescription,
  SettingDivider,
  SettingGroup,
  SettingRow,
  SettingRowTitle,
  SettingsContentColumn
} from '@renderer/components/SettingsPrimitives'
import {
  filterWorktreesByTime,
  worktreeSinceMs,
  type WorktreeRecord,
  type WorktreeTimeFilter
} from '@shared/git/worktreePorcelain'

import { settingsFieldClassName } from './controlClass'
import { loadOpenWorkspacePath, requestWorktrees } from './worktreeStore'

const FILTERS: WorktreeTimeFilter[] = ['all', '7d', '30d']

export function WorktreeSettings() {
  const { t } = useTranslation()
  const [cwd, setCwd] = useState<string | null>(null)
  const [items, setItems] = useState<WorktreeRecord[]>([])
  const [loaded, setLoaded] = useState(false)
  const [filter, setFilter] = useState<WorktreeTimeFilter>('all')
  const [branch, setBranch] = useState('')
  const [error, setError] = useState<string | null>(null)
  const visible = useMemo(() => filterWorktreesByTime(items, worktreeSinceMs(filter)), [filter, items])

  const refresh = async () => {
    setError(null)
    const path = await loadOpenWorkspacePath()
    setCwd(path)
    if (!path) {
      setItems([])
      setLoaded(true)
      return null
    }
    const result = await requestWorktrees({ cwd: path, action: 'list' })
    setItems(result.items)
    setLoaded(true)
    if (result.error) setError(t('settings.aligned.worktree.failed'))
    return path
  }

  useEffect(() => {
    let active = true
    void loadOpenWorkspacePath()
      .then(async (path) => {
        if (!active) return
        setCwd(path)
        if (!path) {
          setItems([])
          setLoaded(true)
          return
        }
        const result = await requestWorktrees({ cwd: path, action: 'list' })
        if (!active) return
        setItems(result.items)
        setLoaded(true)
        if (result.error) setError(t('settings.aligned.worktree.failed'))
      })
      .catch(() => {
        if (!active) return
        setLoaded(true)
        setError(t('settings.aligned.worktree.unavailable'))
      })
    return () => {
      active = false
    }
  }, [t])

  const create = async () => {
    const name = branch.trim()
    if (!name) return
    const path = await refresh()
    if (!path) return
    const result = await requestWorktrees({ cwd: path, action: 'create', branch: name })
    setItems(result.items)
    if (result.error) setError(t('settings.aligned.worktree.failed'))
  }

  const clear = async () => {
    const path = await refresh()
    if (!path) return
    const result = await requestWorktrees({ cwd: path, action: 'clear', sinceMs: worktreeSinceMs(filter) })
    setItems(result.items)
    if (result.error) setError(t('settings.aligned.worktree.failed'))
  }

  return (
    <SettingsContentColumn>
      <SettingGroup id="setting-worktree-list">
        <SettingDescription>{t('settings.aligned.worktree.desc')}</SettingDescription>
        <div className="mt-3 flex flex-wrap items-center gap-2">
          <Button
            type="button"
            id="setting-worktree-refresh"
            size="sm"
            variant="outline"
            onClick={() => void refresh()}>
            {t('settings.aligned.worktree.refresh')}
          </Button>
          <SearchSelect
            size="sm"
            searchable={false}
            allowClear={false}
            id="setting-worktree-time-filter"
            aria-label={t('settings.aligned.worktree.time')}
            value={filter}
            options={FILTERS.map((id) => ({
              value: id,
              label:
                id === '7d'
                  ? t('settings.aligned.worktree.time_7d')
                  : id === '30d'
                    ? t('settings.aligned.worktree.time_30d')
                    : t('settings.aligned.worktree.time_all')
            }))}
            onChange={(raw) => {
              const next = raw as WorktreeTimeFilter
              if (FILTERS.includes(next)) setFilter(next)
            }}
            className="w-[180px]"
          />
          <Button type="button" id="setting-worktree-clear" size="sm" variant="outline" onClick={() => void clear()}>
            {t('settings.aligned.worktree.clear')}
          </Button>
          <input
            className={`${settingsFieldClassName} h-8 w-36`}
            aria-label={t('settings.aligned.worktree.branch')}
            placeholder={t('settings.aligned.worktree.branch')}
            value={branch}
            onChange={(event) => setBranch(event.target.value)}
          />
          <Button type="button" id="setting-worktree-create" size="sm" variant="outline" onClick={() => void create()}>
            {t('settings.aligned.worktree.create')}
          </Button>
        </div>
        {loaded && !cwd ? (
          <p className="mt-3 text-[13px] text-[#8a8a8a]">{t('settings.aligned.worktree.no_workspace')}</p>
        ) : null}
        {loaded && cwd && visible.length === 0 ? (
          <p className="mt-3 text-[13px] text-[#8a8a8a]">{t('settings.aligned.worktree.empty')}</p>
        ) : null}
        {visible.length > 0 ? (
          <div className="mt-3">
            {visible.map((item) => (
              <div key={item.id}>
                <SettingDivider />
                <SettingRow>
                  <SettingRowTitle>{item.path}</SettingRowTitle>
                  {item.primary ? null : (
                    <Button
                      type="button"
                      size="sm"
                      variant="outline"
                      onClick={() => {
                        if (!cwd) return
                        void requestWorktrees({ cwd, action: 'remove', path: item.path }).then((result) => {
                          setItems(result.items)
                          setError(result.refused ? t('settings.aligned.worktree.remove_dirty') : null)
                        })
                      }}>
                      {t('settings.aligned.worktree.remove')}
                    </Button>
                  )}
                </SettingRow>
              </div>
            ))}
          </div>
        ) : null}
        {error ? (
          <p role="alert" className="mt-3 text-[13px] text-[#b42318]">
            {error}
          </p>
        ) : null}
      </SettingGroup>
    </SettingsContentColumn>
  )
}
