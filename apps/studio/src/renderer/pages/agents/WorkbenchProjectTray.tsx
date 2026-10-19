import { ChevronDown, Folder, X } from 'lucide-react'
import { useCallback, useEffect, useState } from 'react'
import { useTranslation } from 'react-i18next'

import { WorkspaceSelector } from '@renderer/components/resourceCatalog/selectors'
import { useQuery } from '@renderer/data/hooks/useDataApi'
import { isKnowledgeCompileWorkbenchRow } from '@renderer/pages/knowledge/knowledgeCompileAgent'
import { AGENT_WORKSPACE_TYPE } from '@shared/data/api/schemas/agentWorkspaces'

import { WorkbenchWorkspaceFilePicker, type WorkbenchWorkspaceFilesControl } from './WorkbenchWorkspaceFiles'

const PENDING_PROJECT_KEY = 'workbench.pending_project:v1'

type PendingStorage = Pick<Storage, 'getItem' | 'setItem' | 'removeItem'>

function defaultStorage(): PendingStorage | null {
  try {
    return globalThis.localStorage ?? null
  } catch {
    return null
  }
}

export function readPendingProject(storage = defaultStorage()): string | null {
  try {
    return storage?.getItem(PENDING_PROJECT_KEY) || null
  } catch {
    return null
  }
}

export function writePendingProject(projectId: string | null, storage = defaultStorage()) {
  try {
    if (projectId) storage?.setItem(PENDING_PROJECT_KEY, projectId)
    else storage?.removeItem(PENDING_PROJECT_KEY)
  } catch {
    // Private mode or quota: the choice still lives in component state for this session.
  }
}

const PROJECT_SELECT_EVENT = 'workbench:select-project'

/** Selects a project for the home composer, whether or not its tray is mounted yet. */
export function selectWorkbenchProject(projectId: string) {
  writePendingProject(projectId)
  globalThis.dispatchEvent?.(new CustomEvent<string>(PROJECT_SELECT_EVENT, { detail: projectId }))
}

export interface WorkbenchProjectSelectionArgs {
  workspace?: { type?: string; name?: string; path?: string } | null
  workspaceId?: string | null
  /** Present once the composer is bound to a session; until then the pick is held as pending. */
  onWorkspaceChange?: (workspaceId: string | null) => void | Promise<void>
  enabled?: boolean
  /** A task conversation shows the project and ignores further picks. */
  locked?: boolean
}

export interface WorkbenchProjectSelection {
  selectedId: string | null
  selectedName: string | null
  selectedPath: string | null
  bound: boolean
  changeProject: (projectId: string | null) => Promise<void>
  pickerOpen: boolean
  setPickerOpen: (open: boolean) => void
}

export function useWorkbenchProjectSelection({
  workspace,
  workspaceId,
  onWorkspaceChange,
  enabled = true,
  locked = false
}: WorkbenchProjectSelectionArgs): WorkbenchProjectSelection {
  const [pendingId, setPendingId] = useState(() => readPendingProject())
  const [pickerOpen, setPickerOpen] = useState(false)
  const { data: rows } = useQuery('/agent-workspaces', { enabled })
  const projects = (Array.isArray(rows) ? rows : []).filter(
    (row) => row.type === AGENT_WORKSPACE_TYPE.USER && !isKnowledgeCompileWorkbenchRow({ path: row.path })
  )
  const boundId = workspace?.type === AGENT_WORKSPACE_TYPE.USER ? (workspaceId ?? null) : null
  const selectedId = onWorkspaceChange ? boundId : pendingId
  const selectedProject = projects.find((project) => project.id === selectedId)
  const selectedName =
    (onWorkspaceChange && boundId ? workspace?.name?.trim() : undefined) || selectedProject?.name || null
  const selectedPath = (onWorkspaceChange && boundId ? workspace?.path : undefined) || selectedProject?.path || null

  useEffect(() => {
    if (!enabled || locked || !onWorkspaceChange || !pendingId) return
    writePendingProject(null)
    setPendingId(null)
    if (pendingId !== boundId) void onWorkspaceChange(pendingId)
  }, [boundId, enabled, locked, onWorkspaceChange, pendingId])

  const changeProject = useCallback(
    async (projectId: string | null) => {
      if (locked) return
      if (onWorkspaceChange) {
        await onWorkspaceChange(projectId)
        return
      }
      writePendingProject(projectId)
      setPendingId(projectId)
    },
    [locked, onWorkspaceChange]
  )

  useEffect(() => {
    if (!enabled || locked) return
    const onSelect = (event: Event) => {
      const projectId = (event as CustomEvent<string>).detail
      if (!projectId) return
      if (onWorkspaceChange) writePendingProject(null)
      void changeProject(projectId)
    }
    globalThis.addEventListener(PROJECT_SELECT_EVENT, onSelect)
    return () => globalThis.removeEventListener(PROJECT_SELECT_EVENT, onSelect)
  }, [changeProject, enabled, locked, onWorkspaceChange])

  return {
    selectedId,
    selectedName,
    selectedPath,
    bound: Boolean(onWorkspaceChange),
    changeProject,
    pickerOpen,
    setPickerOpen
  }
}

