import type { ColumnDef } from '@a3s-lab/ui'
import {
  Alert,
  Badge,
  Button,
  Center,
  Combobox,
  ConfirmDialog,
  DataTable,
  DateTimePicker,
  Dialog,
  DialogContent,
  DialogDescription,
  DialogFooter,
  DialogHeader,
  DialogTitle,
  DropdownMenu,
  DropdownMenuContent,
  DropdownMenuGroup,
  DropdownMenuItem,
  DropdownMenuTrigger,
  EmptyState,
  Field,
  FieldError,
  FieldGroup,
  FieldLabel,
  Input,
  InputGroup,
  InputGroupAddon,
  InputGroupInput,
  InputGroupText,
  Item,
  ItemActions,
  ItemContent,
  ItemDescription,
  ItemGroup,
  ItemSeparator,
  ItemTitle,
  Pagination,
  PaginationContent,
  PaginationItem,
  PaginationNext,
  PaginationPrevious,
  RowFlex,
  Scrollbar,
  SearchInput,
  SegmentedControl,
  Select,
  SelectContent,
  SelectGroup,
  SelectItem,
  SelectTrigger,
  SelectValue,
  Spinner,
  Switch,
  Tabs,
  TabsContent,
  TabsList,
  TabsTrigger,
  Tooltip
} from '@a3s-lab/ui'
import { Link, useNavigate, useParams } from '@tanstack/react-router'
import { Cron } from 'croner'
import type { TFunction } from 'i18next'
import {
  ArrowLeft,
  ArrowRight,
  Bot,
  CalendarCheck2,
  CalendarClock,
  CalendarFold,
  ChevronDown,
  ChevronRight,
  CircleCheck,
  CircleSlash,
  CircleStop,
  CircleX,
  Clock3,
  Cpu,
  FileText,
  Folder,
  LayoutGrid,
  Loader2,
  MoreHorizontal,
  PencilLine,
  Play,
  Plus,
  Sparkles,
  Trash2,
  X
} from 'lucide-react'
import { type FC, Fragment, type ReactNode, useCallback, useEffect, useId, useMemo, useRef, useState } from 'react'
import { useTranslation } from 'react-i18next'

import { loggerService } from '@logger'
import CollapsibleSearchBar from '@renderer/components/CollapsibleSearchBar'
import { PageEntry } from '@renderer/components/layout/PageEntry'
import { ModelSelector } from '@renderer/components/ModelSelector'
import PromptEditorField from '@renderer/components/PromptEditorField'
import { PromptPolishActions } from '@renderer/components/resourceCatalog/dialogs/components/PromptPolishActions'
import { AgentSelector, WorkspaceSelector } from '@renderer/components/resourceCatalog/selectors'
import {
  SettingDescription,
  SettingDivider,
  SettingGroup,
  SettingsContentColumn,
  SettingTitle
} from '@renderer/components/SettingsPrimitives'
import { useDataChange, useQuery } from '@renderer/data/hooks/useDataApi'
import { useChannels } from '@renderer/hooks/agent/useChannels'
import {
  useAllTasks,
  useCreateTask,
  useDeleteTask,
  useRunTask,
  useSetTaskEnabled,
  useTask,
  useTaskLogs,
  useUpdateTask
} from '@renderer/hooks/agent/useTasks'
import { useConversationNavigation } from '@renderer/hooks/useConversationNavigation'
import { useModelById } from '@renderer/hooks/useModel'
import { useTheme } from '@renderer/hooks/useTheme'
import { useHomeLocalSkills } from '@renderer/pages/agents/homeSlashCatalog'
import { defaultAppWorkspacePath, fileMentionRoot } from '@renderer/pages/settings/GeneralSettings/appWorkspace'
import { TaskPromptComposer } from '@renderer/pages/settings/TaskPromptComposer'
import { openRoute } from '@renderer/services/mainWindowNavigation'
import { toast } from '@renderer/services/toast'
import { cn } from '@renderer/utils/style'
import type { AgentChannelEntity } from '@shared/data/api/schemas/agentChannels'
import { AGENTS_MAX_LIMIT } from '@shared/data/api/schemas/agents'
import { AGENT_WORKSPACE_TYPE } from '@shared/data/api/schemas/agentWorkspaces'
import type { Trigger } from '@shared/data/api/schemas/jobs'
import type { ScheduledTaskEntity, ScheduledTaskListItem, TaskRunLogEntity } from '@shared/data/types/agent'
import { isUniqueModelId, type Model, type UniqueModelId } from '@shared/data/types/model'
import type { AgentTaskForm, AgentTaskPatch } from '@shared/ipc/schemas/ai'
import { readStudioHomePath } from '@shared/studioRuntime'
import { AbsoluteFilePathSchema, type DirectoryEntry } from '@shared/types/file'
import { isNonChatModel } from '@shared/utils/model'

const logger = loggerService.withContext('TasksSettings')
const ALL_TASKS_FILTER = 'all'
const WORKBENCH_PAGE_SIZE = 24
const WORKBENCH_TASK_LIMIT = 500
const STATUS_FILTERS = [
  { id: ALL_TASKS_FILTER, labelKey: 'settings.scheduledTasks.allStatuses', icon: LayoutGrid },
  { id: 'active', labelKey: 'agent.tasks.status.active', icon: CalendarClock },
  { id: 'paused', labelKey: 'agent.tasks.status.paused', icon: CircleSlash },
  { id: 'completed', labelKey: 'agent.tasks.status.completed', icon: CalendarCheck2 },
  { id: 'missed', labelKey: 'agent.tasks.status.missed', icon: Clock3 }
] as const
const SCHEDULE_HOURS = Array.from({ length: 24 }, (_, hour) => String(hour).padStart(2, '0'))
const SCHEDULE_MINUTES = Array.from({ length: 60 }, (_, minute) => String(minute).padStart(2, '0'))

const TASK_PROMPT_GENERATION_SYSTEM_PROMPT = [
  'Write a concise execution prompt for a scheduled Agent task based on the supplied task name.',
  'Describe the concrete work the Agent should perform each time the schedule runs.',
  'Include the expected result or delivery outcome when it can be inferred.',
  'Do not create a persona, role profile, background, greeting, or initialization script.',
  'Keep the output in the same language as the task name.',
  'Return only the task prompt with no explanation, wrapper, or code fence.'
].join('\n')

const TASK_PROMPT_POLISH_SYSTEM_PROMPT = [
  'Improve the supplied scheduled task prompt without changing its intent.',
  'Make the recurring action, required inputs, constraints, and expected result concrete and concise.',
  'Do not turn the task into a persona, role profile, background, greeting, or initialization script.',
  'Keep the output in the same language as the input.',
  'Preserve Markdown, code, URLs, and every placeholder token verbatim, including tokens shaped like {{name}} and ${name}; keep duplicate occurrences.',
  'Return only the polished task prompt with no explanation, wrapper, or code fence.'
].join('\n')

type AgentInfo = { id: string; name: string; model: string | null }
type ChannelInfo = { id: string; agentId?: string | null; name: string; isActive?: boolean; hasActiveChatIds?: boolean }

function toChannelInfo(channel: AgentChannelEntity): ChannelInfo {
  // Config keys are snake_case in the channel config schemas; only some channel
  // types carry allowed_channel_ids, hence the narrow local view of the union.
  const config = channel.config as { allowed_chat_ids?: string[]; allowed_channel_ids?: string[] } | undefined
  return {
    id: channel.id,
    agentId: channel.agentId ?? null,
    name: channel.name || channel.type,
    isActive: channel.isActive,
    hasActiveChatIds:
      (config?.allowed_chat_ids?.length ?? 0) > 0 ||
      (config?.allowed_channel_ids?.length ?? 0) > 0 ||
      (channel.activeChatIds?.length ?? 0) > 0
  }
}

export type ScheduleKind = 'hourly' | 'daily' | 'weekdays' | 'weekly' | 'customDays' | 'interval' | 'once' | 'cron'
export type ScheduleFormState = {
  kind: ScheduleKind
  value: string
  weekday: string
  timeoutMinutes: string
}

type TaskFileRef = { path: string; name: string }

type TaskDraftSnapshot = {
  name: string
  prompt: string
  schedule: ScheduleFormState
  channelIds: string[]
  workspaceId: string | null
  reuseSession: boolean
  model: string | null
  skills: string[]
  files: TaskFileRef[]
}
type TaskUpdateResult = {
  succeeded: boolean
  task: ScheduledTaskEntity
}

const DEFAULT_SCHEDULE: ScheduleFormState = {
  kind: 'daily',
  value: '09:00',
  weekday: '1',
  timeoutMinutes: ''
}

const WEEKDAY_ORDER = ['1', '2', '3', '4', '5', '6', '0'] as const
const ALL_WEEKDAYS = WEEKDAY_ORDER.join(',')

function parseWeekdayList(value: string): string[] {
  const selected = new Set(
    value
      .split(',')
      .map((part) => part.trim())
      .filter((part) => /^[0-6]$/.test(part))
  )
  return WEEKDAY_ORDER.filter((day) => selected.has(day))
}

function expandDayOfWeek(field: string): string[] | null {
  if (field === '*') return [...WEEKDAY_ORDER]
  const selected = new Set<string>()
  for (const part of field.split(',')) {
    const range = /^([0-6])-([0-6])$/.exec(part)
    if (range) {
      const start = Number(range[1])
      const end = Number(range[2])
      if (start > end) return null
      for (let day = start; day <= end; day += 1) selected.add(String(day))
      continue
    }
    if (/^[0-6]$/.test(part)) {
      selected.add(part)
      continue
    }
    return null
  }
  const days = WEEKDAY_ORDER.filter((day) => selected.has(day))
  return days.length > 0 ? [...days] : null
}

const parseScheduleDate = (value: string) => {
  if (!value) return undefined
  const date = new Date(value)
  return Number.isNaN(date.getTime()) ? undefined : date
}

// Canonical value for the time presets: a sorted, deduped 'HH:MM,HH:MM…' list
// sharing one minute — per-entry minutes would cross-product into extra cron firings.
const parseTimes = (value: string): { hours: string[]; minute: string } | null => {
  const matches = value.split(',').map((part) => /^(\d{1,2}):(\d{2})$/.exec(part.trim()))
  if (matches.some((match) => match === null)) return null

  const minute = matches[0]![2]
  if (Number(minute) > 59 || matches.some((match) => match![2] !== minute)) return null

  const hours = matches.map((match) => Number(match![1]))
  if (hours.some((hour) => hour > 23)) return null

  const uniqueSorted = [...new Set(hours)].sort((a, b) => a - b)
  return { hours: uniqueSorted.map((hour) => String(hour).padStart(2, '0')), minute }
}

const formatTimes = (hours: string[], minute: string) =>
  [...new Set(hours)]
    .sort()
    .map((hour) => `${hour}:${minute}`)
    .join(',')

function isValidCronExpression(value: string): boolean {
  const expression = value.trim()

  let cron: Cron | undefined
  try {
    cron = new Cron(expression, { paused: true })
    return cron.nextRun() !== null
  } catch {
    return false
  } finally {
    cron?.stop()
  }
}

export function triggerToFormState(trigger: Trigger): Omit<ScheduleFormState, 'timeoutMinutes'> {
  if (trigger.kind === 'interval') {
    return {
      kind: 'interval',
      value: String(Math.max(1, Math.round(trigger.ms / 60_000))),
      weekday: '1'
    }
  }

  if (trigger.kind === 'once') {
    return {
      kind: 'once',
      value: new Date(trigger.at).toISOString(),
      weekday: '1'
    }
  }

  const parts = trigger.expr.trim().split(/\s+/)
  if (parts.length !== 5) {
    return { kind: 'cron', value: trigger.expr, weekday: '1' }
  }

  const [minutePart, hourPart, dayOfMonth, month, dayOfWeek] = parts
  if (minutePart === '0' && hourPart === '*' && dayOfMonth === '*' && month === '*' && dayOfWeek === '*') {
    return { kind: 'hourly', value: '', weekday: '1' }
  }

  const hourStep = hourPart === '*' ? null : /^\*\/([1-9]\d*)$/.exec(hourPart)?.[1]
  if (minutePart === '0' && hourStep && dayOfMonth === '*' && month === '*') {
    const days = expandDayOfWeek(dayOfWeek)
    const hours = Number(hourStep)
    if (days && hours >= 1 && hours <= 23) {
      return { kind: 'customDays', value: String(hours), weekday: days.join(',') }
    }
  }

  const minute = Number(minutePart)
  const hourParts = hourPart.split(',')
  const hasValidTime =
    Number.isInteger(minute) &&
    minute >= 0 &&
    minute <= 59 &&
    hourParts.every((part) => /^\d{1,2}$/.test(part) && Number(part) <= 23)

  if (!hasValidTime || dayOfMonth !== '*' || month !== '*') {
    return { kind: 'cron', value: trigger.expr, weekday: '1' }
  }

  // Canonical form: sorted, deduped (matches parseTimes/formatTimes so the UI
  // multi-select never disagrees with the value it renders).
  const value = formatTimes(
    hourParts.map((part) => part.padStart(2, '0')),
    String(minute).padStart(2, '0')
  )
  if (dayOfWeek === '*') return { kind: 'daily', value, weekday: '1' }
  if (dayOfWeek === '1-5') return { kind: 'weekdays', value, weekday: '1' }
  if (/^[0-6]$/.test(dayOfWeek)) return { kind: 'weekly', value, weekday: dayOfWeek }
  return { kind: 'cron', value: trigger.expr, weekday: '1' }
}

