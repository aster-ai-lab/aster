import { fireEvent, render, screen, waitFor, within } from '@testing-library/react'
import userEvent from '@testing-library/user-event'
import { beforeEach, describe, expect, it } from 'vitest'

import i18n from '@renderer/i18n/resolver'
import { isJobScheduleDue } from '@shared/job/scheduleDue'

import {
  defaultWorkScheduleDraft,
  deleteWorkTask,
  emptyWorkScheduleState,
  isWorkTaskDue,
  loadWorkSchedule,
  pauseWorkTask,
  recordWorkTaskRun,
  resetWorkScheduleMemory,
  saveWorkTask
} from '../workSchedule'
import { WorkScheduledTasks } from '../WorkScheduledTasks'

function renderWork() {
  return render(<WorkScheduledTasks />)
}

function taskRow(name: string) {
  const row = document.querySelector(`[data-ui="workbench.schedule.task"][data-task-name="${name}"]`)
  if (!row) throw new Error(`missing task row ${name}`)
  return within(row as HTMLElement)
}

async function pick(picker: string, option: string) {
  const user = userEvent.setup()
  await user.click(screen.getByRole('combobox', { name: picker }))
  await user.click(screen.getByRole('option', { name: option }))
}

function scheduleTabs() {
  const tabs = document.querySelector('[data-ui="workbench.schedule.tabs"]')
  if (!tabs) throw new Error('missing schedule tabs')
  return within(tabs as HTMLElement)
}

