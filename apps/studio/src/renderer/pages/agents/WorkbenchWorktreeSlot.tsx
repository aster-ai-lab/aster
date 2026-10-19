import { useEffect, useState } from 'react'
import { useTranslation } from 'react-i18next'

import { SearchSelect } from '@a3s-lab/ui'
import { dataApiService } from '@data/DataApiService'
import { requestWorktrees } from '@renderer/pages/settings/aligned/worktreeStore'
import type { WorktreeRecord } from '@shared/git/worktreePorcelain'

/** A new task may use a clean linked worktree. The primary checkout and a dirty tree stay out. */
export function selectableWorktrees(items: readonly WorktreeRecord[]): WorktreeRecord[] {
  return items.filter((item) => !item.primary && !item.dirty)
}

export function WorkbenchWorktreeSlot({
  cwd,
  hasMessages,
  onWorkspaceChange
}: {
  cwd?: string
  hasMessages: boolean
  onWorkspaceChange?: (workspaceId: string | null) => void | Promise<void>
}) {
  const { t } = useTranslation()
  const [choices, setChoices] = useState<WorktreeRecord[] | null>(null)
  const [value, setValue] = useState('')

  useEffect(() => {
    if (hasMessages || !cwd) {
      setChoices(null)
      return
    }
    let active = true
    void requestWorktrees({ cwd, action: 'list' })
      .then((result) => {
        if (!active) return
        const next = result.error ? [] : selectableWorktrees(result.items)
        setChoices(next.length > 0 ? next : null)
      })
      .catch(() => {
        if (active) setChoices(null)
      })
    return () => {
      active = false
    }
  }, [cwd, hasMessages])

  if (!choices) return null

  return (
    <div data-ui="workbench.worktree" className="px-1 pt-2">
      <SearchSelect
        data-ui="workbench.worktree.pick"
        aria-label={t('workbench.worktree.label')}
        searchable={false}
        allowClear={false}
        size="sm"
        value={value}
        placeholder={t('workbench.worktree.placeholder')}
        options={choices.map((item) => ({ value: item.path, label: item.path }))}
        onChange={(path) => {
          const picked = choices.find((item) => item.path === path)
          if (!picked) return
          void dataApiService
            .post('/agent-workspaces', { body: { path: picked.path } })
            .then((workspace) => onWorkspaceChange?.(workspace.id))
            .then(() => setValue(picked.path))
            .catch(() => undefined)
        }}
      />
    </div>
  )
}