export function formStateToTrigger(schedule: ScheduleFormState): Trigger | null {
  if (schedule.kind === 'hourly') return { kind: 'cron', expr: '0 * * * *' }

  if (schedule.kind === 'interval') {
    const minutes = Number(schedule.value.trim())
    if (!Number.isInteger(minutes) || minutes <= 0) return null
    return { kind: 'interval', ms: minutes * 60_000 }
  }

  if (schedule.kind === 'once') {
    const at = Date.parse(schedule.value.trim())
    return Number.isFinite(at) ? { kind: 'once', at } : null
  }

  if (schedule.kind === 'cron') {
    const expr = schedule.value.trim()
    return isValidCronExpression(expr) ? { kind: 'cron', expr } : null
  }

  if (schedule.kind === 'customDays') {
    const hours = Number(schedule.value.trim())
    const days = parseWeekdayList(schedule.weekday)
    if (!Number.isInteger(hours) || hours <= 0 || hours > 23 || days.length === 0) return null
    const dow = days.length === WEEKDAY_ORDER.length ? '*' : days.join(',')
    return { kind: 'cron', expr: `0 */${hours} * * ${dow}` }
  }

  const times = parseTimes(schedule.value)
  if (!times || times.hours.length === 0) return null
  const prefix = `${Number(times.minute)} ${times.hours.map(Number).join(',')} * *`

  if (schedule.kind === 'daily') return { kind: 'cron', expr: `${prefix} *` }
  if (schedule.kind === 'weekdays') return { kind: 'cron', expr: `${prefix} 1-5` }
  if (!/^[0-6]$/.test(schedule.weekday)) return null
  return { kind: 'cron', expr: `${prefix} ${schedule.weekday}` }
}

function scheduleForKind(kind: ScheduleKind, current: ScheduleFormState): ScheduleFormState {
  switch (kind) {
    case 'hourly':
      return { ...current, kind, value: '' }
    case 'daily':
    case 'weekdays':
    case 'weekly':
      return { ...current, kind, value: parseTimes(current.value) ? current.value : '09:00' }
    case 'customDays':
      return { ...current, kind, value: '1', weekday: ALL_WEEKDAYS }
    case 'interval':
    case 'once':
    case 'cron':
      return { ...current, kind, value: '' }
  }
}

function taskToDraftSnapshot(task: ScheduledTaskEntity): TaskDraftSnapshot {
  return {
    name: task.name,
    prompt: task.prompt,
    schedule: {
      ...triggerToFormState(task.trigger),
      timeoutMinutes: task.timeoutMinutes > 0 ? task.timeoutMinutes.toString() : ''
    },
    channelIds: task.channelIds ?? [],
    workspaceId: task.workspace.type === AGENT_WORKSPACE_TYPE.USER ? task.workspace.workspaceId : null,
    reuseSession: task.reuseSession,
    model: task.model ?? null,
    skills: task.skills ?? [],
    files: (task.files ?? []).map((file) => ({ path: file.path, name: file.name }))
  }
}

function scheduleInputsEqual(a: ScheduleFormState, b: ScheduleFormState): boolean {
  return a.kind === b.kind && a.value === b.value && a.weekday === b.weekday
}

const TASK_BUILTIN_SKILLS = [
  { name: 'skill-creator', labelKey: 'capabilities.skill.menu.create' }
] as const
const TASK_SKILL_LIMIT = 8
const TASK_FILE_LIMIT = 20

function taskFileName(path: string) {
  const normalized = path.replace(/\\/g, '/')
  return normalized.split('/').pop() || normalized
}

function taskFileInsideRoot(path: string, root: string) {
  if (!root) return false
  const normalizedRoot = root.replace(/\\/g, '/').replace(/\/+$/, '')
  const normalized = path.replace(/\\/g, '/')
  return normalized === normalizedRoot || normalized.startsWith(`${normalizedRoot}/`)
}

function taskFileLabel(root: string, path: string) {
  const normalizedRoot = root.replace(/\\/g, '/').replace(/\/+$/, '')
  const normalized = path.replace(/\\/g, '/')
  if (normalizedRoot && normalized.startsWith(`${normalizedRoot}/`)) {
    return normalized.slice(normalizedRoot.length + 1)
  }
  return taskFileName(path)
}

function taskFilesEqual(left: readonly TaskFileRef[], right: readonly TaskFileRef[]) {
  return (
    left.length === right.length &&
    left.every((file, index) => file.path === right[index]?.path && file.name === right[index]?.name)
  )
}

async function listTaskFiles(root: string, query: string): Promise<TaskFileRef[]> {
  const parsed = AbsoluteFilePathSchema.safeParse(root)
  const listDirectoryEntries = window.api?.file?.listDirectoryEntries
  if (!parsed.success || !listDirectoryEntries) return []
  const trimmed = query.trim()
  const entries: DirectoryEntry[] = await listDirectoryEntries(parsed.data, {
    recursive: trimmed.length > 0,
    maxDepth: trimmed.length > 0 ? 10 : 1,
    includeHidden: false,
    includeFiles: true,
    includeDirectories: false,
    maxEntries: 40,
    searchPattern: trimmed || '.'
  })
  return entries.flatMap((entry) => (entry.isDirectory ? [] : [{ path: entry.path, name: taskFileName(entry.path) }]))
}

function scheduledTaskModelLabel(modelId: string | null | undefined) {
  if (!modelId) return null
  const separator = modelId.indexOf('::')
  return separator > 0 ? modelId.slice(separator + 2) : modelId
}

const TaskModelField: FC<{
  value: string | null
  disabled?: boolean
  onChange: (modelId: string | null) => void
}> = ({ value, disabled, onChange }) => {
  const { t } = useTranslation()
  const modelId = value && isUniqueModelId(value) ? value : undefined
  const { model } = useModelById(modelId)
  const label = model?.name ?? scheduledTaskModelLabel(modelId) ?? t('library.config.basic.model_pick')
  return (
    <ModelSelector
      multiple={false}
      selectionType="id"
      value={modelId}
      align="start"
      filter={(item: Model) => !isNonChatModel(item)}
      onSelect={(next: UniqueModelId | undefined) => onChange(next ?? null)}
      trigger={
        <Button
          type="button"
          variant="ghost"
          size="sm"
          disabled={disabled}
          aria-label={t('selection.settings.user_modal.model.label')}>
          <Cpu size={14} />
          <span>{label}</span>
          <ChevronDown size={14} />
        </Button>
      }
    />
  )
}

const TaskSkillField: FC<{
  value: string[]
  disabled?: boolean
  onChange: (skills: string[]) => void
}> = ({ value, disabled, onChange }) => {
  const { t } = useTranslation()
  const localSkills = useHomeLocalSkills(true)
  const options = useMemo(() => {
    const seen = new Set<string>()
    const items: { value: string; label: string; description?: string }[] = []
    for (const skill of TASK_BUILTIN_SKILLS) {
      seen.add(skill.name)
      items.push({ value: skill.name, label: t(skill.labelKey) })
    }
    for (const skill of localSkills) {
      if (seen.has(skill.name)) continue
      seen.add(skill.name)
      items.push({ value: skill.name, label: skill.name, description: skill.description || undefined })
    }
    return items
  }, [localSkills, t])
  const selected = value.length > 0 ? value.join(', ') : t('plugins.skills')
  return (
    <Combobox
      multiple
      size="sm"
      searchable
      disabled={disabled}
      className="max-w-[220px]"
      aria-label={t('plugins.skills')}
      placeholder={t('plugins.skills')}
      searchPlaceholder={t('library.skill_marketplace.search_placeholder')}
      emptyText={t('library.skill_marketplace.no_results_title')}
      options={options}
      value={value}
      onChange={(next) => {
        if (Array.isArray(next)) onChange(next.slice(0, TASK_SKILL_LIMIT))
      }}
      renderValue={() => (
        <span
          className={cn(
            'flex min-w-0 flex-1 items-center gap-1 truncate text-left',
            value.length === 0 && 'text-[#8a8a8a]'
          )}>
          <Sparkles size={14} />
          <span className="truncate">{selected}</span>
        </span>
      )}
    />
  )
}

const TaskFileField: FC<{
  root: string
  value: TaskFileRef[]
  disabled?: boolean
  onChange: (files: TaskFileRef[]) => void
}> = ({ root, value, disabled, onChange }) => {
  const { t } = useTranslation()
  const [query, setQuery] = useState('')
  const [found, setFound] = useState<TaskFileRef[]>([])
  useEffect(() => {
    let active = true
    const timer = window.setTimeout(
      () => {
        void listTaskFiles(root, query)
          .then((files) => {
            if (active) setFound(files)
          })
          .catch(() => {
            if (active) setFound([])
          })
      },
      query ? 200 : 0
    )
    return () => {
      active = false
      window.clearTimeout(timer)
    }
  }, [query, root])
  const options = useMemo(() => {
    const byPath = new Map<string, TaskFileRef>()
    for (const file of value) byPath.set(file.path, file)
    for (const file of found) byPath.set(file.path, file)
    return [...byPath.values()].map((file) => ({
      value: file.path,
      label: taskFileLabel(root, file.path),
      description: file.path
    }))
  }, [found, root, value])
  const placeholder = root ? t('workbench.composer.mention.title') : t('workbench.composer.mention.choose_project')
  const selected = value.length > 0 ? value.map((file) => file.name).join(', ') : placeholder
  return (
    <Combobox
      multiple
      size="sm"
      searchable
      searchDebounceMs={200}
      disabled={disabled || !root}
      className="max-w-[220px]"
      aria-label={t('workbench.composer.mention.title')}
      placeholder={placeholder}
      searchPlaceholder={t('workbench.composer.mention.title')}
      emptyText={t('library.skill_detail.no_files')}
      options={options}
      value={value.map((file) => file.path)}
      onSearch={setQuery}
      onChange={(next) => {
        if (!Array.isArray(next)) return
        const names = new Map<string, string>()
        for (const file of value) names.set(file.path, file.name)
        for (const file of found) names.set(file.path, file.name)
        onChange(
          next.slice(0, TASK_FILE_LIMIT).map((path) => ({
            path,
            name: names.get(path) ?? taskFileName(path)
          }))
        )
      }}
      renderValue={() => (
        <span
          className={cn(
            'flex min-w-0 flex-1 items-center gap-1 truncate text-left',
            value.length === 0 && 'text-[#8a8a8a]'
          )}>
          <FileText size={14} />
          <span className="truncate">{selected}</span>
        </span>
      )}
    />
  )
}

function stringArraysEqual(a: string[], b: string[]): boolean {
  return a.length === b.length && a.every((value, index) => value === b[index])
}

function triggersEqual(a: Trigger, b: Trigger): boolean {
  if (a.kind === 'cron' && b.kind === 'cron') {
    return a.expr === b.expr && a.timezone === b.timezone && a.limit === b.limit
  }
  if (a.kind === 'interval' && b.kind === 'interval') return a.ms === b.ms && a.anchor === b.anchor
  if (a.kind === 'once' && b.kind === 'once') return a.at === b.at
  return false
}

function preserveCompatibleTriggerMetadata(previous: Trigger, next: Trigger): Trigger {
  if (previous.kind === 'cron' && next.kind === 'cron') {
    return { ...previous, expr: next.expr }
  }
  if (previous.kind === 'interval' && next.kind === 'interval') {
    return { ...previous, ms: next.ms }
  }
  return next
}

function getWeekdayLabel(weekday: string, t: TFunction) {
  const labels: Record<string, string> = {
    '0': t('agent.tasks.schedule.weekdays.sunday'),
    '1': t('agent.tasks.schedule.weekdays.monday'),
    '2': t('agent.tasks.schedule.weekdays.tuesday'),
    '3': t('agent.tasks.schedule.weekdays.wednesday'),
    '4': t('agent.tasks.schedule.weekdays.thursday'),
    '5': t('agent.tasks.schedule.weekdays.friday'),
    '6': t('agent.tasks.schedule.weekdays.saturday')
  }
  return labels[weekday] ?? weekday
}

function getTaskStatusLabel(status: string, t: TFunction) {
  const labels: Record<string, string> = {
    active: t('agent.tasks.status.active'),
    paused: t('agent.tasks.status.paused'),
    completed: t('agent.tasks.status.completed'),
    missed: t('agent.tasks.status.missed')
  }
  return labels[status] ?? status
}

function getTaskScheduleStatusIconPresentation(status: ScheduledTaskEntity['status']) {
  switch (status) {
    case 'active':
      return {
        Icon: CalendarClock,
        wrapperClassName: 'bg-info-subtle text-info-subtle-foreground',
        iconClassName: 'text-info-subtle-foreground'
      }
    case 'missed':
    case 'paused':
      return {
        Icon: CalendarFold,
        wrapperClassName: 'bg-warning-subtle text-warning-subtle-foreground',
        iconClassName: 'text-warning-subtle-foreground'
      }
    case 'completed':
      return {
        Icon: CalendarCheck2,
        wrapperClassName: 'bg-success-subtle text-success-subtle-foreground',
        iconClassName: 'text-success-subtle-foreground'
      }
  }
}

const TaskScheduleStatusIcon: FC<{ status: ScheduledTaskEntity['status'] }> = ({ status }) => {
  const { Icon, wrapperClassName, iconClassName } = getTaskScheduleStatusIconPresentation(status)

  return (
    <div className={cn('flex size-10 shrink-0 items-center justify-center rounded-lg', wrapperClassName)}>
      <Icon size={20} aria-hidden className={iconClassName} />
    </div>
  )
}

function formatTaskCardTime(value: string) {
  return new Date(value).toLocaleString(undefined, {
    month: 'numeric',
    day: 'numeric',
    hour: '2-digit',
    minute: '2-digit',
    hour12: false
  })
}

const TaskRunSummaryLine: FC<{ summary: NonNullable<ScheduledTaskListItem['runSummary']> }> = ({ summary }) => {
  const { t } = useTranslation()

  if (summary.status === 'queued') {
    return (
      <span className="flex items-center gap-1.5 text-muted-foreground">
        <Clock3 aria-hidden className="size-3" />
        {t('agent.tasks.runSummary.queued')}
      </span>
    )
  }

  if (summary.status === 'running') {
    return (
      <span className="flex items-center gap-1.5 text-foreground">
        <Loader2 aria-hidden className="size-3 text-foreground motion-safe:animate-spin" />
        {t('agent.tasks.runSummary.running')}
      </span>
    )
  }

  const time = formatTaskCardTime(summary.finishedAt)
  if (summary.status === 'completed') {
    return (
      <span className="flex items-center gap-1.5 text-foreground">
        <CircleCheck aria-hidden className="size-3 text-foreground" />
        {t('agent.tasks.runSummary.completed', { time })}
      </span>
    )
  }
  if (summary.status === 'failed') {
    return (
      <span className="flex items-center gap-1.5 text-foreground">
        <CircleX aria-hidden className="size-3 text-foreground" />
        {t('agent.tasks.runSummary.failed', { time })}
      </span>
    )
  }
  return (
    <span className="flex items-center gap-1.5 text-foreground">
      <CircleStop aria-hidden className="size-3 text-foreground" />
      {t('agent.tasks.runSummary.cancelled', { time })}
    </span>
  )
}

