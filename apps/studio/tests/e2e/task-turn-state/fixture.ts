import type { Page } from '@playwright/test'

import {
  installAgentEventsBridge,
  installWorkbench,
  openHome,
  type OfficeMessage,
  type WorkbenchInstall
} from '../workbench-office/fixture'
import { UI_PROJECT, type WorkbenchUiSession } from '../workbench-office/uiFixture'

const STAMP = '2026-10-18T00:00:00.000Z'
export const HISTORY_ID = 'session-history-turn'
export const HELD_ID = 'session-held-turn'

export function heldParts(approval = false): NonNullable<OfficeMessage['parts']> {
  if (approval) {
    return [
      {
        type: 'tool-bash',
        toolCallId: 'held-approve',
        state: 'pending',
        input: { command: 'rm -rf /tmp/held' }
      }
    ]
  }
  return [
    {
      type: 'tool-bash',
      toolCallId: 'held-done',
      state: 'output-available',
      input: { command: 'pwd' },
      output: '/tmp/a3s-workbench-office'
    },
    { type: 'text', text: '环境就绪，开始构建工作簿。' }
  ]
}

export async function installTurnState(
  page: Page,
  options: {
    sessions?: WorkbenchUiSession[]
    officeSessions?: WorkbenchInstall['sessions']
    heldStream?: boolean
  } = {}
) {
  await page.addInitScript(() => {
    localStorage.removeItem('workbench.sidebarCollapsed')
  })
  await installAgentEventsBridge(page)

  const sessions =
    options.sessions ??
    ([
      {
        id: HISTORY_ID,
        name: '历史任务',
        place: 'inbox' as const,
        updatedAt: STAMP
      }
    ] satisfies WorkbenchUiSession[])

  await installWorkbench(page, {
    withAgent: true,
    collab: true,
    workspaceFiles: true,
    heldStream: options.heldStream === true,
    sessions: options.officeSessions,
    ui: {
      withAgent: true,
      projects: [UI_PROJECT],
      sessions
    }
  })
}

export async function openWorkbenchHome(page: Page) {
  await openHome(page)
  await page.locator('[data-ui="workbench.home"]').waitFor({ state: 'visible', timeout: 30000 })
}

export async function pushTopicStatus(
  page: Page,
  sessionId: string,
  status: 'streaming' | 'done' | 'aborted',
  anchorMessageId?: string
) {
  await page.evaluate(
    ({ sessionId, status, anchorMessageId }) => {
      const push = (globalThis as { __E2E_SSE_PUSH__?: (channel: string, payload: unknown) => void }).__E2E_SSE_PUSH__
      if (!push) throw new Error('sse push missing')
      const topicId = `agent-session:${sessionId}`
      if (status === 'streaming') {
        push('topic.status', {
          topicId,
          status: {
            status: 'streaming',
            activeExecutions: anchorMessageId
              ? [{ executionId: 'held-exec', attemptId: 1, anchorMessageId }]
              : []
          }
        })
        return
      }
      push('topic.status', {
        topicId,
        status: {
          status,
          activeExecutions: [],
          lastCompletedAt: new Date().toISOString()
        }
      })
    },
    { sessionId, status, anchorMessageId }
  )
}
