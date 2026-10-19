import { createHash } from 'node:crypto'

import { expect, test, type Page } from '@playwright/test'

const stamp = '2026-10-15T00:00:00.000Z'
const sessionId = 'session-ledger-long'
const sessionName = 'ledger-long-task'
const workspacePath = '/tmp/ledger-long'
const spendPath = `${workspacePath}/out/spend.json`
const formSpec = 'root = Input("vendor", "vendor")'
const formRevision = `sha256:${createHash('sha256').update(formSpec).digest('hex')}`
const requestId = 'a'.repeat(64)

const jsonCard = JSON.stringify({
  root: 'spend_card',
  title: 'spend_card',
  state: { labels: { amount: '金额' }, form: { amount: '100' } }
})

const workspace = {
  id: 'workspace-ledger-long',
  name: 'Ledger',
  path: workspacePath,
  type: 'user',
  orderKey: 'a0',
  createdAt: stamp,
  updatedAt: stamp
}

const session = {
  id: sessionId,
  agentId: null,
  name: sessionName,
  isNameManuallyEdited: true,
  workspaceId: workspace.id,
  workspace,
  orderKey: 'a0',
  lastActivityAt: stamp,
  createdAt: stamp,
  updatedAt: stamp
}

function bubble(id: string, role: 'user' | 'assistant', parts: unknown[], status: 'success' | 'pending' = 'success') {
  return {
    id,
    sessionId,
    role,
    data: { parts },
    status,
    modelId: null,
    messageSnapshot: null,
    stats: null,
    searchableText: '',
    runtimeResumeToken: null,
    createdAt: stamp,
    updatedAt: stamp
  }
}

const messages = [
  bubble('msg-user-goal', 'user', [
    {
      type: 'text',
      text: '把 src/ledger.ts 的金额改成按分累加；用已安装的 spend-normalize 处理 data/spend.csv；用已启用的 receipts 连接器为表里的供应商取回执；缺必填项时向我要表单。'
    }
  ]),
  bubble('msg-code', 'assistant', [
    {
      type: 'tool-read',
      toolCallId: 'lt-read',
      state: 'output-available',
      input: { filePath: 'src/ledger.ts' },
      output: 'export function sum(rows) { return rows.map((row) => row.amount).join("") }'
    },
    {
      type: 'tool-edit',
      toolCallId: 'lt-edit',
      state: 'output-available',
      input: {
        path: 'src/ledger.ts',
        old_string: 'return rows.map((row) => row.amount).join("")',
        new_string: 'return rows.reduce((total, row) => total + row.amountCents, 0)'
      },
      output: 'ok'
    },
    {
      type: 'tool-bash',
      toolCallId: 'lt-check',
      state: 'output-error',
      input: { command: 'npm test' },
      errorText: 'npm test failed'
    }
  ]),
  bubble('msg-skill', 'assistant', [
    {
      type: 'dynamic-tool',
      toolName: 'spend-normalize',
      toolCallId: 'lt-skill',
      state: 'output-available',
      input: { skill: 'spend-normalize', path: 'data/spend.csv' },
      output: { path: spendPath }
    },
    {
      type: 'dynamic-tool',
      toolName: 'Write',
      toolCallId: 'lt-spend-file',
      state: 'output-available',
      input: {
        file_path: spendPath,
        content: '{"date":"2026-01-01","vendor":"north","amountCents":100}'
      },
      output: 'ok'
    }
  ]),
  bubble('msg-connector', 'assistant', [
    {
      type: 'dynamic-tool',
      toolName: 'receipts',
      toolCallId: 'lt-receipts',
      state: 'output-available',
      input: { vendor: 'north' },
      output: { receiptId: 'rcpt-north-1' }
    }
  ]),
  bubble('msg-json', 'assistant', [
    { type: 'text', text: '```json\n{"kind":"ordinary-json-kept"}\n```' },
    { type: 'text', text: ['```json', jsonCard, '```'].join('\n') },
    { type: 'text', text: '```json-render\n{"kind":"legacy-fence-kept"}\n```' },
    { type: 'text', text: '```aix-ui\n{"kind":"aix-ui-kept"}\n```' }
  ]),
  bubble('msg-catalog', 'assistant', [
    {
      type: 'text',
      text: [
        '```openui',
        'city = Input("city", "城市", "上海", "text")',
        'agree = Checkbox("agree", "同意", true)',
        'root = Card("trip_card", [city, agree])',
        '```'
      ].join('\n')
    },
    {
      type: 'data-form',
      data: {
        id: 'b'.repeat(64),
        format: 'openui',
        state: 'form-requested',
        spec: 'root = Input("ghost", "ghost")'
      }
    }
  ]),
  bubble(
    'msg-form',
    'assistant',
    [
      {
        type: 'dynamic-tool',
        toolName: 'ask_user_form',
        toolCallId: 'lt-ask',
        state: 'input-available',
        input: { form: formSpec }
      },
      {
        type: 'data-form',
        data: {
          id: requestId,
          format: 'openui',
          state: 'form-requested',
          spec: formSpec,
          binding: {
            protocolVersion: 1,
            requestId,
            revision: formRevision,
            owner: {
              sessionId,
              executionId: 'model',
              attemptId: 1,
              anchorMessageId: 'msg-form',
              acpSessionId: 'acp',
              promptId: 'prompt'
            }
          }
        }
      }
    ],
    'pending'
  )
]

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
        if (path === `/agent-sessions/${fixture.session.id}/messages`) return envelope({ items: fixture.messages })
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
    { session, messages }
  )
}