const TaskCardRunStatus: FC<{ task: ScheduledTaskListItem }> = ({ task }) => {
  const { t } = useTranslation()
  const nextRun = task.status === 'active' ? task.nextRun : null

  if (!task.runSummary && !nextRun) return null

  return (
    <div className="flex flex-col items-end gap-0.5 text-xs whitespace-nowrap">
      {task.runSummary && <TaskRunSummaryLine summary={task.runSummary} />}
      {nextRun && (
        <span className="text-muted-foreground">
          {t('agent.tasks.nextRun')} · {formatTaskCardTime(nextRun)}
        </span>
      )}
    </div>
  )
}

function getTriggerSummary(trigger: Trigger, t: TFunction) {
  const schedule = triggerToFormState(trigger)
  // Time presets may hold a comma-joined list; space it out for display.
  const time = schedule.value.split(',').join(', ')
  switch (schedule.kind) {
    case 'hourly':
      return t('agent.tasks.schedule.summary.hourly')
    case 'daily':
      return t('agent.tasks.schedule.summary.daily', { time })
    case 'weekdays':
      return t('agent.tasks.schedule.summary.weekdays', { time })
    case 'weekly':
      return t('agent.tasks.schedule.summary.weekly', {
        weekday: getWeekdayLabel(schedule.weekday, t),
        time
      })
    case 'customDays':
      return t('agent.tasks.schedule.summary.customDays', {
        days: parseWeekdayList(schedule.weekday)
          .map((day) => getWeekdayLabel(day, t))
          .join(' '),
        count: Number(schedule.value)
      })
    case 'interval':
      return t('agent.tasks.schedule.summary.interval', { count: Number(schedule.value) })
    case 'once':
      return new Date(schedule.value).toLocaleString()
    case 'cron':
      return schedule.value
  }
}

export const TaskTimeSelect: FC<{
  value: string
  disabled?: boolean
  fill?: boolean
  onChange: (value: string) => void
}> = ({ value, disabled, fill = false, onChange }) => {
  const { t } = useTranslation()
  // An empty preset means the user cleared every hour; render no selection so the
  // UI agrees with the empty form value (which blocks saving until repopulated).
  // A non-empty value that fails to parse is unexpected, so fall back to the
  // default selection rather than rendering a blank, uneditable control.
  const { hours, minute } =
    parseTimes(value) ?? (value === '' ? { hours: [], minute: '00' } : { hours: ['09'], minute: '00' })

  // The minute is encoded inside the comma-joined value, so clearing the last
  // hour would otherwise discard it (formatTimes([], '30') === ''). Track the
  // last committed minute in state so re-picking an hour after clearing every
  // selection keeps the minute the user did not edit (e.g. 18:30 -> clear 18 ->
  // pick 20 yields 20:30, not 20:00). While no hour is selected the minute
  // stays a local preview only: the form value remains '' and saving is blocked.
  // Seed the retained minute from the mounted value: lastValue starts equal to
  // value, so the update block below never runs on the first render and a
  // fresh/remounted selector loading a non-zero-minute schedule would otherwise
  // fall back to 00 once every hour is cleared.
  const [rememberedMinute, setRememberedMinute] = useState(() => {
    const parsed = parseTimes(value)
    return parsed && parsed.hours.length > 0 ? parsed.minute : '00'
  })
  const [lastValue, setLastValue] = useState(value)
  if (value !== lastValue) {
    setLastValue(value)
    const parsed = parseTimes(value)
    if (parsed && parsed.hours.length > 0) setRememberedMinute(parsed.minute)
  }
  const displayMinute = hours.length > 0 ? minute : rememberedMinute

  return (
    <RowFlex
      role="group"
      aria-label={t('agent.tasks.schedule.time')}
      className={fill ? 'min-w-0 flex-1 items-center gap-2' : 'items-center gap-2'}>
      <div className={fill ? 'min-w-0 flex-1' : undefined}>
        <Combobox
          multiple
          searchable={false}
          width={fill ? '100%' : 160}
          aria-label={t('agent.tasks.schedule.hours')}
          placeholder={t('agent.tasks.schedule.hours')}
          disabled={disabled}
          options={SCHEDULE_HOURS.map((hour) => ({ value: hour, label: hour }))}
          value={hours}
          renderValue={(selectedHours) => {
            const values = Array.isArray(selectedHours) ? selectedHours : []
            return (
              <span className={cn('min-w-0 flex-1 truncate text-left', values.length === 0 && 'text-muted-foreground')}>
                {values.length > 0 ? values.join(', ') : t('agent.tasks.schedule.hours')}
              </span>
            )
          }}
          onChange={(next) => {
            if (Array.isArray(next)) onChange(formatTimes(next, displayMinute))
          }}
        />
      </div>
      <InputGroupText aria-hidden="true">:</InputGroupText>
      <Select
        value={displayMinute}
        disabled={disabled}
        onValueChange={(nextMinute) => {
          if (hours.length === 0) {
            // No hour selected: the minute cannot live in the value string, so
            // keep it only as the preview for the next hour selection.
            setRememberedMinute(nextMinute)
            return
          }
          onChange(formatTimes(hours, nextMinute))
        }}>
        <SelectTrigger aria-label={t('agent.tasks.schedule.minute')} className={fill ? 'w-16 shrink-0' : undefined}>
          <SelectValue />
        </SelectTrigger>
        <SelectContent>
          <SelectGroup>
            {SCHEDULE_MINUTES.map((option) => (
              <SelectItem key={option} value={option}>
                {option}
              </SelectItem>
            ))}
          </SelectGroup>
        </SelectContent>
      </Select>
    </RowFlex>
  )
}

function updatePositiveIntegerInput(value: string, onChange: (value: string) => void) {
  if (/^\d*$/.test(value)) onChange(value)
}

const CustomWeekdaySchedule: FC<{
  value: ScheduleFormState
  disabled?: boolean
  onChange: (value: ScheduleFormState) => void
}> = ({ value, disabled, onChange }) => {
  const { t } = useTranslation()
  const selected = parseWeekdayList(value.weekday)

  const toggleDay = (day: string) => {
    const next = selected.includes(day) ? selected.filter((item) => item !== day) : [...selected, day]
    onChange({ ...value, weekday: WEEKDAY_ORDER.filter((item) => next.includes(item)).join(',') })
  }

  return (
    <div className="flex min-w-0 flex-wrap items-center gap-2">
      <div className="flex min-w-0 flex-wrap items-center gap-1 rounded-[8px] border border-[#e5e5e5] bg-white p-1">
        {WEEKDAY_ORDER.map((day) => {
          const pressed = selected.includes(day)
          return (
            <button
              key={day}
              type="button"
              aria-pressed={pressed}
              disabled={disabled}
              onClick={() => toggleDay(day)}
              className={cn(
                'inline-flex h-7 items-center gap-1 rounded-[6px] px-2 text-[13px] outline-none focus-visible:ring-2 focus-visible:ring-black/10',
                pressed ? 'bg-[#f5f5f5] font-medium text-[#171717]' : 'text-[#8a8a8a] hover:bg-[#f7f7f7]'
              )}>
              {getWeekdayLabel(day, t)}
              {pressed ? <X className="size-3" strokeWidth={1.7} aria-hidden /> : null}
            </button>
          )
        })}
      </div>
      <span className="text-[13px] text-[#3f3f3f]">{t('agent.tasks.schedule.every')}</span>
      <input
        type="text"
        inputMode="numeric"
        aria-label={t('workbench.schedule.hours')}
        disabled={disabled}
        value={value.value}
        onChange={(event) => {
          if (/^\d*$/.test(event.target.value)) onChange({ ...value, value: event.target.value })
        }}
        className="h-9 w-12 rounded-[8px] border-0 bg-[#f5f5f5] px-2 text-center text-[13px] text-[#171717] outline-none focus-visible:ring-2 focus-visible:ring-black/10 focus-visible:ring-inset disabled:opacity-50"
      />
      <span className="text-[13px] text-[#3f3f3f]">{t('agent.tasks.schedule.perHour')}</span>
    </div>
  )
}

const TaskScheduleControls: FC<{
  value: ScheduleFormState
  disabled?: boolean
  invalid?: boolean
  tone?: 'settings' | 'workbench'
  onChange: (value: ScheduleFormState) => void
}> = ({ value, disabled, invalid, tone = 'settings', onChange }) => {
  const { t } = useTranslation()
  const id = useId()

  const workbench = tone === 'workbench'
  const updateKind = (kind: ScheduleKind) => {
    onChange(scheduleForKind(kind, value))
  }

  const updateValue = (nextValue: string) => onChange({ ...value, value: nextValue })

  let frequencyControl: ReactNode = null
  if (value.kind === 'customDays') {
    frequencyControl = <CustomWeekdaySchedule value={value} disabled={disabled} onChange={onChange} />
  } else if (value.kind === 'daily' || value.kind === 'weekdays') {
    frequencyControl = (
      <TaskTimeSelect value={value.value} disabled={disabled} fill={workbench} onChange={updateValue} />
    )
  } else if (value.kind === 'weekly') {
    frequencyControl = (
      <>
        <Select
          value={value.weekday}
          disabled={disabled}
          onValueChange={(weekday) => {
            onChange({ ...value, weekday })
          }}>
          <SelectTrigger
            aria-label={t('agent.tasks.schedule.weekday')}
            className={workbench ? 'h-9 w-28 shrink-0' : undefined}>
            <SelectValue />
          </SelectTrigger>
          <SelectContent>
            <SelectGroup>
              {['1', '2', '3', '4', '5', '6', '0'].map((weekday) => (
                <SelectItem key={weekday} value={weekday}>
                  {getWeekdayLabel(weekday, t)}
                </SelectItem>
              ))}
            </SelectGroup>
          </SelectContent>
        </Select>
        <TaskTimeSelect value={value.value} disabled={disabled} fill={workbench} onChange={updateValue} />
      </>
    )
  } else if (value.kind === 'interval') {
    frequencyControl = (
      <InputGroup className={workbench ? 'w-full max-w-[240px]' : 'w-40'} data-disabled={disabled || undefined}>
        <InputGroupInput
          type="text"
          inputMode="numeric"
          pattern="[0-9]*"
          value={value.value}
          placeholder={t('agent.tasks.intervalPlaceholder')}
          disabled={disabled}
          aria-label={t('agent.tasks.schedule.intervalMinutes')}
          aria-invalid={invalid || undefined}
          onChange={(event) => updatePositiveIntegerInput(event.target.value, updateValue)}
        />
        <InputGroupAddon align="inline-end">
          <InputGroupText>{t('agent.tasks.intervalUnit')}</InputGroupText>
        </InputGroupAddon>
      </InputGroup>
    )
  } else if (value.kind === 'once') {
    frequencyControl = (
      <DateTimePicker
        value={parseScheduleDate(value.value)}
        granularity="minute"
        format="yyyy-MM-dd HH:mm"
        placeholder={t('agent.tasks.oncePlaceholder')}
        disabled={disabled}
        labels={{
          hour: t('agent.tasks.schedule.hour'),
          minute: t('agent.tasks.schedule.minute')
        }}
        onChange={(date) => {
          if (date) updateValue(date.toISOString())
        }}
      />
    )
  } else if (value.kind === 'cron') {
    frequencyControl = (
      <Input
        className={workbench ? 'w-full font-mono' : 'w-72 max-w-full font-mono'}
        value={value.value}
        placeholder={t('agent.tasks.schedule.cronPlaceholder')}
        disabled={disabled}
        aria-label={t('agent.tasks.schedule.cron')}
        aria-invalid={invalid || undefined}
        onChange={(event) => updateValue(event.target.value)}
      />
    )
  }

  return (
    <FieldGroup>
      <Field data-invalid={invalid || undefined}>
        <FieldLabel htmlFor={`${id}-kind`}>{t('agent.tasks.frequency.label')}</FieldLabel>
        <RowFlex className={workbench ? 'w-full items-center gap-2' : 'flex-wrap items-center gap-3'}>
          <Select value={value.kind} disabled={disabled} onValueChange={(kind) => updateKind(kind as ScheduleKind)}>
            <SelectTrigger
              id={`${id}-kind`}
              aria-invalid={invalid || undefined}
              className={workbench ? 'h-9 w-[11rem] shrink-0' : undefined}>
              <SelectValue />
            </SelectTrigger>
            <SelectContent>
              <SelectGroup>
                <SelectItem value="hourly">{t('agent.tasks.schedule.hourly')}</SelectItem>
                <SelectItem value="daily">{t('agent.tasks.schedule.daily')}</SelectItem>
                <SelectItem value="weekdays">{t('agent.tasks.schedule.weekdaysOnly')}</SelectItem>
                {tone === 'workbench' ? (
                  <SelectItem value="customDays">{t('agent.tasks.schedule.weekly')}</SelectItem>
                ) : (
                  <SelectItem value="weekly">{t('agent.tasks.schedule.weekly')}</SelectItem>
                )}
                {tone === 'workbench' && value.kind === 'weekly' ? (
                  <SelectItem value="weekly">{t('agent.tasks.schedule.weekly')}</SelectItem>
                ) : null}
                {tone === 'settings' ? (
                  <SelectItem value="customDays">{t('agent.tasks.schedule.customDays')}</SelectItem>
                ) : null}
                <SelectItem value="interval">{t('agent.tasks.schedule.interval')}</SelectItem>
                <SelectItem value="once">{t('agent.tasks.schedule.once')}</SelectItem>
                <SelectItem value="cron">{t('agent.tasks.schedule.cron')}</SelectItem>
              </SelectGroup>
            </SelectContent>
          </Select>
          {frequencyControl ? (
            <div className={workbench ? 'flex min-w-0 flex-1 items-center gap-2' : 'contents'}>{frequencyControl}</div>
          ) : null}
        </RowFlex>
        <FieldError>
          {invalid
            ? t(value.kind === 'cron' ? 'agent.tasks.schedule.invalidCron' : 'agent.tasks.schedule.invalid')
            : undefined}
        </FieldError>
      </Field>

      <Field>
        <FieldLabel htmlFor={`${id}-timeout`}>{t('agent.tasks.timeout.label')}</FieldLabel>
        <InputGroup data-disabled={disabled || undefined}>
          <InputGroupInput
            id={`${id}-timeout`}
            type="text"
            inputMode="numeric"
            pattern="[0-9]*"
            value={value.timeoutMinutes}
            placeholder={t('agent.tasks.timeout.placeholder')}
            disabled={disabled}
            onChange={(event) =>
              updatePositiveIntegerInput(event.target.value, (timeoutMinutes) => onChange({ ...value, timeoutMinutes }))
            }
          />
          <InputGroupAddon align="inline-end">
            <InputGroupText>{t('agent.tasks.intervalUnit')}</InputGroupText>
          </InputGroupAddon>
        </InputGroup>
      </Field>
    </FieldGroup>
  )
}

