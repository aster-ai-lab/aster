import { beforeEach, describe, expect, it, vi } from 'vitest'

const prepareBuiltinSkill = vi.hoisted(() => vi.fn().mockResolvedValue(undefined))

vi.mock('@renderer/pages/capabilities/capabilitiesData', () => ({
  prepareBuiltinSkill
}))

import { queueScheduledTaskComposer } from '../scheduleTaskComposer'
import { registerWorkbenchComposer, settleWorkbenchComposer } from '../workbenchComposerBridge'

describe('queueScheduledTaskComposer', () => {
  beforeEach(() => {
    prepareBuiltinSkill.mockClear()
    prepareBuiltinSkill.mockResolvedValue(undefined)
  })

  it('prepares the built-in skill and queues its prompt for the home composer', () => {
    const fill = vi.fn()
    const focus = vi.fn()
    queueScheduledTaskComposer((key) => (key === 'settings.scheduledTasks.agentCreatePrompt' ? 'confirm first' : key))
    settleWorkbenchComposer()
    expect(fill).not.toHaveBeenCalled()

    const unregister = registerWorkbenchComposer({ fill, focus })
    expect(prepareBuiltinSkill).toHaveBeenCalledWith('schedule-task')
    expect(fill).toHaveBeenCalledWith('使用技能 schedule-task：confirm first')
    expect(focus).toHaveBeenCalledOnce()
    unregister()
  })

  it('still queues the draft when the skill cannot be prepared', async () => {
    prepareBuiltinSkill.mockRejectedValueOnce(new Error('unavailable'))
    const fill = vi.fn()
    queueScheduledTaskComposer(() => 'confirm first')
    const unregister = registerWorkbenchComposer({ fill, focus() {} })
    await vi.waitFor(() => expect(prepareBuiltinSkill).toHaveBeenCalledOnce())
    expect(fill).toHaveBeenCalledWith('使用技能 schedule-task：confirm first')
    unregister()
  })
})
