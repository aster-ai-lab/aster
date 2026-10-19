import type { ComposerSerializedToken } from '@renderer/components/composer/tokens'

export type WorkbenchComposerFill =
  | string
  | {
      text: string
      tokens: readonly ComposerSerializedToken[]
    }

type WorkbenchComposerBridge = {
  fill: (next: WorkbenchComposerFill) => void
  focus: () => void
}

let bridge: WorkbenchComposerBridge | null = null
let pending: WorkbenchComposerFill | null = null
let sessionDraftWritesHeld = false

/** 新建任务清空输入时，作曲家仍绑着上一个任务。这段时间不写那个任务的草稿。 */
export function holdSessionDraftWrites() {
  sessionDraftWritesHeld = true
}

export function releaseSessionDraftWrites() {
  sessionDraftWritesHeld = false
}

/** 首页上的文字属于下一次发送，不写入当前绑定的另一个任务。 */
export function sessionDraftWriteAllowed(homeActive: boolean) {
  return !sessionDraftWritesHeld && !homeActive
}

function paintPending() {
  if (!bridge || pending == null) return
  const next = pending
  pending = null
  bridge.fill(next)
  bridge.focus()
}

/** Remember a draft until the home composer is on screen, then insert it. */
export function queueWorkbenchComposer(next: WorkbenchComposerFill) {
  pending = next
}

/** Insert a queued draft once the home composer is active. */
export function settleWorkbenchComposer() {
  paintPending()
}

export function registerWorkbenchComposer(next: WorkbenchComposerBridge): () => void {
  bridge = next
  paintPending()
  return () => {
    if (bridge === next) bridge = null
  }
}

export function fillWorkbenchComposer(next: WorkbenchComposerFill) {
  if (!bridge) return
  bridge.fill(next)
  bridge.focus()
}

export function clearWorkbenchComposer() {
  if (!bridge) return
  bridge.fill('')
  bridge.focus()
}

export function focusWorkbenchComposer() {
  bridge?.focus()
}