const TaskChannelSelector: FC<{
  channels: ChannelInfo[]
  channelIds: string[]
  onChange: (value: string[]) => void
  disabled?: boolean
}> = ({ channels, channelIds, onChange, disabled }) => {
  const { t } = useTranslation()

  if (channels.length === 0) return null

  const hasNoChatIds = channelIds.some((id) => !channels.find((channel) => channel.id === id)?.hasActiveChatIds)

  return (
    <Field>
      <FieldLabel>{t('agent.tasks.channels.label')}</FieldLabel>
      <Combobox
        multiple
        size="default"
        width="100%"
        value={channelIds}
        disabled={disabled}
        searchable={channels.length > 5}
        onChange={(nextValue) => {
          if (Array.isArray(nextValue)) onChange(nextValue)
        }}
        placeholder={t('agent.tasks.channels.placeholder')}
        searchPlaceholder={t('agent.tasks.channels.placeholder')}
        emptyText={t('common.no_results')}
        options={channels.map((channel) => ({
          value: channel.id,
          label: channel.name,
          isActive: channel.isActive
        }))}
        renderOption={(option) => (
          <span className="flex min-w-0 items-center gap-2">
            <span
              aria-hidden="true"
              className={`inline-block h-1.5 w-1.5 rounded-full ${option.isActive ? 'bg-success' : 'bg-muted-foreground'}`}
            />
            <span className="truncate">{option.label}</span>
            <span className="sr-only">{t(option.isActive ? 'common.enabled' : 'common.disabled')}</span>
          </span>
        )}
      />
      {hasNoChatIds && <Alert type="warning" showIcon description={t('agent.tasks.channels.noActiveChatIds')} />}
    </Field>
  )
}

const TaskSessionReuseField: FC<{
  value: boolean
  onChange: (value: boolean) => void
  disabled?: boolean
}> = ({ value, onChange, disabled }) => {
  const { t } = useTranslation()

  return (
    <Field>
      <RowFlex className="items-center justify-between gap-3">
        <div className="min-w-0">
          <FieldLabel htmlFor="task-form-reuse-session">{t('agent.tasks.reuseSession.label')}</FieldLabel>
          <ItemDescription>{t('agent.tasks.reuseSession.description')}</ItemDescription>
        </div>
        <Switch
          id="task-form-reuse-session"
          className="shrink-0"
          checked={value}
          disabled={disabled}
          onCheckedChange={onChange}
        />
      </RowFlex>
      {value && <Alert type="warning" showIcon description={t('agent.tasks.reuseSession.warning')} />}
    </Field>
  )
}

const TaskLogsInline: FC<{ taskId: string; agentId: string; tone?: 'settings' | 'workbench' }> = ({
  taskId,
  agentId,
  tone = 'settings'
}) => {
  const { t, i18n } = useTranslation()
  const locale = i18n.language
  const { openConversation } = useConversationNavigation('agents')
  const { logs, isLoading, error: logsError } = useTaskLogs(agentId, taskId)
  const [searchText, setSearchText] = useState('')

  const filteredLogs = useMemo(() => {
    if (!searchText.trim()) return logs
    const query = searchText.toLowerCase()
    return logs.filter(
      (log) =>
        log.result?.toLowerCase().includes(query) ||
        log.error?.toLowerCase().includes(query) ||
        log.status.toLowerCase().includes(query) ||
        new Date(log.startedAt).toLocaleString(locale).toLowerCase().includes(query)
    )
  }, [locale, logs, searchText])

  const columns = useMemo<ColumnDef<TaskRunLogEntity>[]>(
    () => [
      {
        accessorKey: 'startedAt',
        header: t('agent.tasks.logs.runAt'),
        meta: { width: 160 },
        cell: ({ getValue }) =>
          new Date(getValue() as string).toLocaleString(undefined, {
            month: 'numeric',
            day: 'numeric',
            hour: '2-digit',
            minute: '2-digit',
            hour12: false
          })
      },
      {
        accessorKey: 'durationMs',
        header: t('agent.tasks.logs.duration'),
        meta: { width: 80 },
        cell: ({ getValue, row }) => {
          const value = getValue() as number | null
          if (value == null || row.original.status === 'running') return '-'
          if (value < 1000) return `${value}ms`
          if (value < 60_000) return `${(value / 1000).toFixed(1)}s`
          return `${(value / 60_000).toFixed(1)}m`
        }
      },
      {
        accessorKey: 'status',
        header: t('agent.tasks.logs.status'),
        meta: { width: 90 },
        cell: ({ getValue }) => {
          const value = getValue() as string
          const labels: Record<string, string> = {
            completed: t('agent.tasks.logs.completed'),
            running: t('agent.tasks.logs.running'),
            failed: t('agent.tasks.logs.failed'),
            cancelled: t('agent.tasks.logs.cancelled')
          }
          return (
            <Badge variant={value === 'failed' ? 'destructive' : value === 'running' ? 'secondary' : 'outline'}>
              {labels[value] ?? value}
            </Badge>
          )
        }
      },
      {
        id: 'result',
        header: t('agent.tasks.logs.result'),
        meta: { width: 'calc(100% - 330px)', className: 'min-w-0' },
        cell: ({ row }) => {
          const record = row.original
          const isErrorStatus = record.status === 'failed' || record.status === 'cancelled'
          const text =
            record.status === 'running'
              ? t('agent.tasks.logs.running')
              : isErrorStatus
                ? record.error
                : (record.result ?? '-')

          return (
            <RowFlex className="items-start gap-1">
              {record.sessionId && (
                <Tooltip title={t('agent.tasks.logs.viewSession')}>
                  <Button
                    variant="ghost"
                    size="icon-sm"
                    aria-label={t('agent.tasks.logs.viewSession')}
                    onClick={() => openConversation(record.sessionId!)}>
                    <ArrowRight size={13} />
                  </Button>
                </Tooltip>
              )}
              <span className={isErrorStatus ? 'line-clamp-4 text-error' : 'line-clamp-4'}>{text}</span>
            </RowFlex>
          )
        }
      }
    ],
    [openConversation, t]
  )

  if (isLoading) {
    return (
      <Center className="py-4">
        <Spinner text={t('common.loading')} />
      </Center>
    )
  }

  if (logsError) {
    return <EmptyState compact preset="no-result" description={t('agent.tasks.logs.loadError')} />
  }

  if (logs.length === 0) {
    return <EmptyState compact preset="no-result" description={t('agent.tasks.logs.empty')} />
  }

  if (tone === 'workbench') {
    return (
      <div className="flex flex-col gap-3">
        <SearchInput
          value={searchText}
          placeholder={t('agent.tasks.logs.search')}
          aria-label={t('agent.tasks.logs.search')}
          clearLabel={t('common.clear')}
          onClear={() => setSearchText('')}
          onChange={(event) => setSearchText(event.target.value)}
          containerClassName="h-[30px] w-[220px] border-0 bg-[#f5f5f5] shadow-none"
        />
        <ul className="m-0 divide-y divide-[#efefef] border-y border-[#efefef] p-0">
          {filteredLogs.map((record) => {
            let text = record.result ?? '-'
            let statusLabel = t('agent.tasks.logs.completed')
            if (record.status === 'running') {
              text = t('agent.tasks.logs.running')
              statusLabel = text
            } else if (record.status === 'failed') {
              text = record.error ?? '-'
              statusLabel = t('agent.tasks.logs.failed')
            } else if (record.status === 'cancelled') {
              text = record.error ?? '-'
              statusLabel = t('agent.tasks.logs.cancelled')
            }
            return (
              <li key={record.id} className="flex items-start justify-between gap-4 py-3">
                <div className="min-w-0">
                  <p className="m-0 text-[13px] text-[#171717]">{statusLabel}</p>
                  <p className="m-0 mt-1 text-[12px] leading-5 text-[#8a8a8a]">
                    {new Date(record.startedAt).toLocaleString(locale, {
                      month: 'numeric',
                      day: 'numeric',
                      hour: '2-digit',
                      minute: '2-digit',
                      hour12: false
                    })}
                  </p>
                  <p className="m-0 mt-1 text-[13px] leading-5 wrap-anywhere text-[#3f3f3f]">{text}</p>
                </div>
                {record.sessionId ? (
                  <button
                    type="button"
                    aria-label={t('agent.tasks.logs.viewSession')}
                    onClick={() => openConversation(record.sessionId!)}
                    className="inline-flex h-[30px] shrink-0 items-center rounded-[8px] border border-[#e5e5e5] bg-white px-3 text-[13px] text-[#171717] outline-none hover:bg-[#fafafa] focus-visible:ring-2 focus-visible:ring-black/10">
                    {t('agent.tasks.logs.viewSession')}
                  </button>
                ) : null}
              </li>
            )
          })}
        </ul>
      </div>
    )
  }

  return (
    <FieldGroup>
      <SearchInput
        size="sm"
        value={searchText}
        placeholder={t('agent.tasks.logs.search')}
        clearLabel={t('common.clear')}
        onClear={() => setSearchText('')}
        onChange={(event) => setSearchText(event.target.value)}
      />
      <div data-slot="task-logs-table-scroll" className="max-w-full overflow-x-auto">
        <div data-slot="task-logs-table-width" className="min-w-[720px]">
          <DataTable data={filteredLogs} columns={columns} rowKey="id" emptyText={t('agent.tasks.logs.empty')} />
        </div>
      </div>
    </FieldGroup>
  )
}

