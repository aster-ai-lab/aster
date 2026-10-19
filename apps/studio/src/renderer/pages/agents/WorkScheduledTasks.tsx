import { SearchSelect, SegmentedControl } from '@a3s-lab/ui'
import { AlarmClockCheck, CalendarClock, History, Plus, X, type LucideIcon } from 'lucide-react'
import { useEffect, useId, useRef, useState } from 'react'
import { useTranslation } from 'react-i18next'

import { useRestingDialog } from '@renderer/components/restingDialog'
import { cn } from '@renderer/utils/style'

import {
  type WorkPeriod,
  type WorkRunStatus,
  type WorkScheduleDraft,
  type WorkScheduleError,
  type WorkScheduleMode,
  type WorkScheduleState,
  type WorkTask,
  commitWorkSchedule,
  defaultWorkScheduleDraft,
  deleteWorkTask,
  draftFromTask,
  formatWorkSchedule,
  isWorkTaskDue,
  loadWorkSchedule,
  manualWorkTaskRunStatus,
  pauseWorkTask,
  recordWorkTaskRun,
  resumeWorkTask,
  saveWorkTask,
  weekdayLabel
} from './workSchedule'
import { WORK_SCHEDULE_TEMPLATES } from './workScheduleTemplates'

type StatusFilter = 'all' | 'active' | 'paused'
type ScheduleView = 'tasks' | 'runs'

const PERIODS: WorkPeriod[] = ['daily', 'weekly', 'biweekly', 'monthly', 'yearly']
const MODES: WorkScheduleMode[] = ['period', 'interval', 'once']

const VIEW_OPTIONS = [
  { value: 'tasks', labelKey: 'workbench.schedule.view.tasks', icon: CalendarClock },
  { value: 'runs', labelKey: 'workbench.schedule.view.records', icon: History }
] as const satisfies ReadonlyArray<{ value: ScheduleView; labelKey: string; icon: LucideIcon }>

function viewTabLabel(label: string, Icon: LucideIcon) {
  return (
    <span className="inline-flex items-center gap-1.5">
      <Icon className="size-3.5 shrink-0" strokeWidth={1.7} aria-hidden />
      {label}
    </span>
  )
}

const fieldClass =
  'h-9 w-full rounded-[8px] border-0 bg-[#f5f5f5] px-2.5 text-[13px] text-[#171717] outline-none focus-visible:ring-2 focus-visible:ring-black/10'

function segmentClass(selected: boolean, flex = false) {
  return cn(
    'inline-flex h-[22px] shrink-0 items-center justify-center rounded-[6px] px-3 text-[13px] font-medium whitespace-nowrap outline-none focus-visible:ring-2 focus-visible:ring-black/10 focus-visible:ring-inset',
    flex && 'min-w-0 flex-1',
    selected ? 'bg-white text-[#171717] shadow-[0_1px_2px_rgba(0,0,0,0.06)]' : 'text-[#6f6f6f] hover:text-[#171717]'
  )
}