test('one ledger session keeps code, skill, connector, and form turns', async ({ page }) => {
  test.setTimeout(180000)
  const errors: string[] = []
  page.on('pageerror', (error) => errors.push(`${error.name}: ${error.message}`))
  await installFixture(page)
  await page.goto('/windows/main/index.html', { waitUntil: 'domcontentloaded' })
  await page.getByRole('button', { name: sessionName }).click()

  const edit = page.locator('[data-task-tool-name="edit"]')
  await expect(edit).toContainText('ledger.ts')
  await expect(edit).toHaveAttribute('data-task-tool-state', 'success')
  await edit.locator('summary').click()
  await expect(edit.locator('[data-kind="addition"] code')).toContainText('amountCents')

  const failed = page.locator('[data-task-tool-name="bash"][data-task-tool-state="error"]')
  await expect(failed).toBeVisible()
  await expect(edit).toHaveAttribute('data-task-tool-state', 'success')

  await expect(page.locator('[data-task-tool-name="spend-normalize"]')).toHaveAttribute(
    'data-task-tool-state',
    'success'
  )
  const product = page.locator(`[data-task-product-path="${spendPath}"]`)
  await expect(product).toBeVisible()

  const receipts = page.locator('[data-task-tool-name="receipts"]')
  await expect(receipts).toHaveAttribute('data-task-tool-state', 'success')
  await receipts.locator('summary').click()
  await expect(receipts).toContainText('rcpt-north-1')

  await expect(page.getByRole('heading', { name: 'spend_card' })).toBeVisible()
  await expect(page.getByRole('textbox', { name: '金额' })).toHaveValue('100')
  await expect(page.getByRole('textbox', { name: 'amount' })).toHaveCount(0)
  await expect(page.getByRole('button', { name: '提交' })).toBeDisabled()
  await expect(page.getByText('这里不能继续这个任务。').first()).toBeVisible()
  await expect(page.getByText('legacy-fence-kept')).toBeVisible()
  await expect(page.getByText('ordinary-json-kept')).toBeVisible()
  await expect(page.getByText('aix-ui-kept')).toBeVisible()
  await expect(page.getByRole('heading', { name: 'trip_card' })).toBeVisible()
  await expect(page.getByRole('textbox', { name: '城市' })).toHaveValue('上海')
  await expect(page.getByRole('checkbox', { name: '同意' })).toBeChecked()
  await expect(page.getByText('root = Input("ghost", "ghost")')).toBeVisible()
  await expect(page.getByRole('textbox', { name: 'ghost' })).toHaveCount(0)
  await expect(page.getByRole('button', { name: '提交' })).toBeDisabled()
  await expect(page.getByRole('textbox', { name: 'vendor' })).toBeVisible()
  expect(errors, errors.join('\n')).toEqual([])
  console.log(
    [
      `label ${await page.getByRole('textbox', { name: '金额' }).getAttribute('aria-label')}`,
      `value ${await page.getByRole('textbox', { name: '金额' }).inputValue()}`,
      `disabled 提交 ${await page.getByRole('button', { name: '提交' }).isDisabled()}`,
      await page.getByText('这里不能继续这个任务。').first().innerText(),
      await page.getByText('legacy-fence-kept').innerText(),
      await page.getByText('ordinary-json-kept').innerText(),
      `label ${await page.getByRole('textbox', { name: '城市' }).getAttribute('aria-label')}`,
      `checkbox ${await page.getByRole('checkbox', { name: '同意' }).getAttribute('aria-label')}`,
      `ghost ${await page.getByRole('textbox', { name: 'ghost' }).count()}`
    ].join('\n')
  )
})