describe('work page scheduled tasks', () => {
  beforeEach(async () => {
    resetWorkScheduleMemory()
    await i18n.changeLanguage('zh-CN')
  })

  it('keeps 定时任务 and 运行记录 on the work page', async () => {
    const user = userEvent.setup()
    renderWork()

    const surface = document.querySelector('[data-ui="workbench.schedule"]')
    expect(surface).toBeTruthy()
    const tabs = document.querySelector('[data-ui="workbench.schedule.tabs"]')
    expect(tabs).toHaveClass('h-[30px]')
    expect(tabs).toHaveAttribute('data-value', 'tasks')
    expect(scheduleTabs().getByRole('button', { name: '定时任务' }).querySelector('svg')).toHaveClass('size-3.5')
    expect(scheduleTabs().getByRole('button', { name: '运行记录' }).querySelector('svg')).toHaveClass('size-3.5')
    expect(screen.getByRole('heading', { name: '自动化任务模版' })).toBeInTheDocument()
    expect(screen.getByText('开启你的第一个自动化任务吧')).toBeInTheDocument()
    await user.click(screen.getByRole('button', { name: '每日 AI 新闻推送' }))
    expect(screen.getByRole('textbox', { name: '名称' })).toHaveValue('每日 AI 新闻推送')
    await user.click(screen.getByRole('button', { name: '取消' }))

    await user.click(scheduleTabs().getByRole('button', { name: '运行记录' }))
    expect(screen.getByText('还没有运行记录')).toBeInTheDocument()

    await user.click(scheduleTabs().getByRole('button', { name: '定时任务' }))
    await user.click(screen.getByRole('button', { name: '添加自动化' }))
    await user.type(screen.getByRole('textbox', { name: '名称' }), '晨报')
    await user.type(screen.getByRole('textbox', { name: '提示词' }), '汇总昨日进展')
    await user.click(screen.getByRole('button', { name: '保存' }))

    expect(taskRow('晨报').getByText('每天 09:00')).toBeInTheDocument()
    expect(taskRow('晨报').getByText('已启用')).toBeInTheDocument()
    const daily = loadWorkSchedule().tasks.find((task) => task.name === '晨报')
    if (!daily) throw new Error('晨报 was not saved')
    expect(document.querySelector('[data-task-name="晨报"]')).toHaveAttribute(
      'data-due',
      isJobScheduleDue(daily.schedule, Date.now()) ? 'true' : 'false'
    )

    await user.click(screen.getByRole('button', { name: '添加自动化' }))
    await user.type(screen.getByRole('textbox', { name: '名称' }), '开会')
    await user.type(screen.getByRole('textbox', { name: '提示词' }), '提醒开会')
    await user.click(screen.getByRole('radio', { name: '单次' }))
    await user.type(screen.getByLabelText('日期'), '2099-01-01T09:00')
    await user.click(screen.getByRole('button', { name: '保存' }))

    expect(taskRow('开会').getByText('单次 · 2099-01-01 09:00')).toBeInTheDocument()
    expect(taskRow('开会').getByText('已启用')).toBeInTheDocument()

    await user.click(screen.getByRole('button', { name: '添加自动化' }))
    await user.type(screen.getByRole('textbox', { name: '名称' }), '空提示')
    await user.click(screen.getByRole('button', { name: '保存' }))
    expect(screen.getByText('请填写提示词')).toBeInTheDocument()
    expect(document.querySelector('[data-task-name="空提示"]')).toBeNull()
    await user.click(screen.getByRole('button', { name: '取消' }))

    await user.click(screen.getByRole('button', { name: '添加自动化' }))
    await user.type(screen.getByRole('textbox', { name: '名称' }), '过去')
    await user.type(screen.getByRole('textbox', { name: '提示词' }), '已经过了')
    await user.click(screen.getByRole('radio', { name: '单次' }))
    await user.type(screen.getByLabelText('日期'), '2000-01-01T09:00')
    await user.click(screen.getByRole('button', { name: '保存' }))
    expect(screen.getByText('请选择将来的时间')).toBeInTheDocument()
    expect(document.querySelector('[data-task-name="过去"]')).toBeNull()
    await user.click(screen.getByRole('button', { name: '取消' }))

    await user.click(taskRow('晨报').getByRole('button', { name: '暂停' }))
    expect(taskRow('晨报').getByText('已暂停')).toBeInTheDocument()
    const paused = loadWorkSchedule().tasks.find((task) => task.name === '晨报')
    if (!paused) throw new Error('晨报 was not paused')
    expect(isJobScheduleDue(paused.schedule, Date.now())).toBe(false)
    expect(document.querySelector('[data-task-name="晨报"]')).toHaveAttribute('data-due', 'false')
    await user.click(taskRow('晨报').getByRole('button', { name: '测试运行' }))
    await user.click(scheduleTabs().getByRole('button', { name: '运行记录' }))
    expect(
      within(document.querySelector('[data-ui="workbench.schedule.run"]') as HTMLElement).getByText('失败')
    ).toBeInTheDocument()
    await user.click(scheduleTabs().getByRole('button', { name: '定时任务' }))
    await user.click(taskRow('晨报').getByRole('button', { name: '恢复' }))
    expect(taskRow('晨报').getByText('已启用')).toBeInTheDocument()
    await user.click(taskRow('开会').getByRole('button', { name: '删除' }))
    expect(document.querySelector('[data-task-name="开会"]')).not.toBeNull()
    expect(taskRow('开会').getByRole('button', { name: '删除' })).toHaveClass('text-[#b42318]')
    await user.click(taskRow('开会').getByRole('button', { name: '删除' }))
    expect(document.querySelector('[data-task-name="开会"]')).toBeNull()

    await user.type(screen.getByRole('searchbox', { name: '搜索定时任务' }), '没有这个任务')
    expect(screen.getByText('没有匹配的定时任务')).toBeInTheDocument()
    expect(document.querySelector('[data-task-name="晨报"]')).toBeNull()
    await user.clear(screen.getByRole('searchbox', { name: '搜索定时任务' }))

    await user.click(taskRow('晨报').getByRole('button', { name: '测试运行' }))
    await user.click(scheduleTabs().getByRole('button', { name: '运行记录' }))
    const runs = [...document.querySelectorAll('[data-ui="workbench.schedule.run"]')]
    expect(runs).toHaveLength(2)
    expect(within(runs[0] as HTMLElement).getByText('晨报')).toBeInTheDocument()
    expect(within(runs[0] as HTMLElement).getByText('成功')).toBeInTheDocument()
    expect(within(runs[1] as HTMLElement).getByText('晨报')).toBeInTheDocument()
    expect(within(runs[1] as HTMLElement).getByText('失败')).toBeInTheDocument()
    expect(screen.queryByText('还没有运行记录')).not.toBeInTheDocument()
  })

  it('creates the other schedules and filters to no match', async () => {
    const user = userEvent.setup()
    renderWork()
    expect(screen.getByText('开启你的第一个自动化任务吧')).toBeInTheDocument()

    await user.click(screen.getByRole('button', { name: '添加自动化' }))
    await user.type(screen.getByRole('textbox', { name: '名称' }), '周会')
    await user.type(screen.getByRole('textbox', { name: '提示词' }), '整理本周')
    await pick('周期', '每周')
    await user.click(screen.getByRole('button', { name: '保存' }))
    expect(taskRow('周会').getByText('每周 周一 · 09:00')).toBeInTheDocument()
    expect(taskRow('周会').getByText('已启用')).toBeInTheDocument()

    await user.click(screen.getByRole('button', { name: '添加自动化' }))
    await user.type(screen.getByRole('textbox', { name: '名称' }), '双周会')
    await user.type(screen.getByRole('textbox', { name: '提示词' }), '双周同步')
    await pick('周期', '双周')
    await user.click(screen.getByRole('button', { name: '保存' }))
    expect(taskRow('双周会').getByText('双周 周一 · 09:00')).toBeInTheDocument()

    await user.click(screen.getByRole('button', { name: '添加自动化' }))
    await user.type(screen.getByRole('textbox', { name: '名称' }), '月报')
    await user.type(screen.getByRole('textbox', { name: '提示词' }), '汇总本月')
    await pick('周期', '每月')
    await user.click(screen.getByRole('button', { name: '保存' }))
    expect(taskRow('月报').getByText('每月 1日 · 09:00')).toBeInTheDocument()

    await user.click(screen.getByRole('button', { name: '添加自动化' }))
    await user.type(screen.getByRole('textbox', { name: '名称' }), '年报')
    await user.type(screen.getByRole('textbox', { name: '提示词' }), '汇总本年')
    await pick('周期', '每年')
    await user.click(screen.getByRole('button', { name: '保存' }))
    expect(taskRow('年报').getByText('每年 1月1日 · 09:00')).toBeInTheDocument()

    await user.click(screen.getByRole('button', { name: '添加自动化' }))
    await user.type(screen.getByRole('textbox', { name: '名称' }), '巡检')
    await user.type(screen.getByRole('textbox', { name: '提示词' }), '检查服务')
    await user.click(screen.getByRole('radio', { name: '按间隔' }))
    const hours = screen.getByRole('spinbutton', { name: '小时' })
    await user.clear(hours)
    await user.type(hours, '2')
    await user.click(screen.getByRole('button', { name: '保存' }))
    expect(taskRow('巡检').getByText('每 2 小时')).toBeInTheDocument()
    expect(taskRow('巡检').getByText('已启用')).toBeInTheDocument()

    await pick('筛选状态', '已暂停')
    expect(screen.getByText('没有匹配的定时任务')).toBeInTheDocument()
    expect(document.querySelector('[data-ui="workbench.schedule.task"]')).toBeNull()
    await pick('筛选状态', '全部状态')
    expect(screen.queryByText('没有匹配的定时任务')).not.toBeInTheDocument()
    expect(taskRow('巡检').getByText('每 2 小时')).toBeInTheDocument()
  })

  it('dismisses the new-task dialog without saving', async () => {
    const user = userEvent.setup()
    renderWork()
    await user.click(screen.getByRole('button', { name: '添加自动化' }))

    const dialog = screen.getByRole('dialog', { name: '新建定时任务' })
    expect(dialog).toHaveAttribute('aria-modal', 'true')
    expect(dialog).toHaveClass('w-[420px]', 'rounded-[12px]', 'p-6')
    expect(screen.getByRole('textbox', { name: '名称' })).toHaveFocus()
    await user.type(screen.getByRole('textbox', { name: '名称' }), '未保存')
    await user.keyboard('{Escape}')
    await waitFor(() => expect(screen.queryByRole('dialog')).not.toBeInTheDocument())
    expect(document.querySelector('[data-task-name="未保存"]')).toBeNull()

    await user.click(screen.getByRole('button', { name: '添加自动化' }))
    await user.click(screen.getByRole('button', { name: '关闭' }))
    await waitFor(() => expect(screen.queryByRole('dialog')).not.toBeInTheDocument())

    await user.click(screen.getByRole('button', { name: '添加自动化' }))
    const scrim = screen.getByRole('dialog').parentElement
    if (!scrim) throw new Error('missing scrim')
    fireEvent.mouseDown(scrim)
    await waitFor(() => expect(screen.queryByRole('dialog')).not.toBeInTheDocument())
    expect(screen.getByText('开启你的第一个自动化任务吧')).toBeInTheDocument()
  })
})

