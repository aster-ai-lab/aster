import { PopConfirm, SearchInput, Select, SelectContent, SelectItem, SelectTrigger, SelectValue } from '@a3s-lab/ui'
import { Archive, RotateCcw, Trash2 } from 'lucide-react'
import { useEffect, useMemo, useState, type ReactElement } from 'react'
import { useTranslation } from 'react-i18next'

import { dataApiService } from '@data/DataApiService'
import { SettingGroup, SettingsContentColumn } from '@renderer/components/SettingsPrimitives'
import { ipcApi } from '@renderer/ipc'

import {
  ALL_PROJECTS,
  filterArchivedTasks,
  NO_PROJECT,
  sessionToArchivedTask,
  type ArchivedTask
} from './archivedTasks'

const quietButtonClassName =
  'inline-flex h-[30px] shrink-0 items-center justify-center gap-1.5 rounded-[8px] border border-[#e5e5e5] bg-white px-2.5 text-[13px] leading-none text-[#171717] outline-none hover:bg-[#fafafa] focus-visible:ring-2 focus-visible:ring-black/10 disabled:opacity-40'

const actionIconClassName = 'size-3.5 shrink-0 text-current'

const projectOptionClassName =
  'h-9 rounded-[6px] py-0 text-[13px] text-[#3f3f3f] focus:bg-[#f7f7f7] focus:text-[#171717] data-[state=checked]:bg-[#f2f3f5] data-[state=checked]:font-medium data-[state=checked]:text-[#171717]'

/** Radix rejects an empty item value. The filter still uses NO_PROJECT (''). */
const NO_PROJECT_OPTION = '__no_project__'

function projectOptionValue(id: string) {
  return id === NO_PROJECT ? NO_PROJECT_OPTION : id
}

function projectIdFromOption(value: string) {
  return value === NO_PROJECT_OPTION ? NO_PROJECT : value
}

async function loadArchivedTasks(): Promise<ArchivedTask[]> {
  const response = await dataApiService.get('/agent-sessions', { query: { inTrash: true, limit: 200 } })
  const items = Array.isArray(response.items) ? response.items : []
  return items.flatMap((item) => {
    const task = sessionToArchivedTask(item)
    return task ? [task] : []
  })
}

function ConfirmButton({
  id,
  label,
  icon,
  title,
  description,
  confirmLabel,
  cancelLabel,
  tone = 'default',
  disabled = false,
  onConfirm
}: {
  id?: string
  label: string
  icon: ReactElement
  title: string
  description: string
  confirmLabel: string
  cancelLabel: string
  tone?: 'default' | 'danger'
  disabled?: boolean
  onConfirm: () => void | Promise<void>
}) {
  return (
    <PopConfirm
      tone={tone}
      title={title}
      description={description}
      confirmLabel={confirmLabel}
      cancelLabel={cancelLabel}
      disabled={disabled}
      onConfirm={onConfirm}>
      <button type="button" id={id} className={quietButtonClassName} disabled={disabled}>
        {icon}
        {label}
      </button>
    </PopConfirm>
  )
}