export interface WorkbenchProjectTrayProps {
  selection: WorkbenchProjectSelection
  workspaceChanging?: boolean
  goal: string | null
  onClearGoal: () => void
  /** Task conversations do not show this tray. */
  locked?: boolean
  goalPaused?: boolean
  /** Stored goals are cleared with `/goal clear`, not by editing the draft. */
  goalClearable?: boolean
  /** Workspace file picker beside the project menu. Task conversations keep the picker without the menu. */
  files?: WorkbenchWorkspaceFilesControl
}

function ComposerGoalChip({
  goal,
  paused = false,
  clearable = true,
  onClear
}: {
  goal: string
  paused?: boolean
  clearable?: boolean
  onClear: () => void
}) {
  const { t } = useTranslation()
  return (
    <span
      data-composer-goal="true"
      data-goal-paused={paused ? 'true' : 'false'}
      title={
        clearable
          ? undefined
          : t('workbench.composer.goal_durable', '持续目标。发送 /goal clear 后清除，/goal pause 后暂停。')
      }
      className="ml-auto inline-flex h-7 min-w-0 items-center gap-1 rounded-[8px] bg-white px-2 text-[13px] text-[#60646c]">
      <span className="shrink-0">{t('workbench.composer.goal', '目标')}</span>
      <span className="max-w-56 truncate text-[#171717]">{goal || t('workbench.composer.goal_empty', '未填写')}</span>
      {paused ? <span className="shrink-0 text-[#8a8a8a]">{t('workbench.composer.goal_paused', '已暂停')}</span> : null}
      {clearable ? (
        <button
          type="button"
          aria-label={t('workbench.composer.goal_clear', '移除目标')}
          className="grid size-4 place-items-center rounded-[4px] text-[#8a8a8a] hover:text-[#171717]"
          onClick={onClear}>
          <X size={12} strokeWidth={1.7} />
        </button>
      ) : null}
    </span>
  )
}

export function WorkbenchProjectTray({
  selection,
  workspaceChanging = false,
  goal,
  onClearGoal,
  locked = false,
  goalPaused = false,
  goalClearable = true,
  files
}: WorkbenchProjectTrayProps) {
  const { t } = useTranslation()
  const { selectedId, selectedName, selectedPath, bound, changeProject, pickerOpen, setPickerOpen } = selection
  const filePicker = files ? <WorkbenchWorkspaceFilePicker {...files} /> : null
  const goalChip =
    goal === null ? null : (
      <ComposerGoalChip goal={goal} paused={goalPaused} clearable={goalClearable} onClear={onClearGoal} />
    )
  if (locked) {
    if (!filePicker && goalChip === null) return null
  }
  return (
    <div
      data-workbench-project-tray="true"
      role="toolbar"
      aria-label={t('workbench.composer.scope', '任务范围')}
      className="flex min-h-[46px] w-full min-w-0 items-center gap-2 rounded-b-[14px] border border-t-0 border-[#ececec] bg-[#f5f5f7] px-2 pt-[18px] pb-1.5">
      {locked ? null : (
        <WorkspaceSelector
          value={selectedId}
          onChange={changeProject}
          open={pickerOpen}
          onOpenChange={setPickerOpen}
          shouldClearSelectionOnDelete={!bound}
          side="bottom"
          align="start"
          disabled={workspaceChanging}
          trigger={
            <button
              type="button"
              data-workbench-project-trigger="true"
              aria-label={
                selectedName
                  ? `${t('workbench.composer.project.label', '项目文件夹')} ${selectedName}`
                  : t('workbench.composer.project.label', '项目文件夹')
              }
              title={selectedPath || undefined}
              disabled={workspaceChanging}
              className={`inline-flex h-7 max-w-[60%] min-w-0 items-center gap-1.5 rounded-[8px] px-2 text-[13px] transition-colors hover:bg-black/[0.05] hover:text-[#171717] disabled:opacity-50 data-[state=open]:bg-black/[0.05] ${
                selectedName ? 'text-[#171717]' : 'text-[#60646c]'
              }`}>
              <Folder size={14} strokeWidth={1.7} className="shrink-0 text-[#60646c]" />
              <span className="truncate">
                {selectedName ?? t('workbench.composer.project.choose', '选择项目文件夹')}
              </span>
              <ChevronDown size={12} strokeWidth={1.7} className="shrink-0 text-[#8a8a8a]" />
            </button>
          }
        />
      )}
      {filePicker}
      {goalChip}
    </div>
  )
}