export function WorkScheduledTasks() {
  const { t, i18n } = useTranslation()
  const locale = i18n?.resolvedLanguage || i18n?.language || 'zh-CN'
  const titleId = useId()
  const promptErrorId = useId()
  const onceErrorId = useId()
  const dialogRef = useRef<HTMLFormElement>(null)
  const returnFocusRef = useRef<HTMLElement | null>(null)
  const [state, setState] = useState<WorkScheduleState>(() => loadWorkSchedule())
  const [view, setView] = useState<ScheduleView>('tasks')
  const [query, setQuery] = useState('')
  const [filter, setFilter] = useState<StatusFilter>('all')
  const [editor, setEditor] = useState<{ draft: WorkScheduleDraft; taskId?: string } | null>(null)
  const editorFrame = useRef(editor)
  if (editor) editorFrame.current = editor
  const shownEditor = editor ?? editorFrame.current
  const editorPresence = useRestingDialog(editor !== null)
  const [error, setError] = useState<WorkScheduleError | null>(null)
  const [pendingDeleteId, setPendingDeleteId] = useState<string | null>(null)
  const editorOpen = editor !== null

  const commit = (next: WorkScheduleState) => {
    commitWorkSchedule(next)
    setState(next)
  }

  const visible = state.tasks.filter((task) => {
    const needle = query.trim().toLowerCase()
    const matchesQuery =
      !needle || task.name.toLowerCase().includes(needle) || task.prompt.toLowerCase().includes(needle)
    const matchesFilter = filter === 'all' || (filter === 'active' ? task.schedule.enabled : !task.schedule.enabled)
    return matchesQuery && matchesFilter
  })

  const rememberFocus = () => {
    returnFocusRef.current = document.activeElement instanceof HTMLElement ? document.activeElement : null
  }

  const openCreate = (preset?: Partial<WorkScheduleDraft>) => {
    rememberFocus()
    setError(null)
    setPendingDeleteId(null)
    setEditor({ draft: { ...defaultWorkScheduleDraft(), ...preset } })
  }

  const openEdit = (task: WorkTask) => {
    rememberFocus()
    setError(null)
    setPendingDeleteId(null)
    setEditor({ draft: draftFromTask(task), taskId: task.id })
  }

  const closeEditor = () => {
    setEditor(null)
    setError(null)
  }

  const save = () => {
    if (!editor) return
    const result = saveWorkTask(state, editor.draft, Date.now(), editor.taskId)
    if (result.error) {
      setError(result.error)
      return
    }
    commit(result.state)
    setEditor(null)
    setError(null)
  }

  const run = (task: WorkTask, status: WorkRunStatus) => {
    setPendingDeleteId(null)
    commit(recordWorkTaskRun(state, task.id, status, Date.now()))
  }

  useEffect(() => {
    if (!editorOpen) return
    const node = dialogRef.current
    if (!node) return
    const onKey = (event: KeyboardEvent) => {
      if (event.key === 'Escape') {
        event.preventDefault()
        event.stopPropagation()
        closeEditor()
        return
      }
      if (event.key !== 'Tab') return
      const focusable = [
        ...node.querySelectorAll<HTMLElement>(
          'button, [href], input, select, textarea, [tabindex]:not([tabindex="-1"])'
        )
      ].filter((el) => !el.hasAttribute('disabled'))
      if (focusable.length === 0) return
      const first = focusable[0]
      const last = focusable[focusable.length - 1]
      const active = document.activeElement
      if (!(active instanceof Node) || !node.contains(active)) {
        event.preventDefault()
        ;(event.shiftKey ? last : first).focus()
        return
      }
      if (event.shiftKey && active === first) {
        event.preventDefault()
        last.focus()
      } else if (!event.shiftKey && active === last) {
        event.preventDefault()
        first.focus()
      }
    }
    document.addEventListener('keydown', onKey)
    return () => {
      document.removeEventListener('keydown', onKey)
      const back = returnFocusRef.current
      returnFocusRef.current = null
      back?.focus()
    }
  }, [editorOpen])

  const patchDraft = (patch: Partial<WorkScheduleDraft>) => {
    setEditor((current) => (current ? { ...current, draft: { ...current.draft, ...patch } } : current))
  }

  const addButton = (className: string) => (
    <button
      type="button"
      onClick={() => openCreate()}
      className={cn(
        'inline-flex shrink-0 items-center gap-1 rounded-[8px] bg-[#171717] text-[13px] text-white outline-none focus-visible:ring-2 focus-visible:ring-black/10',
        className
      )}>
      <Plus size={16} className="shrink-0 text-current" strokeWidth={1.7} aria-hidden />
      {t('workbench.schedule.add')}
    </button>
  )

  return (
    <section data-ui="workbench.schedule" className="flex min-h-0 flex-1 flex-col bg-white text-[#171717]">
      <div className="flex shrink-0 items-center justify-between gap-4 px-4 pt-3 pb-2">
        <SegmentedControl<ScheduleView>
          aria-label={t('workbench.schedule.title')}
          data-ui="workbench.schedule.tabs"
          size="sm"
          className="h-[30px]"
          value={view}
          options={VIEW_OPTIONS.map(({ value, labelKey, icon }) => ({
            value,
            label: viewTabLabel(t(labelKey), icon)
          }))}
          onValueChange={(next) => {
            setPendingDeleteId(null)
            setView(next)
          }}
        />
      </div>

      {view === 'tasks' ? (
        <div className="flex min-h-0 flex-1 flex-col overflow-y-auto px-8 pt-2 pb-10">
          {state.tasks.length === 0 ? (
            <div className="flex min-h-[300px] flex-col items-center justify-center px-6">
              <AlarmClockCheck className="size-12 text-[#c5c5c5]" strokeWidth={1.4} aria-hidden />
              <p data-ui="workbench.schedule.empty" className="m-0 mt-4 text-[13px] text-[#8a8a8a]">
                {t('workbench.schedule.empty')}
              </p>
              <div className="mt-5">{addButton('h-9 px-4')}</div>
            </div>
          ) : (
            <>
              <div className="flex items-center gap-2">
                <input
                  type="search"
                  value={query}
                  aria-label={t('workbench.schedule.search')}
                  placeholder={t('workbench.schedule.search')}
                  onChange={(event) => setQuery(event.target.value)}
                  className="h-[30px] min-w-0 flex-1 rounded-[8px] border-0 bg-[#f5f5f5] px-2.5 text-[13px] text-[#171717] outline-none placeholder:text-[#8a8a8a] focus-visible:ring-2 focus-visible:ring-black/10"
                />
                <SearchSelect
                  size="sm"
                  searchable={false}
                  allowClear={false}
                  aria-label={t('workbench.schedule.filter')}
                  value={filter}
                  options={(['all', 'active', 'paused'] as const).map((value) => ({
                    value,
                    label: t(`workbench.schedule.filter.${value}`)
                  }))}
                  onChange={(next) => setFilter(next as StatusFilter)}
                />
                {addButton('h-[30px] px-3')}
              </div>
              {visible.length === 0 ? (
                <p data-ui="workbench.schedule.no-match" className="m-0 pt-16 text-center text-[13px] text-[#8a8a8a]">
                  {t('workbench.schedule.no_match')}
                </p>
              ) : (
                <ul className="m-0 mt-4 flex list-none flex-col gap-0.5 p-0">
                  {visible.map((task) => {
                    const due = isWorkTaskDue(task, Date.now())
                    const confirmingDelete = pendingDeleteId === task.id
                    return (
                      <li
                        key={task.id}
                        data-ui="workbench.schedule.task"
                        data-task-name={task.name}
                        data-due={due ? 'true' : 'false'}
                        className="group flex h-9 items-center gap-2.5 rounded-[8px] px-2.5 hover:bg-[#f7f7f7]">
                        <span className="min-w-0 flex-1 truncate text-[13px] text-[#3f3f3f] group-hover:text-[#171717]">
                          {task.name}
                        </span>
                        <span className="shrink-0 text-[12px] text-[#8a8a8a]">
                          {formatWorkSchedule(task.frequency, locale)}
                        </span>
                        <span className="shrink-0 text-[12px] text-[#8a8a8a]">
                          {t(task.schedule.enabled ? 'workbench.schedule.active' : 'workbench.schedule.paused')}
                        </span>
                        <button
                          type="button"
                          onClick={() => openEdit(task)}
                          className="inline-flex h-6 shrink-0 items-center rounded-[6px] px-1.5 text-[13px] text-[#8a8a8a] hover:bg-[#f2f3f5] hover:text-[#171717]">
                          {t('workbench.schedule.edit')}
                        </button>
                        <button
                          type="button"
                          onClick={() => {
                            setPendingDeleteId(null)
                            commit(
                              task.schedule.enabled
                                ? pauseWorkTask(state, task.id, Date.now())
                                : resumeWorkTask(state, task.id, Date.now())
                            )
                          }}
                          className="inline-flex h-6 shrink-0 items-center rounded-[6px] px-1.5 text-[13px] text-[#8a8a8a] hover:bg-[#f2f3f5] hover:text-[#171717]">
                          {t(task.schedule.enabled ? 'workbench.schedule.pause' : 'workbench.schedule.resume')}
                        </button>
                        <button
                          type="button"
                          onClick={() => run(task, manualWorkTaskRunStatus(task))}
                          className="inline-flex h-6 shrink-0 items-center rounded-[6px] px-1.5 text-[13px] text-[#8a8a8a] hover:bg-[#f2f3f5] hover:text-[#171717]">
                          {t('workbench.schedule.test_run')}
                        </button>
                        <button
                          type="button"
                          onClick={() => {
                            if (!confirmingDelete) {
                              setPendingDeleteId(task.id)
                              return
                            }
                            setPendingDeleteId(null)
                            commit(deleteWorkTask(state, task.id))
                          }}
                          className={cn(
                            'inline-flex h-6 shrink-0 items-center rounded-[6px] px-1.5 text-[13px] hover:bg-[#f2f3f5]',
                            confirmingDelete ? 'text-[#b42318]' : 'text-[#8a8a8a] hover:text-[#171717]'
                          )}>
                          {t('workbench.schedule.delete')}
                        </button>
                      </li>
                    )
                  })}
                </ul>
              )}
            </>
          )}

          <h2
            className={cn(
              'm-0 text-[18px] leading-tight font-semibold text-[#171717]',
              state.tasks.length === 0 ? '' : 'mt-10'
            )}>
            {t('workbench.schedule.templates')}
          </h2>
          <div className="mt-4 grid grid-cols-3 gap-4">
            {WORK_SCHEDULE_TEMPLATES.map((template) => {
              const title = t(`workbench.schedule.template.${template.id}.title`)
              const summary = t(`workbench.schedule.template.${template.id}.summary`)
              const Icon = template.icon
              return (
                <button
                  key={template.id}
                  type="button"
                  aria-label={title}
                  onClick={() => openCreate({ name: title, prompt: summary, ...template.schedule })}
                  className="flex items-center gap-3 rounded-[16px] border border-[#efefef] bg-white px-4 py-3 text-left shadow-[0_4px_16px_rgba(0,0,0,0.04)] outline-none hover:bg-[#fafafa] focus-visible:ring-2 focus-visible:ring-black/10">
                  <Icon className="size-5 shrink-0 text-[#3f3f3f]" strokeWidth={1.7} aria-hidden />
                  <span className="min-w-0">
                    <span className="block truncate text-[14px] font-medium text-[#1a1a1a]">{title}</span>
                    <span className="mt-0.5 block truncate text-[12px] leading-5 text-[#8b8b8b]">{summary}</span>
                  </span>
                </button>
              )
            })}
          </div>
        </div>
      ) : (
        <div className="min-h-0 flex-1 overflow-y-auto px-8 pb-8">
          {state.runs.length === 0 ? (
            <p data-ui="workbench.schedule.runs.empty" className="m-0 pt-16 text-center text-[13px] text-[#8a8a8a]">
              {t('workbench.schedule.records_empty')}
            </p>
          ) : (
            <ul className="m-0 flex list-none flex-col gap-0.5 p-0">
              {state.runs.map((run) => (
                <li
                  key={run.id}
                  data-ui="workbench.schedule.run"
                  className="flex h-9 items-center justify-between gap-2.5 rounded-[8px] px-2.5 text-[13px] hover:bg-[#f7f7f7]">
                  <span className="min-w-0 truncate text-[#3f3f3f]">{run.taskName}</span>
                  <span className={run.status === 'success' ? 'shrink-0 text-[#3f3f3f]' : 'shrink-0 text-[#b42318]'}>
                    {t(run.status === 'success' ? 'workbench.schedule.success' : 'workbench.schedule.failure')}
                  </span>
                </li>
              ))}
            </ul>
          )}
        </div>
      )}

      {editorPresence.present && shownEditor ? (
        <div
          data-state={editorPresence.state}
          className={`a3s-dialog-scrim fixed inset-0 z-[80] flex items-center justify-center bg-black/20 p-6 ${editorPresence.state === 'closed' ? 'pointer-events-none' : ''}`}
          onMouseDown={(event) => {
            if (event.target === event.currentTarget) closeEditor()
          }}>
          <form
            ref={dialogRef}
            role="dialog"
            aria-modal="true"
            aria-labelledby={titleId}
            data-ui="workbench.schedule.editor"
            data-state={editorPresence.state}
            onMouseDown={(event) => event.stopPropagation()}
            onSubmit={(event) => {
              event.preventDefault()
              save()
            }}
            className="a3s-dialog-surface flex max-h-[70vh] w-[420px] flex-col overflow-hidden rounded-[12px] bg-white p-6 shadow-[0_12px_32px_rgba(0,0,0,0.12)]">
            <div className="flex shrink-0 items-center gap-2">
              <h2 id={titleId} className="m-0 min-w-0 flex-1 truncate text-[15px] font-medium text-[#171717]">
                {t(shownEditor.taskId ? 'workbench.schedule.edit' : 'workbench.schedule.create')}
              </h2>
              <button
                type="button"
                aria-label={t('common.close')}
                onClick={closeEditor}
                className="grid size-7 shrink-0 place-items-center rounded-[6px] text-[#3f3f3f] outline-none hover:bg-[#f5f5f5] focus-visible:ring-2 focus-visible:ring-black/10">
                <X size={14} strokeWidth={1.7} />
              </button>
            </div>
            <div className="mt-4 flex min-h-0 flex-1 flex-col gap-3 overflow-y-auto">
              <label className="block">
                <span className="text-[12px] text-[#8a8a8a]">{t('workbench.schedule.name')}</span>
                <input
                  value={shownEditor.draft.name}
                  aria-label={t('workbench.schedule.name')}
                  autoFocus
                  onChange={(event) => patchDraft({ name: event.target.value })}
                  className={cn(fieldClass, 'mt-1.5')}
                />
              </label>
              <label className="block">
                <span className="text-[12px] text-[#8a8a8a]">{t('workbench.schedule.prompt')}</span>
                <textarea
                  value={shownEditor.draft.prompt}
                  aria-label={t('workbench.schedule.prompt')}
                  aria-required="true"
                  aria-invalid={error === 'prompt'}
                  aria-describedby={error === 'prompt' ? promptErrorId : undefined}
                  onChange={(event) => patchDraft({ prompt: event.target.value })}
                  className="mt-1.5 h-24 w-full resize-none rounded-[8px] border-0 bg-[#f5f5f5] px-2.5 py-2 text-[13px] text-[#171717] outline-none focus-visible:ring-2 focus-visible:ring-black/10"
                />
              </label>
              {error === 'prompt' ? (
                <p id={promptErrorId} role="alert" className="m-0 -mt-1 text-[12px] text-[#b42318]">
                  {t('workbench.schedule.prompt_required')}
                </p>
              ) : null}
              <div>
                <span className="text-[12px] text-[#8a8a8a]">{t('workbench.schedule.frequency')}</span>
                <div
                  role="radiogroup"
                  aria-label={t('workbench.schedule.frequency')}
                  className="mt-1.5 flex rounded-[8px] bg-[#f5f5f5] p-1">
                  {MODES.map((mode) => (
                    <button
                      key={mode}
                      type="button"
                      role="radio"
                      aria-checked={shownEditor.draft.mode === mode}
                      onClick={() => patchDraft({ mode })}
                      className={segmentClass(shownEditor.draft.mode === mode, true)}>
                      {t(`workbench.schedule.mode.${mode}`)}
                    </button>
                  ))}
                </div>
              </div>
              {shownEditor.draft.mode === 'period' ? (
                <div className="flex flex-col gap-2">
                  <SearchSelect
                    searchable={false}
                    allowClear={false}
                    aria-label={t('workbench.schedule.mode.period')}
                    value={shownEditor.draft.period}
                    options={PERIODS.map((period) => ({
                      value: period,
                      label: t(`workbench.schedule.period.${period}`)
                    }))}
                    onChange={(next) => patchDraft({ period: next as WorkPeriod })}
                  />
                  {shownEditor.draft.period === 'weekly' ? (
                    <div className="flex flex-wrap gap-2 rounded-[8px] bg-[#f5f5f5] p-1">
                      {[1, 2, 3, 4, 5, 6, 0].map((day) => {
                        const checked = shownEditor.draft.weekdays.includes(day)
                        return (
                          <label
                            key={day}
                            className={cn(
                              'flex h-[22px] items-center gap-1 rounded-[6px] px-2 text-[13px]',
                              checked
                                ? 'bg-white font-medium text-[#171717] shadow-[0_1px_2px_rgba(0,0,0,0.06)]'
                                : 'text-[#6f6f6f]'
                            )}>
                            <input
                              type="checkbox"
                              checked={checked}
                              onChange={() => {
                                const weekdays = checked
                                  ? shownEditor.draft.weekdays.filter((item) => item !== day)
                                  : [...shownEditor.draft.weekdays, day]
                                patchDraft({ weekdays })
                              }}
                            />
                            {weekdayLabel(day, locale)}
                          </label>
                        )
                      })}
                    </div>
                  ) : null}
                  {shownEditor.draft.period === 'biweekly' ? (
                    <SearchSelect
                      searchable={false}
                      allowClear={false}
                      aria-label={t('workbench.schedule.period.biweekly')}
                      value={String(shownEditor.draft.weekday)}
                      options={[1, 2, 3, 4, 5, 6, 0].map((day) => ({
                        value: String(day),
                        label: weekdayLabel(day, locale)
                      }))}
                      onChange={(next) => patchDraft({ weekday: Number(next) })}
                    />
                  ) : null}
                  {shownEditor.draft.period === 'monthly' || shownEditor.draft.period === 'yearly' ? (
                    <input
                      type="number"
                      min={1}
                      max={31}
                      aria-label={t('workbench.schedule.month_day')}
                      value={shownEditor.draft.monthDay}
                      onChange={(event) => patchDraft({ monthDay: Number(event.target.value) })}
                      className={fieldClass}
                    />
                  ) : null}
                  {shownEditor.draft.period === 'yearly' ? (
                    <input
                      type="number"
                      min={1}
                      max={12}
                      aria-label={t('workbench.schedule.month')}
                      value={shownEditor.draft.month}
                      onChange={(event) => patchDraft({ month: Number(event.target.value) })}
                      className={fieldClass}
                    />
                  ) : null}
                  <input
                    type="time"
                    aria-label={t('workbench.schedule.time')}
                    value={shownEditor.draft.time}
                    onChange={(event) => patchDraft({ time: event.target.value })}
                    className={fieldClass}
                  />
                </div>
              ) : null}
              {shownEditor.draft.mode === 'interval' ? (
                <label className="block">
                  <span className="text-[12px] text-[#8a8a8a]">{t('workbench.schedule.hours')}</span>
                  <input
                    type="number"
                    min={1}
                    aria-label={t('workbench.schedule.hours')}
                    value={shownEditor.draft.hours}
                    onChange={(event) => patchDraft({ hours: Number(event.target.value) })}
                    className={cn(fieldClass, 'mt-1.5')}
                  />
                </label>
              ) : null}
              {shownEditor.draft.mode === 'once' ? (
                <label className="block">
                  <span className="text-[12px] text-[#8a8a8a]">{t('workbench.schedule.date')}</span>
                  <input
                    type="datetime-local"
                    aria-label={t('workbench.schedule.date')}
                    aria-invalid={error === 'once'}
                    aria-describedby={error === 'once' ? onceErrorId : undefined}
                    value={shownEditor.draft.onceAt}
                    onChange={(event) => patchDraft({ onceAt: event.target.value })}
                    className={cn(fieldClass, 'mt-1.5')}
                  />
                </label>
              ) : null}
              {error === 'once' ? (
                <p id={onceErrorId} role="alert" className="m-0 -mt-1 text-[12px] text-[#b42318]">
                  {t('workbench.schedule.future')}
                </p>
              ) : null}
            </div>
            <div className="mt-4 flex shrink-0 justify-end gap-2">
              <button
                type="button"
                onClick={closeEditor}
                className="h-9 rounded-[8px] border border-[#e5e5e5] bg-white px-3 text-[13px] text-[#171717] outline-none hover:bg-[#fafafa] focus-visible:ring-2 focus-visible:ring-black/10">
                {t('common.cancel')}
              </button>
              <button
                type="submit"
                className="h-9 rounded-[8px] bg-[#171717] px-3 text-[13px] text-white outline-none focus-visible:ring-2 focus-visible:ring-black/10">
                {t('workbench.schedule.save')}
              </button>
            </div>
          </form>
        </div>
      ) : null}
    </section>
  )
}