export function ArchivedTasksPage() {
  const { t } = useTranslation()
  const [tasks, setTasks] = useState<ArchivedTask[]>([])
  const [loaded, setLoaded] = useState(false)
  const [query, setQuery] = useState('')
  const [projectId, setProjectId] = useState(ALL_PROJECTS)

  const reload = async () => {
    setTasks(await loadArchivedTasks())
    setLoaded(true)
  }

  useEffect(() => {
    let active = true
    void loadArchivedTasks()
      .then((next) => {
        if (!active) return
        setTasks(next)
        setLoaded(true)
      })
      .catch(() => {
        if (active) setLoaded(true)
      })
    return () => {
      active = false
    }
  }, [])

  const visible = useMemo(() => filterArchivedTasks(tasks, query, projectId), [tasks, query, projectId])
  const projects = useMemo(() => {
    const seen = new Map<string, string>()
    for (const task of tasks) {
      const id = task.projectId ?? NO_PROJECT
      if (!seen.has(id)) seen.set(id, task.projectName ?? t('settings.aligned.archive.no_project'))
    }
    return [...seen.entries()]
  }, [tasks, t])

  useEffect(() => {
    if (projectId === ALL_PROJECTS) return
    if (tasks.some((task) => (task.projectId ?? NO_PROJECT) === projectId)) return
    setProjectId(ALL_PROJECTS)
  }, [projectId, tasks])

  const drop = (ids: readonly string[]) => {
    const gone = new Set(ids)
    setTasks((current) => current.filter((task) => !gone.has(task.id)))
  }

  const restoreTask = async (id: string) => {
    try {
      await ipcApi.request('ai.agent.session.restore', { sessionId: id })
      drop([id])
    } catch {
      await reload().catch(() => undefined)
    }
  }

  const destroyTasks = async (ids: string[]) => {
    if (ids.length === 0) return
    try {
      await ipcApi.request('ai.agent.session.delete', { sessionIds: ids, permanent: true })
      drop(ids)
    } catch {
      await reload().catch(() => undefined)
    }
  }

  const projectTasks = (id: string) =>
    tasks.filter((task) => (task.projectId ?? NO_PROJECT) === id).map((task) => task.id)

  const cancelLabel = t('settings.aligned.archive.cancel')
  const deleteTitle = t('settings.aligned.archive.confirm')

  return (
    <SettingsContentColumn>
      <SettingGroup data-ui="settings.archive" className="mt-0 overflow-hidden p-0">
        <div className="flex flex-wrap items-center gap-2 p-4">
          <SearchInput
            id="setting-archive-search"
            size="sm"
            className="h-[30px]"
            containerClassName="h-[30px] w-[220px] max-w-full border-0 bg-[#f5f5f5] shadow-none has-[[data-slot=input-group-control]:focus-visible]:border-0"
            aria-label={t('settings.aligned.archive.search')}
            placeholder={t('settings.aligned.archive.search')}
            value={query}
            onChange={(event) => setQuery(event.target.value)}
            onClear={() => setQuery('')}
            clearLabel={t('common.clear')}
          />
          <Select
            value={projectOptionValue(projectId)}
            onValueChange={(value) => setProjectId(projectIdFromOption(value))}>
            <SelectTrigger
              id="setting-archive-project-filter"
              size="sm"
              className="min-w-[9rem] border-0 bg-[#f5f5f5] px-2.5 text-[13px] text-[#171717] shadow-none focus-visible:border-transparent focus-visible:ring-2 focus-visible:ring-black/10"
              aria-label={t('settings.aligned.archive.all_projects')}>
              <SelectValue />
            </SelectTrigger>
            <SelectContent
              align="start"
              className="rounded-[8px] border-[#ececec] bg-white text-[#171717] shadow-[0_12px_32px_rgba(0,0,0,0.12)]">
              <SelectItem value={ALL_PROJECTS} className={projectOptionClassName}>
                {t('settings.aligned.archive.all_projects')}
              </SelectItem>
              {projects.map(([id, name]) => (
                <SelectItem
                  key={projectOptionValue(id)}
                  value={projectOptionValue(id)}
                  className={projectOptionClassName}>
                  {name}
                </SelectItem>
              ))}
            </SelectContent>
          </Select>
          <ConfirmButton
            id="setting-archive-delete-all"
            label={t('settings.aligned.archive.delete_all')}
            icon={<Trash2 className={actionIconClassName} strokeWidth={1.7} aria-hidden />}
            title={deleteTitle}
            description={t('settings.aligned.archive.delete_all_message')}
            confirmLabel={deleteTitle}
            cancelLabel={cancelLabel}
            tone="danger"
            disabled={!loaded || tasks.length === 0}
            onConfirm={() => destroyTasks(tasks.map((task) => task.id))}
          />
          {projectId !== ALL_PROJECTS ? (
            <ConfirmButton
              label={t('settings.aligned.archive.delete_project')}
              icon={<Trash2 className={actionIconClassName} strokeWidth={1.7} aria-hidden />}
              title={deleteTitle}
              description={t('settings.aligned.archive.delete_project_message', {
                name: projects.find(([id]) => id === projectId)?.[1] ?? projectId
              })}
              confirmLabel={deleteTitle}
              cancelLabel={cancelLabel}
              tone="danger"
              onConfirm={() => destroyTasks(projectTasks(projectId))}
            />
          ) : null}
        </div>
        {loaded && tasks.length === 0 ? (
          <p className="m-0 border-t border-[#efefef] px-4 py-6 text-[13px] leading-5 text-[#8a8a8a]">
            {t('settings.aligned.archive.empty')}
          </p>
        ) : null}
        {tasks.length > 0 && visible.length === 0 ? (
          <p className="m-0 border-t border-[#efefef] px-4 py-6 text-[13px] leading-5 text-[#8a8a8a]">
            {t('settings.aligned.archive.no_results')}
          </p>
        ) : null}
        {visible.length > 0 ? (
          <div className="border-t border-[#efefef]">
            {visible.map((task) => (
              <div
                key={task.id}
                className="group flex h-9 items-center gap-2.5 border-b border-[#efefef] px-4 last:border-b-0 hover:bg-[#f7f7f7]">
                <Archive className="size-4 shrink-0 text-[#8a8a8a]" strokeWidth={1.7} aria-hidden />
                <span className="min-w-0 truncate text-[13px] text-[#3f3f3f] group-hover:text-[#171717]">
                  {task.title}
                </span>
                {task.projectName ? (
                  <span className="shrink-0 text-[12px] leading-5 text-[#8a8a8a]">{task.projectName}</span>
                ) : null}
                <span className="min-w-2 flex-1" />
                <ConfirmButton
                  label={t('settings.aligned.archive.restore')}
                  icon={<RotateCcw className={actionIconClassName} strokeWidth={1.7} aria-hidden />}
                  title={t('settings.aligned.archive.restore_confirm')}
                  description={t('settings.aligned.archive.restore_hint')}
                  confirmLabel={t('settings.aligned.archive.restore')}
                  cancelLabel={cancelLabel}
                  onConfirm={() => restoreTask(task.id)}
                />
                <ConfirmButton
                  label={t('settings.aligned.archive.delete')}
                  icon={<Trash2 className={actionIconClassName} strokeWidth={1.7} aria-hidden />}
                  title={deleteTitle}
                  description={t('settings.aligned.archive.delete_one_message')}
                  confirmLabel={deleteTitle}
                  cancelLabel={cancelLabel}
                  tone="danger"
                  onConfirm={() => destroyTasks([task.id])}
                />
              </div>
            ))}
          </div>
        ) : null}
      </SettingGroup>
    </SettingsContentColumn>
  )
}
