import { describe, expect, it } from 'vitest'

import { reviewComposerDraft } from '../reviewHandoff'
import {
  type WorkbenchComposerFill,
  queueWorkbenchComposer,
  registerWorkbenchComposer,
  settleWorkbenchComposer
} from '../workbenchComposerBridge'

describe('workbench composer bridge', () => {
  it('holds a skill draft until the home composer registers', () => {
    const fill = viFill()
    queueWorkbenchComposer('使用技能 PPT 演示文稿：生成一份客户汇报 PPT')
    settleWorkbenchComposer()
    expect(fill.texts).toEqual([])
    const unregister = registerWorkbenchComposer({ fill: fill.fill, focus: fill.focus })
    expect(fill.texts).toEqual(['使用技能 PPT 演示文稿：生成一份客户汇报 PPT'])
    expect(fill.focused).toBe(1)
    settleWorkbenchComposer()
    expect(fill.texts).toHaveLength(1)
    unregister()
  })

  it('delivers a review file chip without dropping its path', () => {
    const fill = viFill()
    const draft = reviewComposerDraft('审查这个文件', '/tmp/设备采购合同_测试样本.docx', '设备采购合同_测试样本.docx')
    queueWorkbenchComposer(draft)
    const unregister = registerWorkbenchComposer({ fill: fill.fill, focus: fill.focus })
    expect(fill.texts).toEqual([draft])
    unregister()
  })
})

function viFill() {
  const texts: WorkbenchComposerFill[] = []
  let focused = 0
  return {
    texts,
    get focused() {
      return focused
    },
    fill(next: WorkbenchComposerFill) {
      texts.push(next)
    },
    focus() {
      focused += 1
    }
  }
}
