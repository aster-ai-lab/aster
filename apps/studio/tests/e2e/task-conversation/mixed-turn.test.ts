import { expect, test, type Page } from '@playwright/test'

import { mixedTurn } from '../../../src/renderer/pages/agents/taskConversation/taskConversationCases'

const stamp = '2026-10-05T00:00:00.000Z'

const workspace = {
  id: 'workspace-alpha',
  name: 'Alpha',
  path: mixedTurn.workspacePath,
  type: 'user',
  orderKey: 'a0',
  createdAt: stamp,
  updatedAt: stamp
}

const session = {
  id: 'session-mixed-1',
  agentId: null,
  name: mixedTurn.sessionName,
  isNameManuallyEdited: true,
  workspaceId: workspace.id,
  workspace,
  orderKey: 'a0',
  lastActivityAt: stamp,
  createdAt: stamp,
  updatedAt: stamp
}

const message = {
  id: 'msg-mixed-1',
  sessionId: session.id,
  role: 'assistant',
  data: { parts: mixedTurn.parts },
  status: 'success',
  modelId: null,
  messageSnapshot: null,
  stats: null,
  searchableText: mixedTurn.prose,
  runtimeResumeToken: null,
  createdAt: stamp,
  updatedAt: stamp
}

async function installFixture(page: Page) {
  await page.addInitScript(
    (fixture) => {
      const defaults = {
        'app.onboarding.provider_setup.status': 'skipped',
        'app.privacy.policy_version': '20260820',
        'app.language': 'zh-CN',
        'ui.sidebar_shortcut': [
          {
            type: 'shortcut',
            id: 'sidebar-shortcut:core.app:agents',
            target: { kind: 'resource', locator: { providerId: 'core.app', resourceId: 'agents' } }
          }
        ]
      }
      const host = globalThis as typeof globalThis & { __A3S_PREFERENCE_DEFAULTS__?: Record<string, unknown> }
      host.__A3S_PREFERENCE_DEFAULTS__ = defaults

      const answer = (request: { id?: string; method?: string; path?: string }) => {
        const method = String(request?.method || 'GET').toUpperCase()
        const path = String(request?.path || '').split('?')[0]
        if (method !== 'GET') return null
        const envelope = (data: unknown) => ({
          id: request?.id,
          status: 200,
          data,
          metadata: { timestamp: Date.now() }
        })
        if (path === '/agent-sessions') return envelope({ items: [fixture.session] })
        if (path === '/agent-sessions/latest') return envelope({ session: fixture.session })
        if (path === `/agent-sessions/${fixture.session.id}`) return envelope(fixture.session)
        if (path === `/agent-sessions/${fixture.session.id}/messages`) return envelope({ items: [fixture.message] })
        if (path === '/pins') return envelope([])
        return null
      }

      const wrap = (api: {
        dataApi?: { request?: (request: unknown) => Promise<unknown> }
        __taskConversationWrapped?: boolean
      }) => {
        if (!api || api.__taskConversationWrapped || !api.dataApi?.request) return api
        const request = api.dataApi.request.bind(api.dataApi)
        api.dataApi.request = (input: unknown) => {
          const hit = answer(input as { id?: string; method?: string; path?: string })
          return hit ? Promise.resolve(hit) : request(input)
        }
        api.__taskConversationWrapped = true
        return api
      }

      let current: unknown
      Object.defineProperty(globalThis, 'api', {
        configurable: true,
        enumerable: true,
        get() {
          return current
        },
        set(value) {
          current = wrap(value)
        }
      })
    },
    { session, message }
  )
}

test('the workbench paints one mixed turn and opens its file', async ({ page }) => {
  const errors: string[] = []
  page.on('pageerror', (error) => errors.push(`${error.name}: ${error.message}`))
  await installFixture(page)
  await page.goto('/windows/main/index.html', { waitUntil: 'domcontentloaded' })

  await page.getByRole('button', { name: mixedTurn.sessionName }).click()
  await expect(page.getByText(mixedTurn.prose)).toBeVisible()
  await expect(page.locator('[data-task-tool-name="bash"]')).toContainText(mixedTurn.command)
  await expect(page.getByText(mixedTurn.code)).toBeVisible()
  await expect(page.locator('[data-task-diagram="rendered"], [data-task-diagram="source"]')).toBeVisible()
  await expect(page.getByText(mixedTurn.diffFile).first()).toBeVisible()
  const product = page.locator(`[data-task-product-path="${mixedTurn.productPath}"]`)
  await expect(product).toContainText(mixedTurn.productName)

  await product.click()
  await expect(page.locator(`[data-workspace-file-editor="${mixedTurn.productPath}"]`)).toBeVisible()
  expect(errors, errors.join('\n')).toEqual([])
})