describe('work schedule due decision', () => {
  it('uses the shipped schedule for pause, delete, once, and daily', () => {
    const now = Date.parse('2026-10-02T08:00:00')
    const dailySaved = saveWorkTask(
      emptyWorkScheduleState(),
      { ...defaultWorkScheduleDraft(), name: '晨报', prompt: '汇总昨日进展', time: '09:00' },
      now
    )
    expect(dailySaved.error).toBeNull()
    const onceSaved = saveWorkTask(
      dailySaved.state,
      {
        ...defaultWorkScheduleDraft(),
        name: '开会',
        prompt: '提醒开会',
        mode: 'once',
        onceAt: '2099-01-01T09:00'
      },
      now
    )
    expect(onceSaved.error).toBeNull()
    const daily = onceSaved.state.tasks.find((task) => task.name === '晨报')
    const once = onceSaved.state.tasks.find((task) => task.name === '开会')
    if (!daily || !once || once.frequency.at === null || daily.schedule.nextRun === null) {
      throw new Error('tasks were not saved')
    }

    const paused = pauseWorkTask(onceSaved.state, daily.id, now)
    const pausedDaily = paused.tasks.find((task) => task.id === daily.id)
    if (!pausedDaily) throw new Error('paused task is missing')
    expect(isJobScheduleDue(pausedDaily.schedule, Date.parse('2099-06-01T00:00:00'))).toBe(false)
    expect(isWorkTaskDue(pausedDaily, Date.parse('2099-06-01T00:00:00'))).toBe(false)

    const deleted = deleteWorkTask(paused, once.id)
    const deletedOnce = deleted.tasks.find((task) => task.id === once.id)
    expect(deletedOnce).toBeUndefined()
    const closedOnce = deleted.closed.find((schedule) => schedule.id === once.id)
    if (!closedOnce) throw new Error('deleted schedule is missing')
    expect(isJobScheduleDue(closedOnce, once.frequency.at)).toBe(false)
    expect(isJobScheduleDue(closedOnce, once.frequency.at + 86_400_000)).toBe(false)

    expect(isWorkTaskDue(once, once.frequency.at - 1)).toBe(false)
    expect(isWorkTaskDue(once, once.frequency.at)).toBe(true)
    const ranOnce = recordWorkTaskRun(onceSaved.state, once.id, 'success', once.frequency.at)
    const spent = ranOnce.tasks.find((task) => task.id === once.id)
    if (!spent) throw new Error('spent once task is missing')
    expect(isJobScheduleDue(spent.schedule, once.frequency.at)).toBe(false)
    expect(isJobScheduleDue(spent.schedule, once.frequency.at + 86_400_000)).toBe(false)
    expect(isWorkTaskDue(spent, once.frequency.at)).toBe(false)
    expect(isWorkTaskDue(spent, once.frequency.at + 86_400_000)).toBe(false)

    const first = Date.parse(daily.schedule.nextRun)
    const ranDaily = recordWorkTaskRun(dailySaved.state, daily.id, 'success', first)
    const again = ranDaily.tasks.find((task) => task.id === daily.id)
    if (!again?.schedule.nextRun) throw new Error('daily task has no next run')
    const next = Date.parse(again.schedule.nextRun)
    expect(next).toBeGreaterThan(first)
    expect(isJobScheduleDue(again.schedule, next - 1)).toBe(false)
    expect(isJobScheduleDue(again.schedule, next)).toBe(true)
    expect(isWorkTaskDue(again, next - 1)).toBe(false)
    expect(isWorkTaskDue(again, next)).toBe(true)

    const intervalSaved = saveWorkTask(
      emptyWorkScheduleState(),
      { ...defaultWorkScheduleDraft(), name: '巡检', prompt: '检查服务', mode: 'interval', hours: 1 },
      now
    )
    expect(intervalSaved.error).toBeNull()
    const interval = intervalSaved.state.tasks.find((task) => task.name === '巡检')
    if (!interval || interval.schedule.trigger.kind !== 'interval') throw new Error('interval task was not saved')
    const pausedInterval = pauseWorkTask(intervalSaved.state, interval.id, now).tasks.find(
      (task) => task.id === interval.id
    )
    if (!pausedInterval) throw new Error('paused interval task is missing')
    const createdMs = Date.parse(pausedInterval.schedule.createdAt)
    expect(isJobScheduleDue(pausedInterval.schedule, createdMs + interval.schedule.trigger.ms)).toBe(false)
    expect(isJobScheduleDue(pausedInterval.schedule, createdMs + interval.schedule.trigger.ms * 24)).toBe(false)
  })
})