const TaskDetail: FC<{
  task: ScheduledTaskEntity
  agents: AgentInfo[]
  onBack: () => void
  onUpdate: (taskId: string, updates: AgentTaskPatch) => Promise<TaskUpdateResult | undefined>
  onDelete: (taskId: string) => Promise<void>
  onRun: (taskId: string) => Promise<void>
  onToggleStatus: (taskId: string, newStatus: string) => Promise<void>
  tone?: 'settings' | 'workbench'
}> = ({ task, agents, onBack, onUpdate, onDelete, onRun, onToggleStatus, tone = 'settings' }) => {
  const { t } = useTranslation()
  const { theme } = useTheme()
  const { channels: rawChannels } = useChannels()
  const { openConversation } = useConversationNavigation('agents')
  const isCompleted = task.status === 'completed'
  const agentName = agents.find((agent) => agent.id === task.agentId)?.name ?? task.agentId
  const taskChannels = useMemo(
    () => rawChannels.map(toChannelInfo).filter((channel) => channel.agentId === task.agentId),
    [rawChannels, task.agentId]
  )
  const selectedChannels = useMemo(
    () =>
      (task.channelIds ?? []).map(
        (channelId) => taskChannels.find((channel) => channel.id === channelId) ?? { id: channelId, name: channelId }
      ),
    [task.channelIds, taskChannels]
  )
  const hasUndeliverableChannel = selectedChannels.some((channel) => !channel.hasActiveChatIds)
  const [editOpen, setEditOpen] = useState(false)
  const [deleteConfirmOpen, setDeleteConfirmOpen] = useState(false)
  const [detailSection, setDetailSection] = useState<'prompt' | 'general' | 'history'>('prompt')
  const { data: workspaces, refetch: refetchWorkspaces } = useQuery('/agent-workspaces')
  useDataChange('/agent-workspaces', () => void refetchWorkspaces())

  const workspaceId = task.workspace.type === AGENT_WORKSPACE_TYPE.USER ? task.workspace.workspaceId : null
  const workspaceLabel =
    workspaceId === null
      ? t('agent.session.workspace_selector.no_project')
      : (workspaces?.find((workspace) => workspace.id === workspaceId)?.name ?? workspaceId)

  const formatDateTime = (iso: string | null | undefined) => {
    if (!iso) return '-'
    const date = new Date(iso)
    const diff = Math.abs(Date.now() - date.getTime())
    if (diff < 86_400_000) {
      return date.toLocaleString(undefined, { hour: '2-digit', minute: '2-digit', hour12: false })
    }
    return date.toLocaleString(undefined, {
      month: 'numeric',
      day: 'numeric',
      hour: '2-digit',
      minute: '2-digit',
      hour12: false
    })
  }

  const modelLabel = scheduledTaskModelLabel(task.model ?? agents.find((agent) => agent.id === task.agentId)?.model)
  const skillLabel = task.skills?.length ? task.skills.join(', ') : ''
  const fileLabel = task.files?.length ? task.files.map((file) => file.name).join(', ') : ''
  const detailItems = [
    ...(tone === 'workbench' ? [] : [{ label: t('agent.channels.bindAgent'), value: agentName }]),
    ...(modelLabel ? [{ label: t('selection.settings.user_modal.model.label'), value: modelLabel }] : []),
    ...(skillLabel ? [{ label: t('plugins.skills'), value: skillLabel }] : []),
    ...(fileLabel ? [{ label: t('workbench.composer.mention.title'), value: fileLabel }] : []),
    { label: t('agent.tasks.frequency.label'), value: getTriggerSummary(task.trigger, t) },
    {
      label: t('agent.tasks.timeout.label'),
      value:
        task.timeoutMinutes > 0
          ? `${task.timeoutMinutes} ${t('agent.tasks.intervalUnit')}`
          : t('agent.tasks.timeout.placeholder')
    },
    { label: t('agent.session.display.workdir'), value: workspaceLabel },
    {
      label: t('agent.tasks.reuseSession.label'),
      // A raw session UUID tells the user nothing — show the state, and make the
      // bound case a real affordance (the same jump the run log already offers).
      // The bound session only exists once a fire has run; until then the switch
      // is on but there is nothing to point at yet.
      value: !task.reuseSession ? (
        t('common.disabled')
      ) : task.reuseSessionId ? (
        <Button
          type="button"
          variant="link"
          size="sm"
          className="h-auto p-0"
          onClick={() => openConversation(task.reuseSessionId as string)}>
          {t('agent.tasks.reuseSession.bound')}
          <ArrowRight size={13} />
        </Button>
      ) : (
        t('agent.tasks.reuseSession.pending')
      )
    },
    {
      label: t('agent.tasks.channels.label'),
      value: selectedChannels.length > 0 ? selectedChannels.map((channel) => channel.name).join(', ') : t('common.none')
    },
    { label: t('agent.tasks.lastRun'), value: formatDateTime(task.lastRun) },
    { label: t('agent.tasks.nextRun'), value: formatDateTime(task.nextRun) }
  ]

  const handleEditSave = useCallback(
    async (request: AgentTaskPatch) => {
      const result = await onUpdate(task.id, request)
      return result?.succeeded === true
    },
    [onUpdate, task.id]
  )

  const dialogs = (
    <>
      <TaskFormDialog
        open={editOpen}
        task={task}
        agents={agents}
        tone={tone}
        onOpenChange={setEditOpen}
        onUpdate={handleEditSave}
      />

      <ConfirmDialog
        open={deleteConfirmOpen}
        onOpenChange={setDeleteConfirmOpen}
        title={t('agent.tasks.delete.confirm')}
        confirmText={t('agent.tasks.delete.label')}
        cancelText={t('agent.tasks.cancel')}
        destructive
        onConfirm={() => onDelete(task.id)}
      />
    </>
  )

  if (tone === 'workbench') {
    const statusLabel = getTaskStatusLabel(task.status, t)
    let scheduleLine = `${statusLabel} · ${getTriggerSummary(task.trigger, t)}`
    if (task.nextRun) {
      scheduleLine = `${scheduleLine} · ${t('agent.tasks.nextRun')} ${formatDateTime(task.nextRun)}`
    }
    const showStatusSwitch = task.status === 'active' || task.status === 'paused'
    let detailBody: ReactNode = (
      <div className="w-full rounded-[8px] bg-[#f5f5f5] px-3 py-3 text-[13px] leading-5 wrap-anywhere whitespace-pre-wrap text-[#171717]">
        {task.prompt}
      </div>
    )
    if (detailSection === 'general') {
      detailBody = (
        <div className="divide-y divide-[#efefef] border-y border-[#efefef]">
          {detailItems.map((item) => (
            <div key={item.label} className="flex items-start justify-between gap-6 py-3">
              <span className="shrink-0 text-[13px] text-[#8a8a8a]">{item.label}</span>
              <span className="min-w-0 text-right text-[13px] break-words text-[#171717]">{item.value}</span>
            </div>
          ))}
          {hasUndeliverableChannel ? (
            <Alert type="warning" showIcon description={t('agent.tasks.channels.noActiveChatIds')} />
          ) : null}
        </div>
      )
    } else if (detailSection === 'history') {
      detailBody = <TaskLogsInline taskId={task.id} agentId={task.agentId} tone="workbench" />
    }

    return (
      <div
        data-ui="workbench.automation.detail"
        className="flex h-full min-h-0 w-full min-w-0 flex-col bg-white text-[#171717]">
        <div className="w-full shrink-0 px-6 pt-4 pb-4">
          <div className="flex items-start justify-between gap-4">
            <div className="flex min-w-0 items-start gap-2">
              <button
                type="button"
                aria-label={t('common.back')}
                title={t('common.back')}
                onClick={onBack}
                className="inline-flex size-[30px] shrink-0 items-center justify-center rounded-[8px] text-[#171717] outline-none hover:bg-[#f5f5f5] focus-visible:ring-2 focus-visible:ring-black/10">
                <ArrowLeft className="size-4" strokeWidth={1.7} />
              </button>
              <div className="min-w-0">
                <h1 className="m-0 text-[24px] leading-tight font-bold break-words text-[#171717]">{task.name}</h1>
                <p className="m-0 mt-1 text-[15px] text-[#8a8a8a]">{scheduleLine}</p>
              </div>
            </div>
            <div className="flex shrink-0 items-center gap-2">
              {showStatusSwitch ? (
                <Switch
                  size="sm"
                  checked={task.status === 'active'}
                  onCheckedChange={(checked) => onToggleStatus(task.id, checked ? 'active' : 'paused')}
                  aria-label={t('agent.tasks.status.active')}
                  title={task.status === 'active' ? t('agent.tasks.pause') : t('agent.tasks.resume')}
                />
              ) : null}
              {isCompleted ? null : (
                <button
                  type="button"
                  onClick={() => setEditOpen(true)}
                  className="inline-flex h-[30px] items-center gap-1.5 rounded-[8px] border border-[#e5e5e5] bg-white px-3 text-[13px] text-[#171717] outline-none hover:bg-[#fafafa] focus-visible:ring-2 focus-visible:ring-black/10">
                  <PencilLine className="size-4" strokeWidth={1.7} />
                  {t('common.edit')}
                </button>
              )}
              {isCompleted ? null : (
                <button
                  type="button"
                  onClick={() => void onRun(task.id)}
                  className="inline-flex h-[30px] items-center gap-1.5 rounded-[8px] bg-[#171717] px-3 text-[13px] text-white outline-none hover:bg-[#2a2a2a] focus-visible:ring-2 focus-visible:ring-black/10">
                  <Play className="size-4" strokeWidth={1.7} />
                  {t('agent.tasks.run')}
                </button>
              )}
              <DropdownMenu>
                <DropdownMenuTrigger asChild>
                  <button
                    type="button"
                    aria-label={t('common.more')}
                    className="inline-flex size-[30px] items-center justify-center rounded-[8px] text-[#171717] outline-none hover:bg-[#f5f5f5] focus-visible:ring-2 focus-visible:ring-black/10">
                    <MoreHorizontal className="size-4" strokeWidth={1.7} />
                  </button>
                </DropdownMenuTrigger>
                <DropdownMenuContent align="end">
                  <DropdownMenuGroup>
                    <DropdownMenuItem variant="destructive" onSelect={() => setDeleteConfirmOpen(true)}>
                      <Trash2 />
                      {t('agent.tasks.delete.label')}
                    </DropdownMenuItem>
                  </DropdownMenuGroup>
                </DropdownMenuContent>
              </DropdownMenu>
            </div>
          </div>
          <SegmentedControl
            aria-label={task.name}
            size="sm"
            className="mt-4 h-[30px]"
            value={detailSection}
            onValueChange={setDetailSection}
            options={[
              { value: 'prompt', label: t('agent.tasks.prompt.label') },
              { value: 'general', label: t('settings.general.title') },
              { value: 'history', label: t('agent.tasks.logs.label') }
            ]}
          />
        </div>
        <Scrollbar className="min-h-0 w-full flex-1">
          <div className="w-full px-6 pb-6">{detailBody}</div>
        </Scrollbar>
        {dialogs}
      </div>
    )
  }

  return (
    <SettingsContentColumn theme={theme} className="w-full pt-4" innerClassName="max-w-none">
      <div>
        <SettingTitle className="flex-wrap gap-3">
          <div className="flex min-w-0 flex-1 items-center gap-2">
            <Button
              type="button"
              size="icon-sm"
              variant="ghost"
              className="-ml-2 shrink-0 rounded-full"
              aria-label={t('common.back')}
              title={t('common.back')}
              onClick={onBack}>
              <ArrowLeft size={16} />
            </Button>
            <span className="min-w-0 break-words">{task.name}</span>
            {task.status === 'missed' && <Badge variant="secondary">{t('agent.tasks.status.missed')}</Badge>}
            {!isCompleted && task.status !== 'missed' && (
              <Switch
                className="ml-1 shrink-0"
                size="sm"
                checked={task.status === 'active'}
                onCheckedChange={(checked) => onToggleStatus(task.id, checked ? 'active' : 'paused')}
                aria-label={t('agent.tasks.status.active')}
                title={task.status === 'active' ? t('agent.tasks.pause') : t('agent.tasks.resume')}
              />
            )}
          </div>
          <RowFlex className="flex-wrap items-center gap-2">
            {!isCompleted && (
              <Button type="button" variant="outline" className="min-w-18" onClick={() => setEditOpen(true)}>
                <PencilLine size={14} />
                {t('common.edit')}
              </Button>
            )}
            {!isCompleted && (
              <Button type="button" variant="default" className="min-w-18" onClick={() => void onRun(task.id)}>
                <Play size={14} />
                {t('agent.tasks.run')}
              </Button>
            )}
            <DropdownMenu>
              <DropdownMenuTrigger asChild>
                <Button type="button" size="icon-sm" variant="ghost" aria-label={t('common.more')}>
                  <MoreHorizontal size={14} />
                </Button>
              </DropdownMenuTrigger>
              <DropdownMenuContent align="end">
                <DropdownMenuGroup>
                  <DropdownMenuItem variant="destructive" onSelect={() => setDeleteConfirmOpen(true)}>
                    <Trash2 />
                    {t('agent.tasks.delete.label')}
                  </DropdownMenuItem>
                </DropdownMenuGroup>
              </DropdownMenuContent>
            </DropdownMenu>
          </RowFlex>
        </SettingTitle>
        {task.nextRun && (
          <SettingDescription>
            {t('agent.tasks.nextRun')}: {formatDateTime(task.nextRun)}
          </SettingDescription>
        )}
      </div>

      <SettingGroup theme={theme}>
        <Tabs defaultValue="prompt" variant="line">
          <TabsList aria-label={task.name}>
            <TabsTrigger value="prompt">{t('agent.tasks.prompt.label')}</TabsTrigger>
            <TabsTrigger value="general">{t('settings.general.title')}</TabsTrigger>
            <TabsTrigger value="history">{t('agent.tasks.logs.label')}</TabsTrigger>
          </TabsList>
          <TabsContent value="prompt" className="min-w-0">
            <SettingDivider />
            <Item variant="muted" className="max-w-full min-w-0">
              <ItemContent className="min-w-0">
                <ItemDescription className="line-clamp-none max-w-full min-w-0 wrap-anywhere whitespace-pre-wrap">
                  {task.prompt}
                </ItemDescription>
              </ItemContent>
            </Item>
          </TabsContent>
          <TabsContent value="general">
            <SettingDivider />
            <ItemGroup>
              {detailItems.map((item, index) => (
                <Fragment key={item.label}>
                  {index > 0 && <ItemSeparator />}
                  <Item size="sm">
                    <ItemContent>
                      <ItemTitle>{item.label}</ItemTitle>
                      <ItemDescription className="line-clamp-none break-words">{item.value}</ItemDescription>
                    </ItemContent>
                  </Item>
                </Fragment>
              ))}
            </ItemGroup>
            {hasUndeliverableChannel && (
              <Alert type="warning" showIcon description={t('agent.tasks.channels.noActiveChatIds')} />
            )}
          </TabsContent>
          <TabsContent value="history">
            <SettingDivider />
            <TaskLogsInline taskId={task.id} agentId={task.agentId} />
          </TabsContent>
        </Tabs>
      </SettingGroup>

      {dialogs}
    </SettingsContentColumn>
  )
}

type TaskFormDialogProps = {
  open: boolean
  agents: AgentInfo[]
  onOpenChange: (open: boolean) => void
  tone?: 'settings' | 'workbench'
} & (
  | {
      task: ScheduledTaskEntity
      onUpdate: (request: AgentTaskPatch) => Promise<boolean>
      onCreate?: never
    }
  | {
      task?: undefined
      onCreate: (agentId: string, request: AgentTaskForm) => Promise<boolean>
      onUpdate?: never
    }
)

const TaskFormDialog: FC<TaskFormDialogProps> = (props) => {
  const { open, task, agents, onOpenChange, tone = 'settings' } = props
  const workbenchForm = tone === 'workbench'
  const { t } = useTranslation()
  const { channels: rawChannels, error: channelsError, isLoading: channelsLoading } = useChannels()
  const channelsReady = !channelsLoading && !channelsError
  const isEditing = task !== undefined
  const [agentId, setAgentId] = useState<string | null>(null)
  const [name, setName] = useState('')
  const [prompt, setPrompt] = useState('')
  const [schedule, setSchedule] = useState<ScheduleFormState>(DEFAULT_SCHEDULE)
  const [channelIds, setChannelIds] = useState<string[]>([])
  const [workspaceId, setWorkspaceId] = useState<string | null>(null)
  const [reuseSession, setReuseSession] = useState(false)
  const [modelId, setModelId] = useState<string | null>(null)
  const [skills, setSkills] = useState<string[]>([])
  const [files, setFiles] = useState<TaskFileRef[]>([])
  const [saving, setSaving] = useState(false)
  const [submitted, setSubmitted] = useState(false)
  const [promptPreviewKey, setPromptPreviewKey] = useState(0)
  const wasOpenRef = useRef(false)
  const initialDraftRef = useRef<TaskDraftSnapshot | null>(null)
  const { data: workspaces, refetch: refetchWorkspaces } = useQuery('/agent-workspaces')
  useDataChange('/agent-workspaces', () => void refetchWorkspaces())

  useEffect(() => {
    if (open && !wasOpenRef.current) {
      const draft = task ? taskToDraftSnapshot(task) : null
      initialDraftRef.current = draft
      let nextAgentId: string | null = null
      if (task?.agentId) nextAgentId = task.agentId
      else if (workbenchForm) nextAgentId = agents[0]?.id ?? null
      else if (agents.length === 1) nextAgentId = agents[0].id
      setAgentId(nextAgentId)
      setName(draft?.name ?? '')
      setPrompt(draft?.prompt ?? '')
      setSchedule(draft?.schedule ?? DEFAULT_SCHEDULE)
      setChannelIds(draft?.channelIds ?? [])
      setWorkspaceId(draft?.workspaceId ?? null)
      setReuseSession(draft?.reuseSession ?? false)
      const agentModel = agents.find((agent) => agent.id === nextAgentId)?.model ?? null
      setModelId(draft ? draft.model : agentModel)
      setSkills(draft?.skills ?? [])
      setFiles(draft?.files ?? [])
      setSaving(false)
      setSubmitted(false)
      setPromptPreviewKey((key) => key + 1)
    }
    wasOpenRef.current = open
  }, [agents, open, task, workbenchForm])

  const availableChannels = useMemo(
    () => (agentId ? rawChannels.map(toChannelInfo).filter((channel) => channel.agentId === agentId) : []),
    [agentId, rawChannels]
  )

  useEffect(() => {
    // Never prune against an unloaded channel list: with the channels query
    // still in flight (or failed) every draft binding would be dropped and the
    // next save would silently clear the task's subscriptions.
    if (!channelsReady) return
    setChannelIds((current) =>
      current.filter((channelId) => availableChannels.some((channel) => channel.id === channelId))
    )
  }, [availableChannels, channelsReady])

  const isSystemWorkspace = workspaceId === null
  const homeWorkspace = defaultAppWorkspacePath(readStudioHomePath())
  const selectedWorkspacePath =
    workspaceId === null ? '' : (workspaces?.find((workspace) => workspace.id === workspaceId)?.path ?? '')
  const fileRoot = fileMentionRoot(selectedWorkspacePath, homeWorkspace)
  const selectedAgent = agents.find((agent) => agent.id === agentId)
  const workspaceLabel = isSystemWorkspace
    ? t('agent.session.workspace_selector.no_project')
    : (workspaces?.find((workspace) => workspace.id === workspaceId)?.name ?? workspaceId)
  const trigger = useMemo(() => formStateToTrigger(schedule), [schedule])
  const unchangedExistingSchedule =
    props.task !== undefined &&
    scheduleInputsEqual(schedule, initialDraftRef.current?.schedule ?? taskToDraftSnapshot(props.task).schedule)
  const resolvedTrigger = trigger ?? (unchangedExistingSchedule ? props.task?.trigger : null)

  const handleSave = useCallback(async () => {
    setSubmitted(true)
    if (!agentId || !name.trim() || !prompt.trim() || !resolvedTrigger) return

    setSaving(true)
    try {
      const timeout = Number(schedule.timeoutMinutes)
      const workspace =
        workspaceId === null
          ? ({ type: AGENT_WORKSPACE_TYPE.SYSTEM } as const)
          : ({ type: AGENT_WORKSPACE_TYPE.USER, workspaceId } as const)
      const timeoutMinutes = Number.isInteger(timeout) && timeout > 0 ? timeout : null

      let saved: boolean
      if (props.task) {
        const initialDraft = initialDraftRef.current ?? taskToDraftSnapshot(props.task)
        const updates: AgentTaskPatch = {}

        if (name !== initialDraft.name) updates.name = name.trim()
        if (prompt !== initialDraft.prompt) updates.prompt = prompt.trim()
        if (schedule.timeoutMinutes !== initialDraft.schedule.timeoutMinutes) {
          updates.timeoutMinutes = timeoutMinutes
        }
        if (workspaceId !== initialDraft.workspaceId) updates.workspace = workspace
        if (reuseSession !== initialDraft.reuseSession) updates.reuseSession = reuseSession
        const chosenModel = modelId && isUniqueModelId(modelId) ? modelId : null
        if (chosenModel !== initialDraft.model) updates.model = chosenModel
        if (!stringArraysEqual(skills, initialDraft.skills)) updates.skills = skills
        if (!taskFilesEqual(files, initialDraft.files)) updates.files = files
        if (!stringArraysEqual(channelIds, initialDraft.channelIds)) updates.channelIds = channelIds
        if (!scheduleInputsEqual(schedule, initialDraft.schedule)) {
          const nextTrigger = preserveCompatibleTriggerMetadata(props.task.trigger, resolvedTrigger)
          if (!triggersEqual(nextTrigger, props.task.trigger)) updates.trigger = nextTrigger
        }

        saved = Object.keys(updates).length === 0 || (await props.onUpdate(updates))
      } else {
        const chosenModel = modelId && isUniqueModelId(modelId) ? modelId : null
        saved = await props.onCreate(agentId, {
          name: name.trim(),
          prompt: prompt.trim(),
          trigger: resolvedTrigger,
          workspace,
          timeoutMinutes,
          reuseSession,
          channelIds: channelIds.length > 0 ? channelIds : undefined,
          ...(chosenModel ? { model: chosenModel } : {}),
          ...(skills.length > 0 ? { skills } : {}),
          ...(files.length > 0 ? { files } : {})
        })
      }
      if (saved) onOpenChange(false)
    } finally {
      setSaving(false)
    }
  }, [
    agentId,
    channelIds,
    files,
    modelId,
    name,
    onOpenChange,
    prompt,
    props,
    reuseSession,
    schedule,
    resolvedTrigger,
    skills,
    workspaceId
  ])

  const workspaceControl = (
    <WorkspaceSelector
      value={workspaceId}
      onChange={(nextWorkspaceId) => {
        setWorkspaceId(nextWorkspaceId)
        const nextProject =
          nextWorkspaceId === null
            ? ''
            : (workspaces?.find((workspace) => workspace.id === nextWorkspaceId)?.path ?? '')
        const nextRoot = fileMentionRoot(nextProject, homeWorkspace)
        setFiles((current) => current.filter((file) => taskFileInsideRoot(file.path, nextRoot)))
      }}
      disabled={saving}
      align="start"
      trigger={
        <Button
          type="button"
          variant="ghost"
          size="sm"
          disabled={saving}
          aria-label={t('agent.session.display.workdir')}>
          {isSystemWorkspace ? <CircleSlash size={14} /> : <Folder size={14} />}
          <span>{workspaceLabel}</span>
          <ChevronDown size={14} />
        </Button>
      }
    />
  )

  const nameField = (
    <Field data-invalid={(submitted && !name.trim()) || undefined}>
      <FieldLabel required htmlFor="task-form-name">
        {t('agent.tasks.name.label')}
      </FieldLabel>
      <Input
        autoFocus
        id="task-form-name"
        value={name}
        disabled={saving}
        required
        placeholder={t('agent.tasks.name.placeholder')}
        aria-invalid={(submitted && !name.trim()) || undefined}
        onChange={(event) => setName(event.target.value)}
      />
      <FieldError>{submitted && !name.trim() ? t('settings.scheduledTasks.validation.name') : undefined}</FieldError>
    </Field>
  )

  const scheduleFields = (
    <>
      <TaskScheduleControls
        value={schedule}
        disabled={saving}
        invalid={submitted && !resolvedTrigger}
        tone={tone}
        onChange={setSchedule}
      />
      <TaskSessionReuseField value={reuseSession} disabled={saving} onChange={setReuseSession} />
      <TaskChannelSelector
        channels={availableChannels}
        channelIds={channelIds}
        disabled={saving}
        onChange={setChannelIds}
      />
    </>
  )

  return (
    <Dialog open={open} onOpenChange={(nextOpen) => !saving && onOpenChange(nextOpen)}>
      <DialogContent
        size="xl"
        closeOnOverlayClick={!saving}
        className={
          // The schedule menu portals into this transformed dialog. overflow:hidden
          // clips that menu against the card edge.
          workbenchForm ? 'flex max-h-[min(820px,calc(100vh-3rem))] flex-col' : undefined
        }>
        <form
          noValidate
          className="contents"
          onSubmit={(event) => {
            event.preventDefault()
            if (!saving) void handleSave()
          }}
          onKeyDown={(event) => {
            if ((event.metaKey || event.ctrlKey) && event.key === 'Enter' && !saving) {
              event.preventDefault()
              void handleSave()
            }
          }}>
          <DialogHeader className={workbenchForm ? 'shrink-0' : undefined}>
            <DialogTitle className={workbenchForm ? 'text-[15px] leading-none font-medium text-[#171717]' : undefined}>
              {t(isEditing ? 'settings.scheduledTasks.editTitle' : 'settings.scheduledTasks.createTitle')}
            </DialogTitle>
            <DialogDescription className={workbenchForm ? 'text-[13px] leading-5 text-[#8a8a8a]' : undefined}>
              {t(isEditing ? 'settings.scheduledTasks.editDescription' : 'settings.scheduledTasks.createDescription')}
            </DialogDescription>
          </DialogHeader>
          {workbenchForm ? (
            <div className="flex min-h-0 flex-1 flex-col gap-4">
              {nameField}
              <Field data-invalid={(submitted && !prompt.trim()) || undefined}>
                <FieldLabel required>{t('agent.tasks.prompt.label')}</FieldLabel>
                <TaskPromptComposer
                  prompt={prompt}
                  onPromptChange={setPrompt}
                  disabled={saving}
                  invalid={submitted && !prompt.trim()}
                  label={t('agent.tasks.prompt.label')}
                  toolbar={
                    <>
                      <TaskModelField value={modelId} disabled={saving} onChange={setModelId} />
                      <TaskSkillField value={skills} disabled={saving} onChange={setSkills} />
                      <TaskFileField root={fileRoot} value={files} disabled={saving} onChange={setFiles} />
                      {workspaceControl}
                    </>
                  }
                />
                <FieldError>
                  {submitted && !prompt.trim() ? t('settings.scheduledTasks.validation.prompt') : undefined}
                </FieldError>
              </Field>
              <Scrollbar className="min-h-0 flex-1 pr-1">
                <FieldGroup>{scheduleFields}</FieldGroup>
              </Scrollbar>
            </div>
          ) : (
            <Scrollbar className="-m-1 max-h-[60vh] p-1 pr-3">
              <FieldGroup>
                {nameField}

                <FieldGroup data-task-input-context>
                  <PromptEditorField
                    label={<FieldLabel required>{t('agent.tasks.prompt.label')}</FieldLabel>}
                    value={prompt}
                    onChange={setPrompt}
                    placeholder={t('agent.tasks.prompt.placeholder')}
                    error={submitted && !prompt.trim() ? t('settings.scheduledTasks.validation.prompt') : undefined}
                    resetPreviewKey={promptPreviewKey}
                    minHeight="100px"
                    actions={
                      <PromptPolishActions
                        value={prompt}
                        fallbackSource={name}
                        emptyValueSystemPrompt={TASK_PROMPT_GENERATION_SYSTEM_PROMPT}
                        existingValueSystemPrompt={TASK_PROMPT_POLISH_SYSTEM_PROMPT}
                        disabled={saving}
                        onChange={(value) => {
                          setPrompt(value)
                          setPromptPreviewKey((key) => key + 1)
                        }}
                      />
                    }
                  />
                  <RowFlex className="flex-wrap items-center gap-2">
                    <TaskModelField value={modelId} disabled={saving} onChange={setModelId} />
                    <TaskSkillField value={skills} disabled={saving} onChange={setSkills} />
                    <TaskFileField root={fileRoot} value={files} disabled={saving} onChange={setFiles} />
                    <AgentSelector
                      value={agentId}
                      onChange={(nextAgentId) => {
                        setAgentId(nextAgentId)
                        setChannelIds([])
                        setModelId(agents.find((agent) => agent.id === nextAgentId)?.model ?? null)
                      }}
                      align="start"
                      mountStrategy="lazy-keep"
                      trigger={
                        <Button
                          type="button"
                          variant="outline"
                          disabled={saving || isEditing}
                          aria-label={t('agent.channels.bindAgent')}
                          aria-invalid={(submitted && !agentId) || undefined}
                          aria-busy={saving || undefined}>
                          <Bot size={14} />
                          <span>{selectedAgent?.name ?? t('agent.channels.selectAgent')}</span>
                          <ChevronDown size={14} />
                        </Button>
                      }
                    />
                    {workspaceControl}
                  </RowFlex>
                  <FieldError>
                    {submitted && !agentId ? t('settings.scheduledTasks.validation.agent') : undefined}
                  </FieldError>
                </FieldGroup>

                {scheduleFields}
              </FieldGroup>
            </Scrollbar>
          )}
          <DialogFooter className={workbenchForm ? 'shrink-0' : undefined}>
            <Button
              type="button"
              variant="outline"
              size={workbenchForm ? 'md' : undefined}
              disabled={saving}
              className={workbenchForm ? 'rounded-[8px]' : undefined}
              onClick={() => onOpenChange(false)}>
              {t('common.cancel')}
            </Button>
            <Button
              type="submit"
              variant={workbenchForm ? 'emphasis' : undefined}
              size={workbenchForm ? 'md' : undefined}
              disabled={saving}
              loading={saving}
              aria-busy={saving}
              className={workbenchForm ? 'rounded-[8px]' : undefined}>
              {t('agent.tasks.save')}
            </Button>
          </DialogFooter>
        </form>
      </DialogContent>
    </Dialog>
  )
}

const TasksSettings: FC<{ surface?: 'settings' | 'workbench'; onCreateWithAgent?: () => void }> = ({
  surface = 'settings',
  onCreateWithAgent
}) => {
  const workbench = surface === 'workbench'
  const { t } = useTranslation()
  const { theme } = useTheme()
  const navigate = useNavigate()
  const params = useParams({ strict: false })
  const [localTaskId, setLocalTaskId] = useState<string | undefined>()
  const taskId = workbench ? localTaskId : params.taskId
  const { createTask } = useCreateTask()
  const { updateTask } = useUpdateTask()
  const { deleteTask } = useDeleteTask()
  const { runTask } = useRunTask()
  const { setTaskEnabled } = useSetTaskEnabled()

  // Mirror AgentSelector's query exactly so both read one shared SWR cache
  // entry: the selector can list (and create in-place) up to the same server
  // cap, and a page-local list capped lower would miss those agents' tasks.
  const {
    data: agentsData,
    error: agentsError,
    isLoading: agentsLoading
  } = useQuery('/agents', { query: { limit: AGENTS_MAX_LIMIT } })
  const agents: AgentInfo[] = useMemo(
    () =>
      (agentsData?.items ?? []).map((agent) => ({
        id: agent.id,
        name: agent.name,
        model: typeof agent.model === 'string' ? agent.model : null
      })),
    [agentsData]
  )
  const createWithAgent = () => {
    if (agents.length > 0 && onCreateWithAgent) {
      onCreateWithAgent()
      return
    }
    if (agents.length > 0) {
      openRoute('/app/agents', { intent: 'schedule' })
      return
    }
    openRoute('/app/agents')
  }
  const {
    tasks,
    total,
    page,
    pageCount,
    error: tasksError,
    isLoading: tasksLoading,
    hasNext,
    hasPrev,
    nextPage,
    prevPage,
    refetch: refetchTasks
  } = useAllTasks(workbench ? WORKBENCH_TASK_LIMIT : undefined)
  const { task: taskDetails, error: taskError, isLoading: taskLoading } = useTask(taskId ?? null)
  const listedTask = taskId ? tasks.find((task) => task.id === taskId) : undefined
  const loading = (agentsLoading && agents.length === 0) || (tasksLoading && tasks.length === 0 && !listedTask)
  const [createOpen, setCreateOpen] = useState(false)
  const [searchQuery, setSearchQuery] = useState('')
  const [agentFilter, setAgentFilter] = useState(ALL_TASKS_FILTER)
  const [statusFilter, setStatusFilter] = useState(ALL_TASKS_FILTER)
  const [localPage, setLocalPage] = useState(1)
  const filterKey = `${searchQuery}\0${agentFilter}\0${statusFilter}`
  const [seenFilter, setSeenFilter] = useState(filterKey)
  if (filterKey !== seenFilter) {
    setSeenFilter(filterKey)
    setLocalPage(1)
  }
  const taskUpdateTailsRef = useRef<Map<string, Promise<boolean>> | null>(null)

  const agentNameById = useMemo(() => new Map(agents.map((agent) => [agent.id, agent.name])), [agents])

  const filteredTasks = useMemo(() => {
    const normalizedQuery = searchQuery.trim().toLocaleLowerCase()

    return tasks.filter((task) => {
      if (agentFilter !== ALL_TASKS_FILTER && task.agentId !== agentFilter) return false
      if (statusFilter !== ALL_TASKS_FILTER && task.status !== statusFilter) return false
      if (!normalizedQuery) return true

      const fields = workbench ? [task.name] : [task.name, agentNameById.get(task.agentId) ?? task.agentId]
      return fields.some((value) => value.toLocaleLowerCase().includes(normalizedQuery))
    })
  }, [agentFilter, agentNameById, searchQuery, statusFilter, tasks, workbench])

  const hasActiveFilters =
    searchQuery.trim().length > 0 || agentFilter !== ALL_TASKS_FILTER || statusFilter !== ALL_TASKS_FILTER

  const clearFilters = useCallback(() => {
    setSearchQuery('')
    setAgentFilter(ALL_TASKS_FILTER)
    setStatusFilter(ALL_TASKS_FILTER)
  }, [])

  useEffect(() => {
    if (agentsError || tasksError || taskError) {
      logger.error('Failed to load tasks settings', agentsError ?? tasksError ?? taskError)
      toast.error(t('agent.tasks.error.loadFailed'))
    }
  }, [agentsError, t, taskError, tasksError])

  const getTaskUpdateTails = useCallback(() => {
    taskUpdateTailsRef.current ??= new Map()
    return taskUpdateTailsRef.current
  }, [])

  const enqueueTaskOperation = useCallback(
    (selectedTaskId: string, operation: (previousSucceeded: boolean) => Promise<boolean>): Promise<boolean> => {
      const tails = getTaskUpdateTails()
      const previous = tails.get(selectedTaskId) ?? Promise.resolve(true)
      const current = previous
        .catch(() => false)
        .then(operation)
        .catch(() => false)

      tails.set(selectedTaskId, current)
      void current.then(() => {
        if (tails.get(selectedTaskId) === current) tails.delete(selectedTaskId)
      })
      return current
    },
    [getTaskUpdateTails]
  )

  const closeTask = useCallback(() => {
    if (workbench) {
      setLocalTaskId(undefined)
      return
    }
    void navigate({ to: '/settings/scheduled-tasks' })
  }, [navigate, workbench])

  const closeTaskRef = useRef(closeTask)
  closeTaskRef.current = closeTask
  const searchQueryRef = useRef(searchQuery)
  searchQueryRef.current = searchQuery

  useEffect(() => {
    if (!workbench) return
    const onKeyDown = (event: KeyboardEvent) => {
      if (event.isComposing || event.key === 'Process') return
      const target = event.target
      const editable =
        target instanceof HTMLElement &&
        (target.tagName === 'INPUT' || target.tagName === 'TEXTAREA' || target.isContentEditable)
      const overlay =
        target instanceof Element && Boolean(target.closest('[role="dialog"], [role="menu"], [role="listbox"]'))
      const mod = event.metaKey || event.ctrlKey
      if (mod && !event.altKey && event.key.toLowerCase() === 'f' && !event.repeat) {
        const field = document.querySelector<HTMLInputElement>('[data-ui="workbench.automation"] input')
        if (!field) return
        event.preventDefault()
        event.stopPropagation()
        field.focus()
        field.select()
        return
      }
      if (event.key === 'Escape' && !mod && !event.altKey && !event.shiftKey) {
        if (event.defaultPrevented || editable || overlay) return
        if (localTaskId) {
          event.preventDefault()
          closeTaskRef.current()
          return
        }
        if (searchQueryRef.current) {
          event.preventDefault()
          setSearchQuery('')
        }
        return
      }
      if (event.key !== 'ArrowDown' && event.key !== 'ArrowUp') return
      if (mod || event.altKey || event.shiftKey || editable || overlay) return
      if (!(target instanceof Element) || !target.closest('[data-ui="workbench.automation.statuses"]')) return
      const items = Array.from(
        document.querySelectorAll<HTMLButtonElement>('[data-ui="workbench.automation.statuses"] button')
      )
      if (items.length === 0) return
      const current = items.findIndex((item) => item.getAttribute('aria-current') === 'page')
      const nextIndex = current < 0 ? 0 : (current + (event.key === 'ArrowDown' ? 1 : -1) + items.length) % items.length
      event.preventDefault()
      items[nextIndex].focus()
      items[nextIndex].click()
    }
    window.addEventListener('keydown', onKeyDown)
    return () => window.removeEventListener('keydown', onKeyDown)
  }, [localTaskId, workbench])

  const handleCreate = useCallback(
    async (agentId: string, request: AgentTaskForm) => {
      const created = await createTask(agentId, request)
      if (!created) return undefined
      if (workbench) {
        setLocalTaskId(created.id)
        return created
      }
      await navigate({ to: '/settings/scheduled-tasks/$taskId', params: { taskId: created.id } })
      return created
    },
    [createTask, navigate, workbench]
  )

  const persistTaskUpdate = useCallback(
    async (task: ScheduledTaskEntity, updates: AgentTaskPatch): Promise<TaskUpdateResult> => {
      const updated = await updateTask(task.agentId, task.id, updates)
      if (!updated) return { succeeded: false, task }
      return { succeeded: true, task: updated }
    },
    [updateTask]
  )

  const getTaskForAction = useCallback(
    (selectedTaskId: string) => {
      if (taskDetails?.id === selectedTaskId) return taskDetails
      return tasks.find((task) => task.id === selectedTaskId)
    },
    [taskDetails, tasks]
  )

  const handleUpdate = useCallback(
    (selectedTaskId: string, updates: AgentTaskPatch): Promise<TaskUpdateResult | undefined> => {
      const task = getTaskForAction(selectedTaskId)
      if (!task) return Promise.resolve(undefined)

      let updateResult: TaskUpdateResult | undefined
      return enqueueTaskOperation(selectedTaskId, async (previousSucceeded) => {
        updateResult = await persistTaskUpdate(task, updates)
        return previousSucceeded && updateResult.succeeded
      }).then(() => updateResult)
    },
    [enqueueTaskOperation, getTaskForAction, persistTaskUpdate]
  )

  const handleDelete = useCallback(
    async (selectedTaskId: string) => {
      const task = getTaskForAction(selectedTaskId)
      if (!task) return
      await deleteTask(task.agentId, selectedTaskId, {
        onDeleted: () => closeTask()
      })
    },
    [closeTask, deleteTask, getTaskForAction]
  )

  const handleRun = useCallback(
    async (selectedTaskId: string) => {
      await enqueueTaskOperation(selectedTaskId, async (previousSucceeded) => {
        if (!previousSucceeded) return false
        const task = getTaskForAction(selectedTaskId)
        if (!task) return false
        const ran = await runTask(task.agentId, selectedTaskId)
        if (!ran) return false
        await refetchTasks()
        return true
      })
    },
    [enqueueTaskOperation, getTaskForAction, refetchTasks, runTask]
  )

  const handleToggleStatus = useCallback(
    async (selectedTaskId: string, newStatus: string) => {
      const task = getTaskForAction(selectedTaskId)
      if (!task) return
      await enqueueTaskOperation(selectedTaskId, async (previousSucceeded) => {
        const enabled = newStatus === 'active'
        if (enabled && !previousSucceeded) return false
        const updated = await setTaskEnabled(task.agentId, task.id, enabled)
        return previousSucceeded && updated !== undefined
      })
    },
    [enqueueTaskOperation, getTaskForAction, setTaskEnabled]
  )

  if (loading && (!workbench || taskId)) {
    if (workbench) {
      return (
        <div
          data-ui="workbench.automation"
          className="relative flex h-full min-h-0 min-w-0 flex-col bg-white text-[#171717]">
          <PageEntry id="workbench.automation" ready={false} />
        </div>
      )
    }
    return (
      <Center className="flex-1">
        <Spinner text={t('common.loading')} />
      </Center>
    )
  }

  const selectedTask = taskId ? (taskDetails ?? listedTask) : undefined

  if (taskId) {
    if (!selectedTask) {
      if (taskLoading) {
        if (workbench) {
          return (
            <div
              data-ui="workbench.automation"
              className="relative flex h-full min-h-0 min-w-0 flex-col bg-white text-[#171717]">
              <PageEntry id="workbench.automation" ready={false} />
            </div>
          )
        }
        return (
          <Center className="flex-1">
            <Spinner text={t('common.loading')} />
          </Center>
        )
      }
      return (
        <SettingsContentColumn theme={theme}>
          <EmptyState
            preset="no-result"
            title={t('settings.scheduledTasks.notFoundTitle')}
            description={t('settings.scheduledTasks.notFoundDescription')}
            actionLabel={t('common.back')}
            onAction={closeTask}
          />
        </SettingsContentColumn>
      )
    }

    return (
      <TaskDetail
        key={selectedTask.id}
        task={selectedTask}
        agents={agents}
        onBack={closeTask}
        onUpdate={handleUpdate}
        onDelete={handleDelete}
        onRun={handleRun}
        onToggleStatus={handleToggleStatus}
        tone={workbench ? 'workbench' : 'settings'}
      />
    )
  }

  if (workbench) {
    const statusHeading =
      STATUS_FILTERS.find((item) => item.id === statusFilter)?.labelKey ?? 'settings.scheduledTasks.allStatuses'
    let listBody: 'empty' | 'filtered' | 'tasks' = 'tasks'
    if (filteredTasks.length === 0 && hasActiveFilters) listBody = 'filtered'
    else if (filteredTasks.length === 0) listBody = 'empty'
    const workbenchPageCount = Math.max(1, Math.ceil(filteredTasks.length / WORKBENCH_PAGE_SIZE))
    const visibleTasks = filteredTasks.slice((localPage - 1) * WORKBENCH_PAGE_SIZE, localPage * WORKBENCH_PAGE_SIZE)

    return (
      <div
        data-ui="workbench.automation"
        className="@container/automation relative flex h-full min-h-0 min-w-0 flex-col bg-white text-[#171717]">
        <div className="shrink-0 bg-white px-6 pt-4 pb-4">
          <div className="flex flex-col gap-2">
            <div className="flex items-center justify-between gap-4">
              <h1 className="m-0 min-w-0 truncate text-[24px] leading-tight font-bold text-[#171717]">
                {t('workbench.nav.scheduled')}
              </h1>
              <div className="flex shrink-0 items-center gap-2">
                <SearchInput
                  value={searchQuery}
                  onChange={(event) => setSearchQuery(event.target.value)}
                  onClear={() => setSearchQuery('')}
                  clearLabel={t('common.clear')}
                  aria-label={t('settings.scheduledTasks.search')}
                  placeholder={t('settings.scheduledTasks.searchPlaceholder')}
                  containerClassName="h-[30px] w-[220px] border-0 bg-[#f5f5f5] shadow-none has-[[data-slot=input-group-control]:focus-visible]:border-0"
                />
                <DropdownMenu>
                  <DropdownMenuTrigger asChild>
                    <button
                      type="button"
                      className="inline-flex h-[30px] items-center justify-center gap-1.5 rounded-[8px] bg-[#171717] px-3 text-[13px] leading-none text-white outline-none hover:bg-[#2a2a2a] focus-visible:ring-2 focus-visible:ring-black/10">
                      <Plus size={16} className="shrink-0 text-current" strokeWidth={1.7} aria-hidden />
                      {t('settings.scheduledTasks.newTask')}
                      <ChevronDown size={14} className="shrink-0 text-current" strokeWidth={1.7} aria-hidden />
                    </button>
                  </DropdownMenuTrigger>
                  <DropdownMenuContent align="end" className="min-w-40">
                    <DropdownMenuGroup>
                      <DropdownMenuItem disabled={agents.length === 0} onSelect={() => setCreateOpen(true)}>
                        <PencilLine />
                        {t('settings.scheduledTasks.manualCreate')}
                      </DropdownMenuItem>
                      <DropdownMenuItem onSelect={createWithAgent}>
                        <Bot />
                        {t('settings.scheduledTasks.agentCreate')}
                      </DropdownMenuItem>
                    </DropdownMenuGroup>
                  </DropdownMenuContent>
                </DropdownMenu>
              </div>
            </div>
            <p className="m-0 text-[15px] text-[#8a8a8a]">{t('settings.scheduledTasks.createDescription')}</p>
          </div>
        </div>

        <div className="flex min-h-0 min-w-0 flex-1 flex-col @[640px]/automation:flex-row">
          <nav
            aria-label={t('settings.scheduledTasks.filterStatus')}
            data-ui="workbench.automation.statuses"
            className="flex min-h-0 min-w-0 gap-0.5 overflow-x-auto bg-white px-3 py-2 @[640px]/automation:w-[200px] @[640px]/automation:min-w-[200px] @[640px]/automation:shrink-0 @[640px]/automation:flex-col @[640px]/automation:overflow-y-auto @[640px]/automation:border-r @[640px]/automation:border-[#efefef] @[640px]/automation:pt-2 @[640px]/automation:pb-4">
            {STATUS_FILTERS.map((item) => {
              const selected = item.id === statusFilter
              const Icon = item.icon
              return (
                <button
                  key={item.id}
                  type="button"
                  aria-current={selected ? 'page' : undefined}
                  onClick={() => setStatusFilter(item.id)}
                  className={cn(
                    'flex h-9 shrink-0 items-center gap-2.5 rounded-[8px] px-2.5 text-left text-[13px] transition-colors outline-none focus-visible:ring-2 focus-visible:ring-black/10 focus-visible:ring-inset',
                    selected
                      ? 'bg-[#f2f3f5] font-medium text-[#171717]'
                      : 'text-[#3f3f3f] hover:bg-[#f7f7f7] hover:text-[#171717]'
                  )}>
                  <Icon className="size-4 shrink-0 text-current" strokeWidth={1.7} />
                  <span className="truncate">{t(item.labelKey)}</span>
                </button>
              )
            })}
          </nav>

          <Scrollbar className="min-h-0 min-w-0 flex-1">
            <div className="flex min-h-full flex-col px-6 pt-4 pb-6">
              <h2 className="m-0 text-[18px] font-semibold text-[#1a1a1a]">{t(statusHeading)}</h2>
              {listBody === 'empty' ? (
                <EmptyState
                  preset={agents.length === 0 ? 'no-agent' : 'no-result'}
                  icon={agents.length === 0 ? undefined : CalendarClock}
                  title={
                    agents.length === 0
                      ? t('settings.scheduledTasks.noAgentsTitle')
                      : t('settings.scheduledTasks.noTasksTitle')
                  }
                  description={
                    agents.length === 0 ? t('settings.scheduledTasks.noAgents') : t('settings.scheduledTasks.noTasks')
                  }
                  actionLabel={agents.length === 0 ? t('settings.scheduledTasks.agentCreate') : undefined}
                  className="py-16"
                  onAction={agents.length === 0 ? () => openRoute('/app/agents') : undefined}
                />
              ) : null}
              {listBody === 'filtered' ? (
                <EmptyState
                  preset="no-result"
                  title={t('settings.scheduledTasks.noMatchesTitle')}
                  description={t('settings.scheduledTasks.noMatches')}
                  actionLabel={t('settings.scheduledTasks.clearFilters')}
                  className="py-16"
                  onAction={clearFilters}
                />
              ) : null}
              {listBody === 'tasks' ? (
                <div className="mt-4 grid grid-cols-1 gap-3 min-[900px]:grid-cols-2">
                  {visibleTasks.map((task) => (
                    <button
                      key={task.id}
                      type="button"
                      onClick={() => setLocalTaskId(task.id)}
                      className="flex min-w-0 items-center gap-3 rounded-[14px] border border-[#ececec] bg-white px-4 py-3 text-left shadow-[0_8px_22px_rgba(20,40,80,0.04)] transition-[border-color,box-shadow] outline-none hover:border-[#e0e0e0] hover:shadow-[0_10px_28px_rgba(20,40,80,0.10)] focus-visible:ring-2 focus-visible:ring-black/10">
                      <span className="grid size-8 shrink-0 place-items-center rounded-[8px] bg-[#f5f5f5] text-[#171717]">
                        <CalendarClock className="size-4" strokeWidth={1.7} aria-hidden />
                      </span>
                      <span className="min-w-0 flex-1">
                        <span className="block truncate text-[14px] font-medium text-[#1a1a1a]">{task.name}</span>
                        <span className="mt-1 block truncate text-[13px] leading-5 text-[#8a8a8a]">
                          {getTriggerSummary(task.trigger, t)}
                          {' · '}
                          {getTaskStatusLabel(task.status, t)}
                        </span>
                      </span>
                      <TaskCardRunStatus task={task} />
                    </button>
                  ))}
                </div>
              ) : null}
              {workbenchPageCount > 1 ? (
                <div className="mt-4 flex items-center justify-center gap-3">
                  <button
                    type="button"
                    disabled={localPage <= 1}
                    onClick={() => setLocalPage((page) => Math.max(1, page - 1))}
                    className="h-[30px] rounded-[8px] border border-[#e5e5e5] bg-white px-3 text-[13px] text-[#171717] outline-none hover:bg-[#fafafa] focus-visible:ring-2 focus-visible:ring-black/10 disabled:opacity-40">
                    {t('common.previous')}
                  </button>
                  <span className="text-[13px] text-[#8a8a8a] tabular-nums">
                    {t('settings.scheduledTasks.paginationStatus', {
                      page: localPage,
                      pageCount: workbenchPageCount,
                      total: filteredTasks.length
                    })}
                  </span>
                  <button
                    type="button"
                    disabled={localPage >= workbenchPageCount}
                    onClick={() => setLocalPage((page) => Math.min(workbenchPageCount, page + 1))}
                    className="h-[30px] rounded-[8px] border border-[#e5e5e5] bg-white px-3 text-[13px] text-[#171717] outline-none hover:bg-[#fafafa] focus-visible:ring-2 focus-visible:ring-black/10 disabled:opacity-40">
                    {t('common.next')}
                  </button>
                </div>
              ) : null}
              {pageCount > 1 ? (
                <div className="mt-3 flex items-center justify-center gap-3">
                  <button
                    type="button"
                    disabled={!hasPrev}
                    onClick={prevPage}
                    className="h-[30px] rounded-[8px] border border-[#e5e5e5] bg-white px-3 text-[13px] text-[#171717] outline-none hover:bg-[#fafafa] focus-visible:ring-2 focus-visible:ring-black/10 disabled:opacity-40">
                    {t('common.previous')}
                  </button>
                  <span className="text-[13px] text-[#8a8a8a] tabular-nums">
                    {t('settings.scheduledTasks.paginationStatus', { page, pageCount, total })}
                  </span>
                  <button
                    type="button"
                    disabled={!hasNext}
                    onClick={nextPage}
                    className="h-[30px] rounded-[8px] border border-[#e5e5e5] bg-white px-3 text-[13px] text-[#171717] outline-none hover:bg-[#fafafa] focus-visible:ring-2 focus-visible:ring-black/10 disabled:opacity-40">
                    {t('common.next')}
                  </button>
                </div>
              ) : null}
            </div>
          </Scrollbar>
        </div>

        <TaskFormDialog
          open={createOpen}
          agents={agents}
          tone="workbench"
          onOpenChange={setCreateOpen}
          onCreate={async (agentId, request) => Boolean(await handleCreate(agentId, request))}
        />
        <PageEntry id="workbench.automation" ready={!loading} />
      </div>
    )
  }

  return (
    <div className="flex min-h-0 flex-1 flex-col overflow-hidden">
      <div className="shrink-0 px-6 pt-4">
        <div className="mx-auto flex w-full max-w-3xl items-center justify-between gap-4">
          <div className="flex min-w-0 items-center gap-1.5">
            <SettingTitle className="shrink-0">
              <span>{t('settings.scheduledTasks.title')}</span>
            </SettingTitle>
            <CollapsibleSearchBar
              onSearch={setSearchQuery}
              value={searchQuery}
              placeholder={t('settings.scheduledTasks.searchPlaceholder')}
              tooltip={t('settings.scheduledTasks.search')}
              clearLabel={t('common.clear')}
              maxWidth={220}
              collapsedSize={30}
              style={{ borderRadius: 8 }}
            />
          </div>
          <div className="flex shrink-0 items-center gap-2">
            {tasks.length > 0 && (
              <>
                <Select value={agentFilter} onValueChange={setAgentFilter}>
                  <SelectTrigger
                    className="h-8 min-w-32 bg-transparent"
                    aria-label={t('settings.scheduledTasks.filterAgent')}>
                    <SelectValue />
                  </SelectTrigger>
                  <SelectContent>
                    <SelectGroup>
                      <SelectItem value={ALL_TASKS_FILTER}>{t('settings.scheduledTasks.allAgents')}</SelectItem>
                      {agents.map((agent) => (
                        <SelectItem key={agent.id} value={agent.id}>
                          {agent.name}
                        </SelectItem>
                      ))}
                    </SelectGroup>
                  </SelectContent>
                </Select>
                <Select value={statusFilter} onValueChange={setStatusFilter}>
                  <SelectTrigger
                    className="h-8 min-w-32 bg-transparent"
                    aria-label={t('settings.scheduledTasks.filterStatus')}>
                    <SelectValue />
                  </SelectTrigger>
                  <SelectContent>
                    <SelectGroup>
                      <SelectItem value={ALL_TASKS_FILTER}>{t('settings.scheduledTasks.allStatuses')}</SelectItem>
                      <SelectItem value="active">{t('agent.tasks.status.active')}</SelectItem>
                      <SelectItem value="paused">{t('agent.tasks.status.paused')}</SelectItem>
                      <SelectItem value="completed">{t('agent.tasks.status.completed')}</SelectItem>
                      <SelectItem value="missed">{t('agent.tasks.status.missed')}</SelectItem>
                    </SelectGroup>
                  </SelectContent>
                </Select>
              </>
            )}
            <DropdownMenu>
              <DropdownMenuTrigger asChild>
                <Button type="button" size="sm" className="shrink-0">
                  <Plus size={12} className="lucide-custom" />
                  {t('settings.scheduledTasks.newTask')}
                  <ChevronDown size={12} className="text-primary-foreground" />
                </Button>
              </DropdownMenuTrigger>
              <DropdownMenuContent align="end" className="min-w-40">
                <DropdownMenuGroup>
                  <DropdownMenuItem disabled={agents.length === 0} onSelect={() => setCreateOpen(true)}>
                    <PencilLine />
                    {t('settings.scheduledTasks.manualCreate')}
                  </DropdownMenuItem>
                  <DropdownMenuItem onSelect={createWithAgent}>
                    <Bot />
                    {t('settings.scheduledTasks.agentCreate')}
                  </DropdownMenuItem>
                </DropdownMenuGroup>
              </DropdownMenuContent>
            </DropdownMenu>
          </div>
        </div>
      </div>

      <Scrollbar className="min-h-0 flex-1 px-6 pt-4 pb-3">
        <div className="mx-auto w-full max-w-3xl">
          {tasks.length === 0 ? (
            <EmptyState
              preset={agents.length === 0 ? 'no-agent' : 'no-result'}
              icon={agents.length === 0 ? undefined : CalendarClock}
              title={
                agents.length === 0
                  ? t('settings.scheduledTasks.noAgentsTitle')
                  : t('settings.scheduledTasks.noTasksTitle')
              }
              description={
                agents.length === 0 ? t('settings.scheduledTasks.noAgents') : t('settings.scheduledTasks.noTasks')
              }
              actionLabel={agents.length === 0 ? t('settings.scheduledTasks.agentCreate') : undefined}
              className="py-20"
              onAction={agents.length === 0 ? () => openRoute('/app/agents') : undefined}
            />
          ) : (
            <>
              {filteredTasks.length === 0 && hasActiveFilters ? (
                <EmptyState
                  preset="no-result"
                  title={t('settings.scheduledTasks.noMatchesTitle')}
                  description={t('settings.scheduledTasks.noMatches')}
                  actionLabel={t('settings.scheduledTasks.clearFilters')}
                  className="py-20"
                  onAction={clearFilters}
                />
              ) : (
                <ItemGroup className="gap-3">
                  {filteredTasks.map((task) => (
                    <Item
                      key={task.id}
                      asChild
                      variant="outline"
                      className="rounded-xl border-border bg-card transition-[border-color,box-shadow] hover:border-border-strong hover:bg-card hover:shadow-sm">
                      <Link
                        to="/settings/scheduled-tasks/$taskId"
                        params={{ taskId: task.id }}
                        style={{ backgroundColor: 'var(--settings-group-background, var(--card))' }}>
                        <TaskScheduleStatusIcon status={task.status} />
                        <ItemContent className="min-w-0">
                          <ItemTitle className="max-w-full min-w-0">
                            <span className="truncate">{task.name}</span>
                            <Badge variant="secondary" className="shrink-0">
                              {getTaskStatusLabel(task.status, t)}
                            </Badge>
                          </ItemTitle>
                          <ItemDescription className="truncate text-xs leading-4">
                            {agents.find((agent) => agent.id === task.agentId)?.name ?? task.agentId} ·{' '}
                            {getTriggerSummary(task.trigger, t)}
                          </ItemDescription>
                        </ItemContent>
                        <ItemActions className="ml-auto shrink-0">
                          <TaskCardRunStatus task={task} />
                          <ChevronRight size={16} className="text-foreground-tertiary" />
                        </ItemActions>
                      </Link>
                    </Item>
                  ))}
                </ItemGroup>
              )}
              {pageCount > 1 && (
                <Pagination aria-label={t('settings.scheduledTasks.paginationLabel')} className="pt-4">
                  <PaginationContent>
                    <PaginationItem>
                      <PaginationPrevious
                        href="#"
                        aria-disabled={!hasPrev}
                        aria-label={t('common.previous')}
                        tabIndex={hasPrev ? undefined : -1}
                        className={hasPrev ? undefined : 'pointer-events-none opacity-40'}
                        onClick={(event) => {
                          event.preventDefault()
                          prevPage()
                        }}>
                        {t('common.previous')}
                      </PaginationPrevious>
                    </PaginationItem>
                    <PaginationItem>
                      <SettingDescription className="mt-0 px-2 tabular-nums">
                        {t('settings.scheduledTasks.paginationStatus', { page, pageCount, total })}
                      </SettingDescription>
                    </PaginationItem>
                    <PaginationItem>
                      <PaginationNext
                        href="#"
                        aria-disabled={!hasNext}
                        aria-label={t('common.next')}
                        tabIndex={hasNext ? undefined : -1}
                        className={hasNext ? undefined : 'pointer-events-none opacity-40'}
                        onClick={(event) => {
                          event.preventDefault()
                          nextPage()
                        }}>
                        {t('common.next')}
                      </PaginationNext>
                    </PaginationItem>
                  </PaginationContent>
                </Pagination>
              )}
            </>
          )}
        </div>
      </Scrollbar>

      <TaskFormDialog
        open={createOpen}
        agents={agents}
        onOpenChange={setCreateOpen}
        onCreate={async (agentId, request) => Boolean(await handleCreate(agentId, request))}
      />
    </div>
  )
}

export default TasksSettings
