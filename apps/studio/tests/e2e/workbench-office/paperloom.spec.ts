import { createHash } from 'node:crypto'
import { readFileSync } from 'node:fs'

import { expect, test, type Page } from '@playwright/test'
import { PDFDocument } from 'pdf-lib'

import { makeWorkbenchProof } from '../../../src/shared/types/__tests__/workbenchProof.fixture'
import { installWorkbench, ipcCount, workAgent } from './fixture'
import { UI_MEMBER_ID } from './uiFixture'

const editor = '[data-ui~="workbench.composer"] .composer-tiptap'
const send = '[data-ui~="workbench.composer"] [data-ui="chat.composer.action.send"]'
const pdf = readFileSync('tests/fixtures/paperloom/two-pages.pdf')
const documentId = createHash('sha256').update(pdf).digest('hex')
const workspaceRoot = '/tmp/a3s-workbench-office/paperloom-workspace'
const sourcePath = `${workspaceRoot}/paper.pdf`
const artifactBytes = Buffer.concat([pdf, Buffer.from('\n% rendered artifact fixture\n')])
const artifactDigest = createHash('sha256').update(artifactBytes).digest('hex')
const paper = {
  document_id: documentId,
  title: 'A paper about reliable agents',
  source_filename: 'paper.pdf',
  page_count: 2,
  bytes: pdf.length,
  active_job_id: null,
  reading_status: 'unread',
  added_at: '2026-10-16T00:00:00Z',
  updated_at: '2026-10-16T00:00:00Z',
  tags: []
}

async function setup(
  page: Page,
  options: {
    connected?: boolean
    processingError?: boolean
    processing?: boolean
    artifacts?: boolean
    unverifiedArtifacts?: boolean
    provenanceFailureOnce?: boolean
    provenanceTampered?: boolean
    offlineAfterSave?: boolean
    noModel?: boolean
    noAgent?: boolean
    withReviewAgent?: boolean
    task?: boolean
  } = {}
) {
  const sessions = options.task ? [{ id: 'paperloom-task', name: '论文精读任务', messages: [] }] : []
  await installWorkbench(page, {
    sessions,
    withAgent: !options.noAgent,
    collab: true,
    publishReview: true,
    invoke: 'record',
    preferences: {
      'chat.default_model_id': options.noModel ? '' : 'a3s::gpt-work',
      'app.workspace.path': workspaceRoot
    },
    ui: { withAgent: !options.noAgent, sessions: sessions.map((session) => ({ ...session, place: 'inbox' })) }
  })
  if (options.noModel)
    await page.route('**/v1/api/org/models', (route) =>
      route.fulfill({ json: { success: true, data: { items: [], defaultModel: null } } })
    )
  if (options.noAgent)
    await page.route('**/api/agents', (route) => route.fulfill({ json: { items: [], total: 0, page: 1 } }))
  await page.route('**/api/agents/agent-work', (route) => route.fulfill({ json: workAgent }))
  await page.route('**/api/catalog-agents*', (route) => route.fulfill({ json: { items: [] } }))
  const plugins = [
    {
      slug: 'a3s-paperloom',
      name: '论文精读',
      packageId: 'use/a3s/paperloom',
      version: '1.0.1',
      digest: 'b'.repeat(64)
    },
    ...(options.withReviewAgent
      ? [
          {
            slug: 'a3s-review',
            name: '合同审查',
            packageId: 'use/a3s/review',
            version: '1.3.0',
            digest: 'd'.repeat(64)
          }
        ]
      : [])
  ]
  await page.route('**/api/aix/views', (route) =>
    route.fulfill({
      json: {
        protocolVersion: 1,
        revision: `sha256:${'c'.repeat(64)}`,
        packages: plugins.map((plugin) => ({
          slug: plugin.slug,
          packageId: plugin.packageId,
          packageDigest: `sha256:${plugin.digest}`,
          generation: 3,
          revision: `sha256:${'a'.repeat(64)}`,
          containers: [],
          views: []
        }))
      }
    })
  )
  await page.route('**/api/capabilities?*', (route) =>
    route.fulfill({
      json: {
        items: plugins.map((plugin) => ({
          slug: plugin.slug,
          kind: 'plugin',
          name: plugin.name,
          category: 'other',
          installed: true,
          description: `通过 @${plugin.name} 使用智能体`,
          version: plugin.version,
          surfaces: ['skill', 'agent'],
          agentRuntime: {
            runtime: 'edge',
            harness: 'builtin',
            resources: { cpuMillis: 500, memoryBytes: 268435456 }
          },
          install: {
            type: 'plugin',
            packageType: 'aix',
            available: true,
            packageId: plugin.packageId,
            digest: `sha256:${plugin.digest}`,
            size: 1024
          }
        })),
        total: plugins.length,
        page: 1,
        pageSize: 100,
        categories: []
      }
    })
  )
  let connected = options.connected ?? true
  let status = 'unread'
  let processingStatus = options.artifacts ? 'succeeded' : ''
  let provenance: ReturnType<typeof makeWorkbenchProof> | null = null
  let provenanceAttempts = 0
  const requests: { path: string; method: string; body: string | null }[] = []
  await page.route('**/api/paperloom/**', async (route) => {
    const request = route.request()
    const url = new URL(request.url())
    expect(url.searchParams.getAll('studio_member')).toHaveLength(1)
    url.searchParams.delete('studio_member')
    const path = url.pathname.slice('/api/paperloom/'.length)
    requests.push({ path, method: request.method(), body: request.postData() })
    const json = (data: unknown) => route.fulfill({ json: { code: 0, message: 'ok', data } })
    if (path === 'connection') {
      if (request.method() === 'PUT') connected = true
      return route.fulfill({
        json: {
          endpoint: '',
          apiKeyConfigured: connected,
          revision: connected ? 'saved-connection' : ''
        }
      })
    }
    if (path === 'provenance' && request.method() === 'GET') return json(provenance ? [provenance] : [])
    if (path === 'provenance' && request.method() === 'POST') {
      provenanceAttempts += 1
      if (options.provenanceFailureOnce && provenanceAttempts === 1)
        return route.fulfill({ status: 503, json: { error: 'provenance_unavailable' } })
      const body = request.postDataJSON()
      const base = makeWorkbenchProof()
      provenance = makeWorkbenchProof({
        sessionId: 'paperloom-workspace-fixture',
        messageId: '33333333-3333-4333-8333-333333333333',
        runId: body.jobId,
        model: 'unreported',
        userText: '',
        sourceText: '',
        policy: { reviewMode: false, artifactTrace: true },
        review: { status: 'off', findings: [], reviewer: 'independent-model' },
        tools: [
          {
            id: '33333333-3333-4333-8333-333333333333',
            name: 'paperloom.verify-saved-copy',
            state: 'success',
            input: JSON.stringify(body),
            output: body.savedPath
          }
        ],
        artifacts: [
          {
            ...base.artifacts[0],
            id: '33333333-3333-4333-8333-333333333333',
            name: 'paper-translated.pdf',
            path: body.savedPath,
            bytes: artifactBytes.length,
            mediaType: 'application/pdf',
            contentDigest: `sha256:${artifactDigest}`,
            toolName: 'paperloom.verify-saved-copy',
            toolCallId: '33333333-3333-4333-8333-333333333333',
            executionId: body.jobId,
            inputDigests: [`sha256:${body.sourceDigest}`],
            origin: {
              kind: 'paperloom-service',
              memberId: UI_MEMBER_ID,
              workspaceRoot: body.root,
              sourcePath: body.sourcePath,
              sourceRevision: body.sourceRevision,
              sourceDigest: `sha256:${body.sourceDigest}`,
              savedRevision: '34'.repeat(32),
              jobId: body.jobId,
              jobStatus: 'succeeded',
              attempt: body.attempt,
              artifactKey: body.artifactKey,
              manifestDigest: `sha256:${'a'.repeat(64)}`,
              connectionRevision: '56'.repeat(32),
              packageDigest: `sha256:${'b'.repeat(64)}`
            }
          }
        ]
      })
      if (options.offlineAfterSave) connected = false
      return json(provenance)
    }
    if (path.startsWith('provenance/') && provenance) {
      if (path.endsWith('/history')) return json([provenance])
      if (path.endsWith('/snapshot'))
        return json({
          id: provenance.artifacts[0].id,
          name: 'paper-translated.pdf',
          mediaType: 'application/pdf',
          contentDigest: `sha256:${artifactDigest}`,
          data: (options.provenanceTampered ? Buffer.from('tampered') : artifactBytes).toString('base64')
        })
      return json(provenance)
    }
    if (path === 'uploads' && options.processingError)
      return route.fulfill({ status: 502, json: { error: 'paperloom_unavailable' } })
    if (path === 'uploads') return json({ document_id: documentId, upload_id: 'source-index' })
    if (path === `documents/${documentId}` && request.method() === 'GET')
      return json({ ...paper, reading_status: status, active_job_id: options.artifacts ? 'gateway-job' : null })
    if (path === 'documents') return json({ documents: [{ ...paper, reading_status: status }], total: 1 })
    if (path === `documents/${documentId}/source.pdf`)
      return route.fulfill({ contentType: 'application/pdf', body: pdf })
    if (path === `documents/${documentId}/ocr` && options.processing) {
      processingStatus = 'running'
      return json({ job_id: 'gateway-job', status: 'queued' })
    }
    if (path === `jobs/gateway-job/cancel`) {
      processingStatus = 'canceled'
      return json({ accepted: true })
    }
    if (path === `jobs/gateway-job/artifacts-manifest`)
      return json({
        job_id: 'gateway-job',
        items: options.artifacts
          ? [
              {
                artifact_key: 'translated-pdf',
                ready: true,
                file_name: 'paper-translated.pdf',
                artifact_kind: 'file',
                checksum: `sha256:${artifactDigest}`,
                attempt: 1
              },
              ...(options.unverifiedArtifacts
                ? [
                    {
                      artifact_key: 'job-root',
                      ready: true,
                      file_name: 'internal-job-directory',
                      artifact_kind: 'dir',
                      checksum: null,
                      attempt: 1
                    },
                    {
                      artifact_key: 'unverified-file',
                      ready: true,
                      file_name: 'unverified.pdf',
                      artifact_kind: 'file',
                      checksum: null,
                      attempt: 1
                    }
                  ]
                : [])
            ]
          : []
      })
    if (path === 'jobs/gateway-job/artifacts/translated-pdf')
      return route.fulfill({ contentType: 'application/pdf', body: artifactBytes })
    if (path === `documents/${documentId}/jobs`)
      return json({
        items: processingStatus
          ? [
              {
                job_id: 'gateway-job',
                workflow: 'ocr',
                status: processingStatus,
                attempt: 1,
                stages: {
                  ocr: {
                    state: processingStatus === 'running' ? 'in_progress' : 'failed',
                    progress: { current: 1, total: 2, percent: 50 }
                  },
                  translation: { state: 'skipped', progress: {} },
                  render: { state: 'skipped', progress: {} }
                }
              }
            ]
          : []
      })
    if (path === `documents/${documentId}` && request.method() === 'PATCH') {
      status = request.postDataJSON().reading_status
      return json({ ...paper, reading_status: status, active_job_id: options.artifacts ? 'gateway-job' : null })
    }
    if (path === 'providers/ocr')
      return json([
        {
          key: 'paddle',
          display_name: 'PaddleOCR',
          credential: { field: 'paddle_token', required_for: ['local_upload'] },
          options: {
            paddle_model: {
              type: 'string',
              default: 'PaddleOCR-VL-1.6',
              choices: ['PaddleOCR-VL-1.6', 'PaddleOCR-VL'],
              required: false
            }
          },
          capabilities: { supports_local_file_upload: true }
        }
      ])
    return route.fulfill({ status: 404, json: { error: 'unexpected_request', path } })
  })
  await page.route('**/api/workspace-search?**', (route) =>
    route.fulfill({ json: { entries: [{ path: sourcePath, isDirectory: false }] } })
  )
  await page.route('**/api/workspace-file/snapshot?**', (route) => {
    const url = new URL(route.request().url())
    if (url.searchParams.get('root') !== workspaceRoot || url.searchParams.get('path') !== sourcePath)
      return route.fulfill({ status: 403 })
    return route.fulfill({
      contentType: 'application/pdf',
      body: pdf,
      headers: {
        etag: '"' + '12'.repeat(32) + '"',
        'content-length': String(pdf.length),
        'access-control-expose-headers': 'etag'
      }
    })
  })
  await page.goto('/windows/main/index.html')
  if (options.task) await page.getByRole('button', { name: '论文精读任务', exact: true }).click()
  await expect(page.locator(editor)).toBeVisible()
  await page.locator(editor).fill('@论文精读')
  await page.getByTestId('quick-panel').locator('[data-id="workbench-agent:a3s-paperloom"]').click()
  return requests
}

const reader = (page: Page) => page.getByRole('region', { name: '论文精读', exact: true })
const pageRail = (page: Page) => reader(page).getByRole('navigation', { name: '页面', exact: true })
const pageImage = (page: Page) => reader(page).getByRole('region', { name: '原文', exact: true }).locator('canvas')

async function shownPageText(page: Page): Promise<string> {
  return ((await pageImage(page).getAttribute('aria-label')) ?? '').trim()
}

async function chooseWorkspacePaper(page: Page) {
  await page.locator('[data-paperloom-workspace]').getByRole('button', { name: '工作区文件', exact: true }).click()
  await page.getByRole('option', { name: 'paper.pdf', exact: true }).click()
  await page.getByRole('button', { name: '添加 1 项', exact: true }).click()
  await expect(pageImage(page)).toHaveAccessibleName(/Page 1/)
  await expect(page.locator('[data-paperloom-pdf] canvas')).toBeVisible()
}

async function openWorkspacePaper(page: Page) {
  await page.locator(send).click()
  await chooseWorkspacePaper(page)
}

test('keeps the installed agent mention callable after navigating through plugins', async ({ page }) => {
  await setup(page, { noModel: true, noAgent: true })
  await page.getByRole('button', { name: '插件', exact: true }).click()
  await expect(page.getByText('论文精读', { exact: true }).first()).toBeVisible()
  await page.getByRole('button', { name: '工作', exact: true }).click()
  await page.locator(editor).fill('')
  await page.locator(editor).pressSequentially('@')
  const candidate = page.getByTestId('quick-panel').locator('[data-id="workbench-agent:a3s-paperloom"]')
  await expect(candidate).toBeVisible()
  await candidate.click()
  await expect(page.locator(send)).toBeEnabled()
  await openWorkspacePaper(page)
  expect(await ipcCount(page, 'ai.stream.open')).toBe(0)
})

test('dismisses the file picker and restores input while exit animations are paused', async ({ page }) => {
  await setup(page)
  await page.addStyleTag({
    content: '[data-selector-shell-content][data-state="closed"] { animation-play-state: paused !important; }'
  })
  await page.getByRole('button', { name: '工作区文件', exact: true }).click()
  await page.getByRole('option', { name: 'paper.pdf', exact: true }).click()
  await page.getByRole('button', { name: '添加 1 项', exact: true }).click()
  await expect(page.getByPlaceholder('搜索文件', { exact: true })).toBeHidden({ timeout: 1000 })
  await expect(page.getByRole('button', { name: '工作区文件', exact: true })).toBeFocused()
  await page.locator(editor).click()
  await page.locator(editor).press('End')
  await page.keyboard.insertText(' @')
  await expect(page.getByTestId('quick-panel').locator('[data-id="workbench-agent:a3s-paperloom"]')).toBeVisible()
})

test('shows the background workspace picker without waiting for animation frames', async ({ page }) => {
  await setup(page)
  await page.evaluate(() => Object.defineProperty(document, 'hasFocus', { configurable: true, value: () => false }))
  await page.addStyleTag({
    content: '[data-selector-shell-content], [data-selector-shell-panel] { animation-play-state: paused !important; }'
  })
  await page.getByRole('button', { name: '工作区文件', exact: true }).click()
  const popup = page.locator('[data-selector-shell-content][data-state="open"]')
  await expect(popup).toHaveCSS('opacity', '1')
  await expect(popup).toBeInViewport({ ratio: 0.95 })
  await page.getByRole('option', { name: 'paper.pdf', exact: true }).click()
  await page.getByRole('button', { name: '添加 1 项', exact: true }).click()
  await expect(popup).toBeHidden()
  await expect(page.locator(editor)).toContainText('paper.pdf')
})

test('renders the source without animation frames before consuming its draft', async ({ page }) => {
  await setup(page, { noModel: true, connected: false })
  const document = await PDFDocument.load(pdf)
  const firstPage = document.getPage(0)
  const size = firstPage.getSize()
  const appearance = document.context.register(
    document.context.flateStream('q 1 0 0 rg 0 0 80 20 re f Q', {
      Type: 'XObject',
      Subtype: 'Form',
      BBox: [0, 0, 80, 20],
      Resources: {}
    })
  )
  // This annotation is visible on screen but has no print flag.
  firstPage.node.addAnnot(
    document.context.register(
      document.context.obj({
        Type: 'Annot',
        Subtype: 'Stamp',
        Rect: [50, 100, 130, 120],
        F: 0,
        AP: { N: appearance }
      })
    )
  )
  const annotated = Buffer.from(await document.save())
  await page.route('**/api/workspace-file/snapshot?**', (route) =>
    route.fulfill({
      contentType: 'application/pdf',
      body: annotated,
      headers: { etag: '"' + '34'.repeat(32) + '"', 'access-control-expose-headers': 'etag' }
    })
  )
  await page.getByRole('button', { name: '工作区文件', exact: true }).click()
  await page.getByRole('option', { name: 'paper.pdf', exact: true }).click()
  await page.getByRole('button', { name: '添加 1 项', exact: true }).click()
  const originalFrame = await page.evaluateHandle(() => window.requestAnimationFrame)
  await page.locator(send).evaluate((button) =>
    button.addEventListener(
      'click',
      () => {
        window.requestAnimationFrame = () => 0
      },
      { once: true, capture: true }
    )
  )
  try {
    await page.locator(send).click()
    await expect(page.locator('[data-paperloom-pdf] canvas')).toBeVisible()
    await expect(pageImage(page)).toHaveAccessibleName(/Page 1/)
    await expect(page.locator('[data-paperloom-pdf] [role=status]')).toHaveCount(0)
    const stamp = await page.locator('[data-paperloom-pdf] canvas').evaluate((canvas: HTMLCanvasElement, size) => {
      const context = canvas.getContext('2d')
      if (!context) throw new Error('PDF canvas has no 2D context')
      return Array.from(
        context.getImageData(
          Math.floor((90 / size.width) * canvas.width),
          Math.floor(((size.height - 110) / size.height) * canvas.height),
          1,
          1
        ).data
      )
    }, size)
    expect(stamp).toEqual([255, 0, 0, 255])
    const stampName = await shownPageText(page)
    expect(stampName).toContain('PaperLoom regression - Page 1')
    console.log(`stamp-pixel=[${stamp.join(', ')}]`)
    console.log(`stamp-name=${stampName}`)
  } finally {
    await page.evaluate((original) => {
      window.requestAnimationFrame = original
    }, originalFrame)
    await originalFrame.dispose()
  }
  await page.getByRole('button', { name: '返回工作页', exact: true }).click()
  await expect(page.locator(editor)).toHaveText('')
  expect(await ipcCount(page, 'ai.stream.open')).toBe(0)
})

test('shows background reading pickers when their animations cannot advance', async ({ page }) => {
  await setup(page, { noModel: true, connected: false })
  await openWorkspacePaper(page)
  await page.evaluate(() => Object.defineProperty(document, 'hasFocus', { configurable: true, value: () => false }))
  await page.addStyleTag({ content: '.ui-search-select__panel { animation-play-state: paused !important; }' })
  await page.getByRole('button', { name: '缩放', exact: true }).click()
  await expect(page.locator('.ui-search-select__panel')).toHaveCSS('opacity', '1')
  await page.getByRole('option', { name: '150%', exact: true }).click()
  await expect(page.getByRole('button', { name: '缩放', exact: true })).toContainText('150%')
  await expect(page.locator('[data-paperloom-pdf] canvas')).toBeVisible()
  await expect(pageImage(page)).toHaveAccessibleName(/Page 1/)
  const leftInset = await page.locator('[data-paperloom-pdf]').evaluate((host) => {
    const canvas = host.querySelector('canvas')
    if (!canvas) throw new Error('PDF canvas is missing')
    return canvas.getBoundingClientRect().left - host.getBoundingClientRect().left
  })
  expect(leftInset).toBeGreaterThanOrEqual(0)
  const rightOverflow = await page.locator('[data-paperloom-pdf]').evaluate((host) => {
    const canvas = host.querySelector('canvas')
    if (!canvas) throw new Error('PDF canvas is missing')
    host.scrollLeft = host.scrollWidth
    const overflow = canvas.getBoundingClientRect().right - host.getBoundingClientRect().right
    host.scrollLeft = 0
    return overflow
  })
  expect(rightOverflow).toBeLessThanOrEqual(1)
  await page.getByRole('button', { name: '阅读视图', exact: true }).click()
  await expect(page.locator('.ui-search-select__panel')).toHaveCSS('opacity', '1')
  await page.getByRole('option', { name: '原文', exact: true }).click()
  await expect(page.locator('.ui-search-select__panel')).toHaveCount(0)
})

for (const search of [false, true]) {
  test(`retries a failed workspace ${search ? 'search' : 'directory'} without reporting an empty result`, async ({
    page
  }) => {
    await setup(page)
    let failed = true
    await page.route('**/api/workspace-search?**', (route) => {
      const query = new URL(route.request().url()).searchParams.get('q')
      return failed && Boolean(query) === search
        ? route.fulfill({ status: 503, json: { error: 'unavailable' } })
        : route.fulfill({ json: { entries: [{ path: sourcePath, isDirectory: false }] } })
    })
    await page.getByRole('button', { name: '工作区文件', exact: true }).click()
    if (search) {
      await expect(page.getByRole('option', { name: 'paper.pdf', exact: true })).toBeVisible()
      await page.getByPlaceholder('搜索文件', { exact: true }).fill('paper')
    }
    await expect(page.getByRole('alert')).toHaveText('无法加载文件，请重试。')
    await expect(page.getByText('没有匹配的文件', { exact: true })).toHaveCount(0)
    await page.screenshot({
      animations: 'disabled',
      path: test.info().outputPath(`workspace-picker-${search ? 'search' : 'directory'}-error.png`)
    })
    failed = false
    await page.getByRole('button', { name: '重试', exact: true }).click()
    await expect(page.getByPlaceholder('搜索文件', { exact: true })).toBeFocused()
    await expect(page.getByRole('option', { name: 'paper.pdf', exact: true })).toBeVisible()
    await expect(page.getByRole('alert')).toHaveCount(0)
    if (search) await expect(page.getByPlaceholder('搜索文件', { exact: true })).toHaveValue('paper')
    await page.getByRole('option', { name: 'paper.pdf', exact: true }).click()
    await page.getByRole('button', { name: '添加 1 项', exact: true }).click()
    await expect(page.locator(editor)).toContainText('paper.pdf')
  })
}

test('retries a refused subdirectory without hiding the other workspace files', async ({ page }) => {
  await setup(page)
  const folder = `${workspaceRoot}/chapters`
  let failed = true
  await page.route('**/api/workspace-search?**', (route) => {
    const root = new URL(route.request().url()).searchParams.get('root')
    if (root === folder)
      return failed
        ? route.fulfill({ status: 403, json: { error: 'not_found' } })
        : route.fulfill({ json: { entries: [{ path: `${folder}/chapter.pdf`, isDirectory: false }] } })
    return route.fulfill({
      json: {
        entries: [
          { path: folder, isDirectory: true },
          { path: sourcePath, isDirectory: false }
        ]
      }
    })
  })
  await page.getByRole('button', { name: '工作区文件', exact: true }).click()
  await page.getByRole('button', { name: '展开 chapters', exact: true }).click()
  await expect(page.getByRole('alert')).toContainText('chapters')
  await expect(page.getByRole('option', { name: 'paper.pdf', exact: true })).toBeVisible()
  await page.screenshot({
    animations: 'disabled',
    path: test.info().outputPath('workspace-picker-subdirectory-error.png')
  })
  failed = false
  await page.getByRole('button', { name: '重试 chapters', exact: true }).click()
  await expect(page.locator(`[data-workspace-file="${folder}"]`)).toBeFocused()
  await expect(page.getByRole('alert')).toHaveCount(0)
  await page.getByRole('option', { name: 'chapter.pdf', exact: true }).click()
  await page.getByRole('button', { name: '添加 1 项', exact: true }).click()
  await expect(page.locator(editor)).toContainText('chapter.pdf')
})

test('ignores the old directory response after closing and reopening the workspace picker', async ({ page }) => {
  await setup(page)
  let release!: () => Promise<void>
  let reads = 0
  await page.route('**/api/workspace-search?**', async (route) => {
    reads += 1
    if (reads === 1) {
      release = () => route.fulfill({ json: { entries: [{ path: `${workspaceRoot}/stale.pdf`, isDirectory: false }] } })
      return
    }
    await route.fulfill({ json: { entries: [{ path: `${workspaceRoot}/current.pdf`, isDirectory: false }] } })
  })
  await page.getByRole('button', { name: '工作区文件', exact: true }).click()
  await expect.poll(() => reads).toBe(1)
  await page.getByRole('button', { name: '取消', exact: true }).click()
  await page.getByRole('button', { name: '工作区文件', exact: true }).click()
  await expect(page.getByRole('option', { name: 'current.pdf', exact: true })).toBeVisible()
  const lateResponse = page.waitForResponse((response) => new URL(response.url()).pathname === '/api/workspace-search')
  await release()
  await (await lateResponse).finished()
  await page.evaluate(
    () => new Promise<void>((resolve) => requestAnimationFrame(() => requestAnimationFrame(() => resolve())))
  )
  await expect(page.getByRole('option', { name: 'current.pdf', exact: true })).toBeVisible()
  await expect(page.getByRole('option', { name: 'stale.pdf', exact: true })).toHaveCount(0)
})

test('uses normal send and only consumes the draft after the original PDF opens', async ({ page }) => {
  await setup(page)
  const initialSessions = await ipcCount(page, 'ai.agent.session.reuse_or_create')
  await expect(page.getByRole('button', { name: '打开精读', exact: true })).toHaveCount(0)
  await expect(page.locator(send)).toBeEnabled()
  await page.locator(send).click()
  await expect(page.getByRole('heading', { name: '选择工作区中的 PDF', exact: true })).toBeVisible()
  await expect(page.locator('[data-paperloom-library], [data-paperloom-workspace] input[type=file]')).toHaveCount(0)
  await page.getByRole('button', { name: '返回工作页', exact: true }).click()
  await expect(page.locator(editor)).toContainText('论文精读')
  await page.locator(send).click()
  await chooseWorkspacePaper(page)
  await page.getByRole('button', { name: '返回工作页', exact: true }).click()
  await expect(page.locator(editor)).toHaveText('')
  const calls = await page.evaluate(() => (globalThis as { __E2E_INVOKE__?: string[] }).__E2E_INVOKE__ ?? [])
  expect(calls).not.toContain('open_workbench_applet_window')
  expect(calls).not.toContain('open_aix_applet_window')
  expect(await ipcCount(page, 'ai.agent.session.reuse_or_create')).toBe(initialSessions)
})

test('hands a question into the paper workspace without creating a duplicate model turn', async ({ page }) => {
  await setup(page)
  const initialSessions = await ipcCount(page, 'ai.agent.session.reuse_or_create')
  await page.locator(editor).press('End')
  await page.keyboard.insertText('解释这篇论文的核心贡献')
  await page.locator('[data-ui="workbench.composer"] [data-ui="chat.composer.action.send"]').click()
  await chooseWorkspacePaper(page)
  await expect(page.getByRole('textbox', { name: '论文问题', exact: true })).toHaveValue('解释这篇论文的核心贡献')
  expect(await ipcCount(page, 'ai.stream.open')).toBe(0)
  expect(await ipcCount(page, 'ai.agent.session.reuse_or_create')).toBe(initialSessions)
})

for (const noAgent of [false, true]) {
  test(`sends a PDF and paper token without a chat model (${noAgent ? 'fallback' : 'resolved'} composer)`, async ({
    page
  }) => {
    await setup(page, { noModel: true, noAgent, connected: false })
    const initialSessions = await ipcCount(page, 'ai.agent.session.reuse_or_create')
    await page.getByRole('button', { name: '工作区文件', exact: true }).click()
    await page.getByRole('option', { name: 'paper.pdf', exact: true }).click()
    await page.getByRole('button', { name: '添加 1 项', exact: true }).click()
    const composer = page.locator('[data-ui~="workbench.composer"]')
    await expect(composer.locator('.composer-tiptap')).toContainText('paper.pdf')
    await expect(page.getByRole('button', { name: '工作区文件', exact: true }).locator('span.truncate')).toHaveText(
      '工作区文件'
    )
    await expect(composer).not.toContainText('使用当前工作区的 PDF 阅读、处理或提问。')
    await expect(page.locator(send)).toBeEnabled()
    if (noAgent)
      await page.screenshot({
        path: test.info().outputPath('browser-unified-send-no-model.png')
      })
    await page.locator(send).click()
    await expect(pageImage(page)).toHaveAccessibleName(/Page 1/)
    await page.getByRole('button', { name: '论文问答', exact: true }).click()
    await expect(page.getByRole('textbox', { name: '论文问题', exact: true })).toHaveValue('')
    await page.getByRole('button', { name: '返回工作页', exact: true }).click()
    await expect(page.locator(editor)).toHaveText('')
    await page.locator(editor).fill('普通聊天仍然需要模型')
    await expect(page.locator(send)).toBeDisabled()
    expect(await ipcCount(page, 'ai.stream.open')).toBe(0)
    expect(await ipcCount(page, 'ai.agent.session.reuse_or_create')).toBe(initialSessions)
  })
}

test('preserves the complete draft when no workspace host accepts send and retries normally', async ({ page }) => {
  await page.addInitScript(() => {
    window.addEventListener('studio:paperloom-open', (event) => event.stopImmediatePropagation(), {
      once: true
    })
  })
  await setup(page)
  await page.locator(editor).press('End')
  await page.keyboard.insertText('解释方法')
  const draft = await page.locator(editor).innerText()
  await page.locator(send).click()
  await expect(page.getByText('未能打开论文精读，输入内容已保留，请重试。', { exact: true })).toBeVisible()
  await expect.poll(() => page.locator(editor).innerText()).toBe(draft)
  await page.locator(send).click()
  await chooseWorkspacePaper(page)
  await expect(page.getByRole('textbox', { name: '论文问题', exact: true })).toHaveValue('解释方法')
})

test('opens paper reading from an existing task composer without a chat model', async ({ page }) => {
  await setup(page, { task: true, noModel: true, connected: false })
  const initialSessions = await ipcCount(page, 'ai.agent.session.reuse_or_create')
  await expect(page.locator(send)).toBeEnabled()
  await page.locator(send).click()
  await expect(page.getByRole('heading', { name: '选择工作区中的 PDF', exact: true })).toBeVisible()
  await expect(page.locator('[data-paperloom-workspace]')).toBeVisible()
  expect(await ipcCount(page, 'ai.stream.open')).toBe(0)
  expect(await ipcCount(page, 'ai.agent.session.reuse_or_create')).toBe(initialSessions)
})

test('preserves the file and question when the selected workspace PDF cannot be read', async ({ page }) => {
  await setup(page)
  await page.getByRole('button', { name: '工作区文件', exact: true }).click()
  await page.getByRole('option', { name: 'paper.pdf', exact: true }).click()
  await page.getByRole('button', { name: '添加 1 项', exact: true }).click()
  await page.locator(editor).press('End')
  await page.keyboard.insertText('解释失败后仍保留的问题')
  const draft = await page.locator(editor).innerText()
  await page.route('**/api/workspace-file/snapshot?**', (route) => route.fulfill({ status: 404 }))
  await page.locator(send).click()
  await expect(page.locator('[data-paperloom-workspace] [role=alert]')).toBeVisible()
  await expect(page.locator('[data-paperloom-pdf]')).toHaveCount(0)
  await page.getByRole('button', { name: '返回工作页', exact: true }).click()
  await expect.poll(() => page.locator(editor).innerText()).toBe(draft)
})

for (const noAgent of [false, true]) {
  test(`switches the selected agent without losing the PDF or question (${noAgent ? 'fallback' : 'resolved'} composer)`, async ({
    page
  }) => {
    await setup(page, { noModel: true, noAgent, connected: false, withReviewAgent: true })
    const initialSessions = await ipcCount(page, 'ai.agent.session.reuse_or_create')
    await page.locator(editor).press('End')
    await page.keyboard.insertText(' @合同审查')
    await page.getByTestId('quick-panel').locator('[data-id="workbench-agent:a3s-review"]').click()
    await expect(page.locator(editor)).not.toContainText('论文精读')
    await expect(page.getByRole('region', { name: '合同审查文件', exact: true })).toBeVisible()
    await page.getByRole('button', { name: '工作区文件', exact: true }).click()
    await page.getByRole('option', { name: 'paper.pdf', exact: true }).click()
    await page.getByRole('button', { name: '添加 1 项', exact: true }).click()
    await page.locator(editor).press('End')
    await page.keyboard.insertText('解释核心贡献')
    await expect(page.locator(send)).toBeDisabled()
    await expect(page.locator('[data-paperloom-workspace]')).toHaveCount(0)
    expect(await ipcCount(page, 'ai.stream.open')).toBe(0)
    expect(await ipcCount(page, 'ai.agent.session.reuse_or_create')).toBe(initialSessions)
    await page.locator(editor).press('End')
    await page.keyboard.insertText(' @论文精读')
    await page.getByTestId('quick-panel').locator('[data-id="workbench-agent:a3s-paperloom"]').click()
    await expect(page.locator(editor)).not.toContainText('合同审查')
    await expect(page.getByRole('region', { name: '合同审查文件', exact: true })).toHaveCount(0)
    await expect(page.locator(editor)).toContainText('paper.pdf')
    await expect(page.locator(editor)).toContainText('解释核心贡献')
    await expect(page.locator(send)).toBeEnabled()
    await page.locator(send).click()
    await expect(pageImage(page)).toHaveAccessibleName(/Page 1/)
    await expect(page.getByRole('textbox', { name: '论文问题', exact: true })).toHaveValue('解释核心贡献')
    expect(await ipcCount(page, 'ai.stream.open')).toBe(0)
    expect(await ipcCount(page, 'ai.agent.session.reuse_or_create')).toBe(initialSessions)
    await page.getByRole('button', { name: '返回工作页', exact: true }).click()
    await expect(page.locator(editor)).toHaveText('')
  })

  test(`preserves the draft when a PDF header hides a corrupt document (${noAgent ? 'fallback' : 'resolved'} composer)`, async ({
    page
  }) => {
    const requests = await setup(page, { noModel: true, noAgent })
    await page.getByRole('button', { name: '工作区文件', exact: true }).click()
    await page.getByRole('option', { name: 'paper.pdf', exact: true }).click()
    await page.getByRole('button', { name: '添加 1 项', exact: true }).click()
    await page.locator(editor).press('End')
    await page.keyboard.insertText('解释损坏后仍保留的问题')
    const draft = await page.locator(editor).innerText()
    const brokenSnapshot = (route: import('@playwright/test').Route) =>
      route.fulfill({
        contentType: 'application/pdf',
        body: '%PDF-1.7\nnot a valid PDF document',
        headers: { etag: '"' + '56'.repeat(32) + '"', 'access-control-expose-headers': 'etag' }
      })
    await page.route('**/api/workspace-file/snapshot?**', brokenSnapshot)
    await page.locator(send).click()
    await expect(page.locator('[data-paperloom-pdf] [role=alert]')).toHaveText('PDF 加载失败，请刷新后重试。')
    expect(requests.filter((request) => request.path.startsWith('uploads'))).toHaveLength(0)
    await page.getByRole('button', { name: '返回工作页', exact: true }).click()
    await expect.poll(() => page.locator(editor).innerText()).toBe(draft)
    await page.unroute('**/api/workspace-file/snapshot?**', brokenSnapshot)
    await page.locator(send).click()
    await expect(pageImage(page)).toHaveAccessibleName(/Page 1/)
    await expect(page.getByRole('textbox', { name: '论文问题', exact: true })).toHaveValue('解释损坏后仍保留的问题')
    await page.getByRole('button', { name: '返回工作页', exact: true }).click()
    await expect(page.locator(editor)).toHaveText('')
    expect(await ipcCount(page, 'ai.stream.open')).toBe(0)
  })

  test(`refuses a stale paper token before reading or consuming its PDF (${noAgent ? 'fallback' : 'resolved'} composer)`, async ({
    page
  }) => {
    await setup(page, { noModel: true, noAgent })
    const initialSessions = await ipcCount(page, 'ai.agent.session.reuse_or_create')
    await page.getByRole('button', { name: '工作区文件', exact: true }).click()
    await page.getByRole('option', { name: 'paper.pdf', exact: true }).click()
    await page.getByRole('button', { name: '添加 1 项', exact: true }).click()
    await page.locator(editor).press('End')
    await page.keyboard.insertText('说明这篇论文')
    const draft = await page.locator(editor).innerText()
    let reads = 0
    page.on('request', (request) => {
      if (new URL(request.url()).pathname === '/api/workspace-file/snapshot') reads += 1
    })
    await page.route('**/api/paperloom/connection?**', (route) =>
      route.fulfill({ status: 404, json: { error: 'not_found' } })
    )
    await page.locator(send).click()
    await expect(page.getByRole('alert')).toContainText('请先在插件页面安装论文精读。')
    await expect(page.locator('[data-paperloom-pdf]')).toHaveCount(0)
    expect(reads).toBe(0)
    await page.getByRole('button', { name: '返回工作页', exact: true }).click()
    await expect.poll(() => page.locator(editor).innerText()).toBe(draft)
    expect(await ipcCount(page, 'ai.stream.open')).toBe(0)
    expect(await ipcCount(page, 'ai.agent.session.reuse_or_create')).toBe(initialSessions)
  })
}

test('keeps workspace reading available when the processing service fails', async ({ page }) => {
  await setup(page, { processingError: true })
  await openWorkspacePaper(page)
  await expect(page.getByRole('alert')).toContainText('暂时无法处理这份 PDF')
  await expect(page.getByRole('button', { name: '处理论文', exact: true })).toBeDisabled()
  await expect(page.locator('[data-paperloom-library]')).toHaveCount(0)
})

test('retries a failed Markdown read through the visible refresh action', async ({ page }) => {
  await setup(page, { artifacts: true })
  await page.route(
    (url) => url.pathname === `/api/paperloom/documents/${documentId}/jobs`,
    (route) =>
      route.fulfill({
        json: {
          code: 0,
          data: {
            items: [
              {
                job_id: 'gateway-job',
                workflow: 'ocr',
                status: 'succeeded',
                attempt: 1,
                markdown_ready: true,
                stages: {
                  ocr: { state: 'completed', progress: {} },
                  translation: { state: 'skipped', progress: {} },
                  render: { state: 'skipped', progress: {} }
                }
              }
            ]
          }
        }
      })
  )
  let reads = 0
  await page.route(
    (url) => url.pathname === '/api/paperloom/jobs/gateway-job/markdown',
    (route) => {
      reads += 1
      if (reads === 1) return route.fulfill({ status: 503, json: { error: 'temporarily_unavailable' } })
      return new URL(route.request().url()).searchParams.get('raw') === 'true'
        ? route.fulfill({ contentType: 'text/markdown', body: '# Recovered paper text' })
        : route.fulfill({ json: { code: 0, data: { job_id: 'gateway-job', content: '# Recovered paper text' } } })
    }
  )
  await openWorkspacePaper(page)
  const workspace = page.locator('[data-paperloom-workspace]')
  await workspace.getByRole('button', { name: '阅读视图', exact: true }).click()
  await page.getByRole('option', { name: 'Markdown', exact: true }).click()
  await expect(workspace.getByRole('alert')).toContainText('Markdown 加载失败，请刷新重试。')
  await expect(workspace.locator('pre')).not.toContainText('正在加载')
  await workspace.getByRole('button', { name: '刷新', exact: true }).click()
  await expect(workspace.locator('pre')).toHaveText('# Recovered paper text')
  await expect(workspace.getByRole('alert')).toHaveCount(0)
  expect(reads).toBe(2)
})

for (const width of [1280, 520]) {
  test(`uses Studio controls with visible nested pickers and fixed dialog actions at ${width}px`, async ({ page }) => {
    await page.setViewportSize({ width, height: 720 })
    await setup(page)
    if (width < 640) await page.getByRole('button', { name: '收起侧边栏', exact: true }).click()
    await openWorkspacePaper(page)
    await expect(page.locator('[data-paperloom-pdf] canvas')).toBeVisible()
    await page.getByRole('button', { name: '处理论文', exact: true }).click()
    const dialog = page.getByRole('dialog', { name: '处理论文', exact: true })
    await expect(dialog).toContainText('读取 PDF 文本层并写成 Markdown')
    await expect(dialog).toBeInViewport({ ratio: 0.95 })
    expect((await dialog.boundingBox())?.x).toBeGreaterThanOrEqual(60)
    await expect(dialog.locator('[data-paperloom-ocr-source]')).toHaveText('本机文本层')
    await expect(dialog.getByRole('textbox', { name: '服务地址', exact: true })).toHaveCount(0)
    await expect(dialog.getByRole('button', { name: '工作流', exact: true })).toHaveCount(0)
    await expect(dialog.getByRole('button', { name: '开始处理', exact: true })).toBeEnabled()
    await expect(dialog.getByRole('button', { name: '关闭', exact: true })).toHaveCount(0)
    await expect(dialog.getByRole('heading', { name: '处理论文', exact: true })).toBeInViewport()
    const fields = dialog.locator('[data-paperloom-process-fields]')
    await expect(dialog.getByRole('heading', { name: '处理论文', exact: true })).toBeInViewport()
    await expect(dialog.getByRole('button', { name: '开始处理', exact: true })).toBeInViewport()
    const fieldsBox = await fields.boundingBox()
    const footerBox = await dialog.locator('[data-slot="dialog-footer"]').boundingBox()
    expect(footerBox?.y).toBeGreaterThanOrEqual((fieldsBox?.y ?? 0) + (fieldsBox?.height ?? 0))
    expect(await dialog.evaluate((node) => node.scrollTop)).toBe(0)
    await page.screenshot({ path: test.info().outputPath(`paperloom-process-${width}.png`) })
    await dialog.getByRole('button', { name: '取消', exact: true }).click()
    await expect(dialog).toHaveCount(0)
    await page.getByRole('button', { name: '阅读状态', exact: true }).click()
    await page.getByRole('option', { name: '已读', exact: true }).click()
    await expect(page.getByRole('button', { name: '阅读状态', exact: true })).toContainText('已读')
    await page.getByRole('button', { name: '下一页', exact: true }).click()
    await expect(page.getByRole('spinbutton', { name: '页码', exact: true })).toHaveValue('2')
    await expect(pageImage(page)).toHaveAccessibleName(/Page 2/)
    await expect(page.locator('[data-paperloom-pdf] canvas')).toBeVisible()
    await page.screenshot({ path: test.info().outputPath(`paperloom-reader-${width}.png`) })
    expect(await page.locator('[data-paperloom-workspace] select, [data-paperloom-workspace] datalist').count()).toBe(0)
  })
}

test('submits OS gateway OCR without a user credential and shows cancellable page progress', async ({ page }) => {
  const requests = await setup(page, { processing: true })
  await openWorkspacePaper(page)
  await page.getByRole('button', { name: '处理论文', exact: true }).click()
  const dialog = page.getByRole('dialog', { name: '处理论文', exact: true })
  await dialog.getByRole('button', { name: '开始处理', exact: true }).click()
  await expect(dialog).toHaveCount(0)
  await expect(page.getByRole('progressbar', { name: 'OCR', exact: true })).toHaveAttribute('value', '50')
  const request = requests.find((item) => item.path === `documents/${documentId}/ocr`)
  expect(JSON.parse(request?.body ?? '{}')).toEqual({ workflow: 'ocr', source: {}, ocr: { provider: 'a3s_gateway' } })
  expect(requests.filter((item) => item.path === 'providers/ocr')).toHaveLength(0)
  await page.getByRole('button', { name: '停止', exact: true }).click()
  await expect(page.getByRole('button', { name: '恢复任务', exact: true })).toBeVisible()
  expect(requests.filter((item) => item.path === 'jobs/gateway-job/cancel')).toHaveLength(1)
})

test('reads the workspace PDF while the local processor reports itself unavailable', async ({ page }) => {
  const requests = await setup(page, { connected: false })
  await openWorkspacePaper(page)
  await expect(page.getByText('处理服务尚未就绪，仍可阅读工作区中的原始 PDF。', { exact: true })).toBeVisible()
  await expect(page.getByRole('button', { name: '处理论文', exact: true })).toBeDisabled()
  await expect(page.getByRole('button', { name: '连接服务', exact: true })).toHaveCount(0)
  await expect(page.getByRole('textbox', { name: '服务地址', exact: true })).toHaveCount(0)
  expect(requests.filter((request) => request.path === 'uploads')).toHaveLength(0)
  expect(requests.some((request) => request.method === 'PUT')).toBe(false)
  await expect(pageImage(page)).toHaveAccessibleName(/Page 1/)
})

test('carries the composer workspace file selection straight into reading', async ({ page }) => {
  const requests = await setup(page)
  await page.getByRole('button', { name: '工作区文件', exact: true }).click()
  await page.getByRole('option', { name: 'paper.pdf', exact: true }).click()
  await page.getByRole('button', { name: '添加 1 项', exact: true }).click()
  await page.locator(send).click()
  await expect(pageImage(page)).toHaveAccessibleName(/Page 1/)
  await expect(page.getByRole('heading', { name: '选择工作区中的 PDF', exact: true })).toHaveCount(0)
  await expect(page.getByRole('button', { name: '处理论文', exact: true })).toBeEnabled()
  expect(requests.filter((request) => request.path === 'uploads')).toHaveLength(1)
  expect(requests.some((request) => request.path === 'documents' || request.path === 'collections')).toBe(false)
})

test('refuses processing when the workspace file changes after opening', async ({ page }) => {
  const requests = await setup(page, { processing: true })
  await openWorkspacePaper(page)
  await expect(page.getByRole('button', { name: '处理论文', exact: true })).toBeEnabled()
  const changed = Buffer.concat([pdf, Buffer.from('\n% source replaced after reading\n')])
  await page.route('**/api/workspace-file/snapshot?**', (route) =>
    route.fulfill({
      contentType: 'application/pdf',
      body: changed,
      headers: {
        etag: `"${createHash('sha256').update(changed).digest('hex')}"`,
        'content-length': String(changed.length),
        'access-control-expose-headers': 'etag'
      }
    })
  )
  await page.getByRole('button', { name: '处理论文', exact: true }).click()
  const dialog = page.getByRole('dialog', { name: '处理论文', exact: true })
  await dialog.getByRole('button', { name: '开始处理', exact: true }).click()
  await expect(dialog.getByRole('alert')).toContainText('源文件已变化')
  expect(requests.some((request) => request.path === `documents/${documentId}/ocr`)).toBe(false)
})

async function installWorkspaceSave(page: Page) {
  const writes: {
    root: string | null
    directory: string | null
    name: string | null
    replace: string | null
    bytes: Buffer
  }[] = []
  await page.route('**/api/local-documents/entries?**', (route) =>
    route.fulfill({
      json: {
        entries: [{ name: 'paper.pdf', path: sourcePath, kind: 'file', sizeBytes: pdf.length, modifiedMs: 0 }],
        capped: false
      }
    })
  )
  await page.route('**/api/workspace-file/import?**', (route) => {
    const url = new URL(route.request().url())
    writes.push({
      root: url.searchParams.get('root'),
      directory: url.searchParams.get('directory'),
      name: url.searchParams.get('name'),
      replace: url.searchParams.get('replace'),
      bytes: route.request().postDataBuffer() ?? Buffer.alloc(0)
    })
    return route.fulfill({ json: {} })
  })
  return writes
}

test('saves checksum-verified output into the same workspace without overwriting the source', async ({ page }) => {
  await setup(page, { artifacts: true })
  const writes = await installWorkspaceSave(page)
  await openWorkspacePaper(page)
  await page.getByRole('radio', { name: '产物', exact: true }).click()
  await page.getByRole('button', { name: '存入工作区', exact: true }).click()
  const dialog = page.getByRole('dialog', { name: '存入工作区', exact: true })
  await expect(dialog).toContainText(workspaceRoot)
  await expect(dialog).not.toContainText('知识 → 文档')
  await dialog.getByRole('button', { name: '保存到此文件夹', exact: true }).click()
  await expect(dialog).toHaveCount(0)
  await expect(page.getByRole('status').filter({ hasText: '已存入工作区' })).toBeVisible()
  expect(writes).toHaveLength(1)
  expect(writes[0]).toMatchObject({
    root: workspaceRoot,
    directory: workspaceRoot,
    name: 'paper-translated.pdf',
    replace: 'false'
  })
  expect(createHash('sha256').update(writes[0].bytes).digest('hex')).toBe(artifactDigest)
  expect(createHash('sha256').update(pdf).digest('hex')).toBe(documentId)
})

test('only offers delivery for verified regular artifact files', async ({ page }) => {
  const requests = await setup(page, { artifacts: true, unverifiedArtifacts: true })
  await openWorkspacePaper(page)
  await page.getByRole('radio', { name: '产物', exact: true }).click()
  await expect(page.locator('[data-paperloom-reader]')).not.toContainText('internal-job-directory')
  const pending = page.getByRole('group', { name: 'unverified.pdf', exact: true })
  await expect(pending).toContainText('产物缺少校验摘要')
  await expect(pending.getByRole('button', { name: '存入工作区', exact: true })).toBeDisabled()
  await expect(pending.getByRole('button', { name: '下载', exact: true })).toBeDisabled()
  await expect(
    page
      .getByRole('group', { name: 'paper-translated.pdf', exact: true })
      .getByRole('button', { name: '存入工作区', exact: true })
  ).toBeEnabled()
  expect(
    requests.filter(
      (request) => request.path.endsWith('/artifacts/job-root') || request.path.endsWith('/artifacts/unverified-file')
    )
  ).toHaveLength(0)
})

test('revalidates file identity at save confirmation and refuses stale-source output', async ({ page }) => {
  await setup(page, { artifacts: true })
  const writes = await installWorkspaceSave(page)
  await openWorkspacePaper(page)
  await page.getByRole('radio', { name: '产物', exact: true }).click()
  await page.getByRole('button', { name: '存入工作区', exact: true }).click()
  const dialog = page.getByRole('dialog', { name: '存入工作区', exact: true })
  await expect(dialog).toBeVisible()
  await page.route('**/api/workspace-file/snapshot?**', (route) =>
    route.fulfill({
      contentType: 'application/pdf',
      body: pdf,
      headers: {
        etag: '"' + '78'.repeat(32) + '"',
        'access-control-expose-headers': 'etag',
        'content-length': String(pdf.length)
      }
    })
  )
  await dialog.getByRole('button', { name: '保存到此文件夹', exact: true }).click()
  await expect(dialog).toHaveCount(0)
  await expect(page.getByRole('alert')).toContainText('源文件已变化')
  expect(writes).toHaveLength(0)
})

async function saveArtifact(page: Page) {
  await openWorkspacePaper(page)
  await page.getByRole('radio', { name: '产物', exact: true }).click()
  await page.getByRole('button', { name: '存入工作区', exact: true }).click()
  await page
    .getByRole('dialog', { name: '存入工作区', exact: true })
    .getByRole('button', { name: '保存到此文件夹', exact: true })
    .click()
}

test('opens existing lineage UI with source-bound service receipts and an immutable PDF snapshot', async ({ page }) => {
  const requests = await setup(page, { artifacts: true })
  const writes = await installWorkspaceSave(page)
  await saveArtifact(page)
  await page.getByRole('button', { name: '查看产物追溯', exact: true }).click()
  const pane = page.locator('[data-paperloom-lineage]')
  await expect(pane.getByRole('tab')).toHaveCount(5)
  await expect(pane).toContainText('外部处理的执行状态由处理服务报告')
  await expect(pane.getByRole('link', { name: '下载此版本', exact: true })).toBeVisible()
  await expect(pane.getByRole('tabpanel')).not.toContainText('%PDF-')
  await pane.getByRole('tab', { name: '来源', exact: true }).click()
  await expect(pane.getByRole('tabpanel')).toContainText(sourcePath)
  await expect(pane.getByRole('tabpanel')).toContainText(`sha256:${documentId}`)
  await expect(pane.getByRole('tabpanel')).toContainText('gateway-job')
  await expect(pane.getByRole('tabpanel')).not.toContainText('paperloom.succeeded')
  await expect(pane.getByRole('tab', { name: '来源', exact: true })).toHaveCSS('background-color', 'rgb(242, 243, 245)')
  const selectedTab = await pane.getByRole('tab', { name: '来源', exact: true }).boundingBox()
  expect(selectedTab?.height).toBeGreaterThanOrEqual(30)
  await pane.getByRole('tab', { name: '来源', exact: true }).press('ArrowRight')
  await expect(pane.getByRole('tab', { name: '执行', exact: true })).toBeFocused()
  await pane.getByRole('button', { name: '返回阅读', exact: true }).click()
  expect(writes).toHaveLength(1)
  expect(requests.find((item) => item.path === 'provenance' && item.method === 'POST')).toMatchObject({
    method: 'POST'
  })
  const body = JSON.parse(requests.find((item) => item.path === 'provenance' && item.method === 'POST')!.body!)
  expect(body).toMatchObject({
    root: workspaceRoot,
    sourcePath,
    sourceRevision: '12'.repeat(32),
    sourceDigest: documentId,
    savedPath: `${workspaceRoot}/paper-translated.pdf`,
    jobId: 'gateway-job',
    artifactKey: 'translated-pdf',
    attempt: 1,
    checksum: `sha256:${artifactDigest}`
  })
  expect(body).not.toHaveProperty('bytes')
})

test('retries failed source registration without writing the saved artifact twice', async ({ page }) => {
  const requests = await setup(page, { artifacts: true, provenanceFailureOnce: true })
  const writes = await installWorkspaceSave(page)
  await saveArtifact(page)
  await expect(page.getByRole('alert')).toContainText('文件已保存，但来源登记失败')
  await expect(page.getByRole('button', { name: '查看产物追溯', exact: true })).toHaveCount(0)
  await page.locator('[data-paperloom-reader]').getByRole('button', { name: '重试', exact: true }).click()
  await expect(page.getByRole('button', { name: '查看产物追溯', exact: true })).toBeVisible()
  expect(writes).toHaveLength(1)
  expect(requests.filter((item) => item.path === 'provenance' && item.method === 'POST')).toHaveLength(2)
})

test('distinguishes saved copies with the same filename in different workspace folders', async ({ page }) => {
  await setup(page, { artifacts: true })
  await installWorkspaceSave(page)
  const registration = page.waitForResponse(
    (response) =>
      response.request().method() === 'POST' && new URL(response.url()).pathname === '/api/paperloom/provenance'
  )
  await saveArtifact(page)
  const { data: original } = await (await registration).json()
  const copy = {
    ...original,
    messageId: '44444444-4444-4444-8444-444444444444',
    artifacts: [
      {
        ...original.artifacts[0],
        id: '44444444-4444-4444-8444-444444444444',
        path: `${workspaceRoot}/review/paper-translated.pdf`
      }
    ]
  }
  await page.route('**/api/paperloom/provenance?*', (route) =>
    route.fulfill({ json: { code: 0, message: 'ok', data: [original, copy] } })
  )
  await page.getByRole('button', { name: '返回工作页', exact: true }).click()
  await page.locator(editor).fill('@论文精读')
  await page.getByTestId('quick-panel').locator('[data-id="workbench-agent:a3s-paperloom"]').click()
  await openWorkspacePaper(page)
  await page.getByRole('radio', { name: '产物', exact: true }).click()
  await expect(page.getByRole('button', { name: '查看 paper-translated.pdf 的产物追溯', exact: true })).toBeVisible()
  await expect(
    page.getByRole('button', { name: '查看 review/paper-translated.pdf 的产物追溯', exact: true })
  ).toBeVisible()
  await expect(page.getByText('review/paper-translated.pdf', { exact: true })).toBeVisible()
})

test('preserves native relevance ranking when searching for the original workspace PDF', async ({ page }) => {
  await setup(page)
  await page.route('**/api/workspace-search?**', (route) =>
    route.fulfill({
      json: {
        entries: [
          { path: sourcePath, isDirectory: false },
          { path: `${workspaceRoot}/paper-native-traced.pdf`, isDirectory: false }
        ]
      }
    })
  )
  await page.getByRole('button', { name: '工作区文件', exact: true }).click()
  await page.getByRole('textbox', { name: '搜索文件', exact: true }).fill('paper.pdf')
  await expect(page.getByRole('option').first()).toHaveAttribute('data-workspace-file', sourcePath)
  await page.getByRole('option').first().click()
  await page.getByRole('button', { name: '添加 1 项', exact: true }).click()
  await page.locator(send).click()
  await expect(pageImage(page)).toHaveAccessibleName(/Page 1/)
})

test('refuses a corrupted immutable snapshot in the shared lineage pane', async ({ page }) => {
  await setup(page, { artifacts: true, provenanceTampered: true })
  await installWorkspaceSave(page)
  await saveArtifact(page)
  await page.getByRole('button', { name: '查看产物追溯', exact: true }).click()
  const pane = page.locator('[data-paperloom-lineage]')
  await expect(pane.getByRole('alert')).toContainText('摘要')
  await expect(pane.getByRole('link', { name: '下载此版本', exact: true })).toHaveCount(0)
})

test('reopens saved lineage from the workspace source while the processor is offline', async ({ page }) => {
  await page.setViewportSize({ width: 520, height: 900 })
  await setup(page, { artifacts: true, offlineAfterSave: true })
  const writes = await installWorkspaceSave(page)
  await saveArtifact(page)
  await expect(page.getByRole('button', { name: '查看产物追溯', exact: true })).toBeVisible()
  await page.getByRole('button', { name: '返回工作页', exact: true }).click()
  await page.locator(editor).fill('@论文精读')
  await page.getByTestId('quick-panel').locator('[data-id="workbench-agent:a3s-paperloom"]').click()
  await page.locator(send).click()
  await chooseWorkspacePaper(page)
  await expect(page.getByRole('button', { name: '处理论文', exact: true })).toBeDisabled()
  await page.getByRole('radio', { name: '产物', exact: true }).click()
  await expect(page.locator('[data-paperloom-reader]')).not.toContainText('选择一个处理版本查看产物')
  await page.getByRole('button', { name: '查看 paper-translated.pdf 的产物追溯', exact: true }).click()
  await expect(
    page.locator('[data-paperloom-lineage]').getByRole('link', { name: '下载此版本', exact: true })
  ).toBeVisible()
  const pane = page.locator('[data-paperloom-lineage]')
  expect(await pane.evaluate((element) => element.scrollWidth <= element.clientWidth)).toBe(true)
  await page.screenshot({
    path: test.info().outputPath('browser-offline-lineage-520.png')
  })
  expect(writes).toHaveLength(1)
})

test('selects the original PDF with keyboard navigation when the workspace also contains output copies', async ({
  page
}) => {
  await setup(page)
  await page.route('**/api/workspace-search?**', (route) =>
    route.fulfill({
      json: {
        entries: [
          { path: `${workspaceRoot}/paper-native-traced.pdf`, isDirectory: false },
          { path: sourcePath, isDirectory: false }
        ]
      }
    })
  )
  await page.getByRole('button', { name: '工作区文件', exact: true }).click()
  const search = page.getByPlaceholder('搜索文件', { exact: true })
  await expect(search).toBeFocused()
  await search.dispatchEvent('keydown', { key: 'ArrowDown', isComposing: true })
  await expect(search).toBeFocused()
  await search.press('ArrowDown')
  const output = page.getByRole('option', { name: 'paper-native-traced.pdf', exact: true })
  const original = page.getByRole('option', { name: 'paper.pdf', exact: true })
  await expect(output).toBeFocused({ timeout: 1000 })
  await output.press('ArrowDown')
  await expect(original).toBeFocused()
  await original.press('Space')
  await expect(original).toHaveAttribute('aria-selected', 'true')
  await page.getByRole('button', { name: '添加 1 项', exact: true }).click()
  await page.locator(send).click()
  await expect(page.locator('[data-paperloom-reader]')).toHaveAttribute('data-paperloom-reader', documentId)
  await expect(pageImage(page)).toHaveAccessibleName(/Page 1/)
})

function paperloomData(data: unknown) {
  return { json: { code: 0, message: 'ok', data } }
}

function jobRecord(id: string, status: 'succeeded' | 'failed', markdownReady: boolean) {
  return {
    job_id: id,
    workflow: 'ocr',
    status,
    attempt: 1,
    markdown_ready: markdownReady,
    stages: {
      ocr: { state: status === 'succeeded' ? 'completed' : 'failed', progress: {} },
      translation: { state: 'skipped', progress: {} },
      render: { state: 'skipped', progress: {} }
    }
  }
}

test('keeps the chosen page across tool radios without a second page list', async ({ page }) => {
  await page.setViewportSize({ width: 1440, height: 900 })
  await setup(page)
  await openWorkspacePaper(page)
  const tools = reader(page).getByRole('radiogroup', { name: '论文工具', exact: true })
  await expect(tools.getByRole('radio')).toHaveCount(3)
  await expect(tools.getByRole('radio', { name: '阅读', exact: true })).toBeChecked()
  await expect(tools.getByRole('radio', { name: '处理任务', exact: true })).not.toBeChecked()
  await expect(pageRail(page)).toHaveCount(0)
  await expect(
    reader(page).getByRole('region', { name: '原文', exact: true }).getByText('原文', { exact: true })
  ).toHaveCount(0)
  await expect(reader(page).getByRole('button', { name: '查找', exact: true })).toHaveCount(0)
  const first = await shownPageText(page)
  await page.getByRole('button', { name: '下一页', exact: true }).click()
  await expect(page.getByRole('spinbutton', { name: '页码', exact: true })).toHaveValue('2')
  await expect.poll(() => shownPageText(page)).not.toBe(first)
  const second = await shownPageText(page)
  expect(second).not.toBe(first)
  await expect(pageImage(page)).toHaveAccessibleName(second)
  const spin = page.getByRole('spinbutton', { name: '页码', exact: true })
  await spin.fill('0')
  await spin.fill('9')
  await spin.fill('1.5')
  await expect(spin).toHaveValue('2')
  await expect(pageImage(page)).toHaveAccessibleName(second)
  await tools.getByRole('radio', { name: '处理任务', exact: true }).click()
  await expect(tools.getByRole('radio', { name: '处理任务', exact: true })).toBeChecked()
  await expect(tools.getByRole('radio', { name: '阅读', exact: true })).not.toBeChecked()
  await expect(reader(page)).toContainText('还没有处理任务。点击“处理论文”开始。')
  await expect(reader(page)).not.toContainText('工作流')
  await tools.getByRole('radio', { name: '阅读', exact: true }).click()
  await expect(spin).toHaveValue('2')
  await expect(pageImage(page)).toHaveAccessibleName(second)
  await page.getByRole('button', { name: '阅读视图', exact: true }).click()
  await expect(page.getByRole('option', { name: '原文', exact: true })).toBeVisible()
  await expect(page.getByRole('option', { name: '译文', exact: true })).toHaveCount(0)
  await expect(page.getByRole('option', { name: '对照', exact: true })).toHaveCount(0)
})

test('hides the page rail for a narrow reader and a one-page PDF', async ({ page }) => {
  await page.setViewportSize({ width: 520, height: 720 })
  await setup(page)
  await page.getByRole('button', { name: '收起侧边栏', exact: true }).click()
  await openWorkspacePaper(page)
  await expect(pageImage(page)).toHaveAccessibleName(/Page 1/)
  await expect(pageRail(page)).toHaveCount(0)
  await page.getByRole('button', { name: '下一页', exact: true }).click()
  await expect(page.getByRole('spinbutton', { name: '页码', exact: true })).toHaveValue('2')

  const source = await PDFDocument.load(pdf)
  const single = await PDFDocument.create()
  const [first] = await single.copyPages(source, [0])
  single.addPage(first)
  const onePage = Buffer.from(await single.save())
  const oneId = createHash('sha256').update(onePage).digest('hex')
  await page.route('**/api/workspace-file/snapshot?**', (route) =>
    route.fulfill({
      contentType: 'application/pdf',
      body: onePage,
      headers: {
        etag: `"${'ab'.repeat(32)}"`,
        'content-length': String(onePage.length),
        'access-control-expose-headers': 'etag'
      }
    })
  )
  await page.route(
    (url) => url.pathname === '/api/paperloom/uploads',
    (route) => route.fulfill(paperloomData({ document_id: oneId }))
  )
  await page.route(
    (url) => url.pathname === `/api/paperloom/documents/${oneId}`,
    (route) =>
      route.fulfill(
        paperloomData({
          ...paper,
          document_id: oneId,
          page_count: 1,
          bytes: onePage.length,
          source_filename: 'paper.pdf'
        })
      )
  )
  await page.setViewportSize({ width: 1440, height: 900 })
  await page.getByRole('button', { name: '选择其他工作区 PDF', exact: true }).click()
  await chooseWorkspacePaper(page)
  await expect(pageImage(page)).toHaveAccessibleName(/Page 1/)
  await expect(pageRail(page)).toHaveCount(0)
  await expect(page.getByRole('button', { name: '第 2 页', exact: true })).toHaveCount(0)
})

test('shows both page texts as markdown after text-layer processing', async ({ page }) => {
  await page.setViewportSize({ width: 1440, height: 900 })
  const requests = await setup(page, { processing: true })
  await openWorkspacePaper(page)
  const first = await shownPageText(page)
  await page.getByRole('button', { name: '下一页', exact: true }).click()
  await expect(page.getByRole('spinbutton', { name: '页码', exact: true })).toHaveValue('2')
  await expect.poll(() => shownPageText(page)).not.toBe(first)
  const second = await shownPageText(page)
  const markdown = `${first}\n\n${second}`
  await page.route(
    (url) => url.pathname === `/api/paperloom/documents/${documentId}/jobs`,
    (route) => route.fulfill(paperloomData({ items: [jobRecord('gateway-job', 'succeeded', true)] }))
  )
  await page.route(
    (url) => url.pathname === '/api/paperloom/jobs/gateway-job/markdown',
    (route) => route.fulfill({ contentType: 'text/markdown', body: markdown })
  )
  await page.getByRole('button', { name: '处理论文', exact: true }).click()
  const dialog = page.getByRole('dialog', { name: '处理论文', exact: true })
  await expect(dialog.getByText('本机文本层', { exact: true })).toBeVisible()
  await expect(dialog.getByRole('textbox', { name: '服务地址', exact: true })).toHaveCount(0)
  await expect(dialog.getByRole('button', { name: '工作流', exact: true })).toHaveCount(0)
  await dialog.getByRole('button', { name: '开始处理', exact: true }).click()
  await expect(dialog).toHaveCount(0)
  const submitted = requests.find((item) => item.path === `documents/${documentId}/ocr`)
  expect(JSON.parse(submitted?.body ?? '{}')).toEqual({
    workflow: 'ocr',
    source: {},
    ocr: { provider: 'a3s_gateway' }
  })
  await reader(page).getByRole('radio', { name: '阅读', exact: true }).click()
  await page.getByRole('button', { name: '阅读视图', exact: true }).click()
  await page.getByRole('option', { name: 'Markdown', exact: true }).click()
  await expect(reader(page)).toContainText(first)
  await expect(reader(page)).toContainText(second)
  await expect(pageRail(page)).toHaveCount(0)
})

test('fails a page without a text layer and offers only the source PDF', async ({ page }) => {
  const blankDoc = await PDFDocument.create()
  blankDoc.addPage()
  const blank = Buffer.from(await blankDoc.save())
  const blankId = createHash('sha256').update(blank).digest('hex')
  const failed = 'empty-text-job'
  await setup(page)
  await page.route('**/api/workspace-file/snapshot?**', (route) =>
    route.fulfill({
      contentType: 'application/pdf',
      body: blank,
      headers: {
        etag: `"${'cd'.repeat(32)}"`,
        'content-length': String(blank.length),
        'access-control-expose-headers': 'etag'
      }
    })
  )
  await page.route(
    (url) => url.pathname === '/api/paperloom/uploads',
    (route) => route.fulfill(paperloomData({ document_id: blankId }))
  )
  await page.route(
    (url) => url.pathname === `/api/paperloom/documents/${blankId}`,
    (route) => {
      if (route.request().method() !== 'GET') return route.fallback()
      return route.fulfill(
        paperloomData({ ...paper, document_id: blankId, page_count: 1, bytes: blank.length, title: 'blank' })
      )
    }
  )
  await page.route(
    (url) => url.pathname === `/api/paperloom/documents/${blankId}/ocr`,
    (route) => route.fulfill(paperloomData({ job_id: failed, status: 'failed' }))
  )
  await page.route(
    (url) => url.pathname === `/api/paperloom/documents/${blankId}/jobs`,
    (route) => route.fulfill(paperloomData({ items: [jobRecord(failed, 'failed', false)] }))
  )
  await page.route(
    (url) => url.pathname === `/api/paperloom/jobs/${failed}/artifacts-manifest`,
    (route) =>
      route.fulfill(
        paperloomData({
          job_id: failed,
          items: [
            {
              artifact_key: 'source',
              ready: true,
              file_name: 'paper.pdf',
              artifact_kind: 'file',
              checksum: `sha256:${blankId}`,
              attempt: 1
            }
          ]
        })
      )
  )
  await page.locator(send).click()
  await page.locator('[data-paperloom-workspace]').getByRole('button', { name: '工作区文件', exact: true }).click()
  await page.getByRole('option', { name: 'paper.pdf', exact: true }).click()
  await page.getByRole('button', { name: '添加 1 项', exact: true }).click()
  await expect(pageImage(page)).toBeVisible()
  await page.getByRole('button', { name: '处理论文', exact: true }).click()
  await page
    .getByRole('dialog', { name: '处理论文', exact: true })
    .getByRole('button', { name: '开始处理', exact: true })
    .click()
  await expect(reader(page).getByText('失败', { exact: true }).first()).toBeVisible()
  await reader(page).getByRole('radio', { name: '产物', exact: true }).click()
  await expect(reader(page).getByRole('group')).toHaveCount(1)
  await expect(reader(page).getByRole('group', { name: 'paper.pdf', exact: true })).toBeVisible()
  await reader(page).getByRole('radio', { name: '阅读', exact: true }).click()
  await page.getByRole('button', { name: '阅读视图', exact: true }).click()
  await expect(page.getByRole('option', { name: 'Markdown', exact: true })).toHaveCount(0)
})

test('opens the current document page from search and ignores a stale result', async ({ page }) => {
  await page.setViewportSize({ width: 1440, height: 900 })
  await setup(page)
  await openWorkspacePaper(page)
  await expect(pageImage(page)).toHaveAccessibleName(/Page 1/)
  await page.getByRole('button', { name: '下一页', exact: true }).click()
  await expect(pageImage(page)).toHaveAccessibleName(/Page 2/)
  await page.getByRole('button', { name: '上一页', exact: true }).click()
  await expect(page.getByRole('spinbutton', { name: '页码', exact: true })).toHaveValue('1')
  const first = 'Page 1'
  const second = 'Page 2'
  let releaseFirst: (() => void) | null = null
  let searches = 0
  await page.route(
    (url) => url.pathname === '/api/paperloom/search',
    async (route) => {
      searches += 1
      const query = new URL(route.request().url()).searchParams.get('q')
      if (searches === 1) await new Promise<void>((resolve) => (releaseFirst = resolve))
      const snippet = query === second ? second : first
      try {
        await route.fulfill(
          paperloomData({
            hits: [
              {
                document_id: documentId,
                job_id: 'search-hit',
                page_idx: snippet === second ? 1 : 0,
                block_id: 'block',
                source_snippet: snippet,
                translated_snippet: ''
              }
            ]
          })
        )
      } catch {
        // The reader aborts the older search. A late fulfill is not a product result.
      }
    }
  )
  const box = reader(page).getByRole('searchbox', { name: '搜索论文正文', exact: true })
  await box.pressSequentially(first)
  await box.press('Enter')
  await expect.poll(() => releaseFirst !== null).toBe(true)
  await box.press('ControlOrMeta+a')
  await box.pressSequentially(second)
  await box.press('Enter')
  releaseFirst?.()
  await expect(reader(page).getByRole('button', { name: `第 2 页 ${second}`, exact: true })).toBeVisible()
  await expect(reader(page).getByRole('button', { name: `第 1 页 ${first}`, exact: true })).toHaveCount(0)
  await reader(page)
    .getByRole('button', { name: `第 2 页 ${second}`, exact: true })
    .click()
  await expect(page.getByRole('spinbutton', { name: '页码', exact: true })).toHaveValue('2')
  await expect(pageImage(page)).toHaveAccessibleName(/Page 2/)
})

test('opens a citation on the current document and stays put when the citation is unavailable', async ({ page }) => {
  await setup(page)
  await openWorkspacePaper(page)
  const current = await shownPageText(page)
  const streams = await ipcCount(page, 'ai.stream.open')
  await page.route(
    (url) => url.pathname === '/api/paperloom/ai/ask',
    (route) =>
      route.fulfill(
        paperloomData({
          answer: current,
          persisted: false,
          citations: [
            {
              ref: 1,
              document_id: 'another-paper',
              job_id: 'other-job',
              page_idx: 1,
              block_id: 'foreign',
              snippet: 'foreign'
            },
            {
              ref: 2,
              document_id: documentId,
              job_id: 'current-job',
              page_idx: 1,
              block_id: 'current',
              snippet: current
            }
          ]
        })
      )
  )
  await page.getByRole('button', { name: '论文问答', exact: true }).click()
  await page.getByRole('textbox', { name: '论文问题', exact: true }).fill(current)
  await reader(page).getByRole('button', { name: '发送', exact: true }).click()
  await expect(reader(page)).toContainText('本轮回答未可靠保存，请保留所需内容后再离开。')
  await expect(reader(page)).toContainText(current)
  expect(await ipcCount(page, 'ai.stream.open')).toBe(streams)
  await reader(page).getByRole('button', { name: '[1] 第 2 页', exact: true }).click()
  await expect(
    reader(page).getByRole('alert').filter({ hasText: '引用不属于当前论文，或对应位置已不可用。' })
  ).toBeVisible()
  await expect(page.getByRole('spinbutton', { name: '页码', exact: true })).toHaveValue('1')
  await expect(pageImage(page)).toHaveAccessibleName(current)
  await reader(page).getByRole('button', { name: '[2] 第 2 页', exact: true }).click()
  await expect(page.getByRole('spinbutton', { name: '页码', exact: true })).toHaveValue('2')
  await expect(pageImage(page)).toHaveAccessibleName(/Page 2/)
})

test('lists verified files only for the selected processing version', async ({ page }) => {
  const done = 'done-reading'
  const failed = 'failed-reading'
  await setup(page)
  await page.route(
    (url) => url.pathname === `/api/paperloom/documents/${documentId}/jobs`,
    (route) =>
      route.fulfill(paperloomData({ items: [jobRecord(done, 'succeeded', false), jobRecord(failed, 'failed', false)] }))
  )
  await page.route(
    (url) => url.pathname.includes('/artifacts-manifest'),
    (route) => {
      const failedJob = route.request().url().includes(failed)
      return route.fulfill(
        paperloomData({
          job_id: failedJob ? failed : done,
          items: [
            {
              artifact_key: failedJob ? 'source' : 'reading',
              ready: true,
              file_name: failedJob ? 'paper.pdf' : 'reading.md',
              artifact_kind: 'file',
              checksum: `sha256:${'e'.repeat(64)}`,
              attempt: 1
            }
          ]
        })
      )
    }
  )
  await openWorkspacePaper(page)
  await reader(page).getByRole('radio', { name: '产物', exact: true }).click()
  await expect(reader(page)).toContainText('选择一个处理版本查看产物。')
  await expect(reader(page).getByRole('button', { name: '存入工作区', exact: true })).toHaveCount(0)
  await page.getByRole('button', { name: '处理版本', exact: true }).click()
  await page.getByRole('option', { name: /已完成$/ }).click()
  await expect(reader(page).getByRole('group', { name: 'reading.md', exact: true })).toBeVisible()
  await expect(reader(page).getByRole('group', { name: 'paper.pdf', exact: true })).toHaveCount(0)
  await page.getByRole('button', { name: '处理版本', exact: true }).click()
  await page.getByRole('option', { name: /失败$/ }).click()
  await expect(reader(page).getByRole('group', { name: 'paper.pdf', exact: true })).toBeVisible()
  await expect(reader(page).getByRole('group', { name: 'reading.md', exact: true })).toHaveCount(0)
})

const complexPdf = readFileSync('tests/fixtures/paperloom/complex-paper.pdf')
const complexCapture = JSON.parse(readFileSync('tests/fixtures/paperloom/complex-paper.capture.json', 'utf8')) as {
  document_id: string
  page_count: number
  markdown: string
  artifacts: { artifact_key: string; file_name: string; checksum: string }[]
  searches: Record<string, { page_idx: number; source_snippet: string }[]>
  asks: Record<
    string,
    {
      answer: string
      persisted: boolean
      citations: { ref: number; page_idx: number; block_id: string; snippet: string }[]
    }
  >
}
const complexId = createHash('sha256').update(complexPdf).digest('hex')
const complexPath = `${workspaceRoot}/复杂论文.pdf`

test('reads one complex paper from the text layer and replays that handler output', async ({ page }) => {
  test.setTimeout(180_000)
  console.log('route=replay source=tests/fixtures/paperloom/complex-paper.capture.json')
  expect(complexId).toBe(complexCapture.document_id)
  expect(complexCapture.page_count).toBe(8)
  await page.setViewportSize({ width: 1280, height: 900 })
  const requests = await setup(page)
  const jobs: {
    job_id: string
    workflow: 'ocr'
    status: 'succeeded'
    attempt: number
    markdown_ready: boolean
    stages: {
      ocr: { state: string; progress: { current: number; total: number; unit: string } }
      translation: { state: string; progress: Record<string, never> }
      render: { state: string; progress: Record<string, never> }
    }
  }[] = []
  let servedMarkdown = ''
  await page.route('**/api/workspace-search?**', (route) =>
    route.fulfill({ json: { entries: [{ path: complexPath, isDirectory: false }] } })
  )
  await page.route('**/api/workspace-file/snapshot?**', (route) => {
    const url = new URL(route.request().url())
    if (url.searchParams.get('root') !== workspaceRoot || url.searchParams.get('path') !== complexPath)
      return route.fallback()
    return route.fulfill({
      contentType: 'application/pdf',
      body: complexPdf,
      headers: {
        etag: `"${complexId}"`,
        'content-length': String(complexPdf.length),
        'access-control-expose-headers': 'etag'
      }
    })
  })
  await page.route('**/api/paperloom/**', async (route) => {
    const request = route.request()
    const url = new URL(request.url())
    expect(url.searchParams.getAll('studio_member')).toHaveLength(1)
    url.searchParams.delete('studio_member')
    const path = url.pathname.slice('/api/paperloom/'.length)
    requests.push({ path, method: request.method(), body: request.postData() })
    const json = (data: unknown) => route.fulfill(paperloomData(data))
    if (path === 'connection') {
      return route.fulfill({ json: { endpoint: '', apiKeyConfigured: true, revision: 'a3s-native-paper' } })
    }
    if (path === 'provenance' && request.method() === 'GET') return json([])
    if (path === 'uploads') return json({ document_id: complexId })
    if (path === `documents/${complexId}` && request.method() === 'GET') {
      return json({
        document_id: complexId,
        title: '复杂论文',
        source_filename: '复杂论文.pdf',
        page_count: 8,
        bytes: complexPdf.length,
        active_job_id: jobs.at(-1)?.job_id ?? null,
        reading_status: 'unread',
        added_at: '2026-10-17T00:00:00Z',
        updated_at: '2026-10-17T00:00:00Z',
        tags: []
      })
    }
    if (path === `documents/${complexId}/source.pdf`)
      return route.fulfill({ contentType: 'application/pdf', body: complexPdf })
    if (path === `documents/${complexId}/ocr` && request.method() === 'POST') {
      const jobId = `complex-job-${jobs.length + 1}`
      jobs.push({
        job_id: jobId,
        workflow: 'ocr',
        status: 'succeeded',
        attempt: 1,
        markdown_ready: true,
        stages: {
          ocr: { state: 'completed', progress: { current: 8, total: 8, unit: 'page' } },
          translation: { state: 'skipped', progress: {} },
          render: { state: 'skipped', progress: {} }
        }
      })
      return json({ job_id: jobId, status: 'succeeded' })
    }
    if (path === `documents/${complexId}/jobs`) return json({ items: jobs })
    if (path.endsWith('/markdown')) {
      servedMarkdown = complexCapture.markdown
      return route.fulfill({ contentType: 'text/plain; charset=utf-8', body: complexCapture.markdown })
    }
    if (path.endsWith('/artifacts-manifest')) {
      const jobId = path.split('/')[1]
      return json({
        job_id: jobId,
        items: complexCapture.artifacts.map((item) => ({
          artifact_key: item.artifact_key,
          ready: true,
          file_name: item.file_name,
          artifact_kind: 'file',
          checksum: item.checksum,
          attempt: 1
        }))
      })
    }
    if (path === 'search') {
      const term = url.searchParams.get('q') ?? ''
      const hits = complexCapture.searches[term] ?? []
      return json({
        hits: hits.map((hit) => ({
          document_id: complexId,
          job_id: jobs.at(-1)?.job_id ?? '',
          page_idx: hit.page_idx,
          block_id: `p${hit.page_idx + 1}`,
          source_snippet: hit.source_snippet,
          translated_snippet: ''
        }))
      })
    }
    if (path === 'ai/ask' && request.method() === 'POST') {
      const question = String(request.postDataJSON()?.question ?? '')
      const answer = complexCapture.asks[question]
      if (!answer) return route.fulfill({ status: 400, json: { error: 'unexpected_question', question } })
      return json({
        answer: answer.answer,
        persisted: answer.persisted,
        citations: answer.citations.map((citation) => ({
          ...citation,
          document_id: complexId,
          job_id: jobs.at(-1)?.job_id ?? ''
        }))
      })
    }
    return route.fulfill({ status: 404, json: { error: 'unexpected_request', path } })
  })

  await page.locator(send).click()
  await page.locator('[data-paperloom-workspace]').getByRole('button', { name: '工作区文件', exact: true }).click()
  await page.getByRole('option', { name: '复杂论文.pdf', exact: true }).click()
  await page.getByRole('button', { name: '添加 1 项', exact: true }).click()
  await expect(pageImage(page)).toBeVisible()
  const cover = await shownPageText(page)
  expect(cover).toContain('复杂论文标题甲')
  expect(cover).not.toContain('页眉丙')
  console.log(`canvas-page-1=${cover}`)
  await expect(page.getByRole('button', { name: '上一页', exact: true })).toBeDisabled()
  await expect(page.getByRole('button', { name: '下一页', exact: true })).toBeEnabled()
  await expect(reader(page)).toContainText('/ 8')
  await expect(pageRail(page)).toHaveCount(0)
  await expect(reader(page).getByRole('button', { name: '查找', exact: true })).toHaveCount(0)

  const spin = page.getByRole('spinbutton', { name: '页码', exact: true })
  await spin.fill('2')
  await expect(spin).toHaveValue('2')
  await expect.poll(() => shownPageText(page)).toContain('摘要句乙')
  expect(await shownPageText(page)).toContain('AlphaToken')
  expect(await shownPageText(page)).toContain('重复词辛')
  expect(await shownPageText(page)).toContain('页眉丙')
  const pageTwo = await shownPageText(page)
  await spin.fill('0')
  await spin.fill('9')
  await spin.fill('1.5')
  await expect(spin).toHaveValue('2')
  await expect(pageImage(page)).toHaveAccessibleName(pageTwo)

  await spin.fill('6')
  await expect.poll(() => shownPageText(page)).toContain('图1说明戊')
  await spin.fill('7')
  await expect(spin).toHaveValue('7')
  await expect.poll(() => shownPageText(page)).toBe('原文 7')
  console.log(`canvas-page-7=${await shownPageText(page)}`)
  await spin.fill('8')
  await expect.poll(() => shownPageText(page)).toContain('参考文献己')
  await expect(page.getByRole('button', { name: '下一页', exact: true })).toBeDisabled()

  const tools = reader(page).getByRole('radiogroup', { name: '论文工具', exact: true })
  await tools.getByRole('radio', { name: '处理任务', exact: true }).click()
  await expect(reader(page)).toContainText('还没有处理任务。点击“处理论文”开始。')
  await expect(reader(page)).not.toContainText('工作流')
  await tools.getByRole('radio', { name: '阅读', exact: true }).click()
  await expect(spin).toHaveValue('8')
  await page.getByRole('button', { name: '阅读视图', exact: true }).click()
  await expect(page.getByRole('option', { name: '原文', exact: true })).toBeVisible()
  await expect(page.getByRole('option', { name: '译文', exact: true })).toHaveCount(0)
  await expect(page.getByRole('option', { name: '对照', exact: true })).toHaveCount(0)
  await expect(page.getByRole('option', { name: 'Markdown', exact: true })).toHaveCount(0)
  await page.keyboard.press('Escape')

  await page.getByRole('button', { name: '处理论文', exact: true }).click()
  const dialog = page.getByRole('dialog', { name: '处理论文', exact: true })
  await expect(dialog.getByText('本机文本层', { exact: true })).toBeVisible()
  await expect(dialog.locator('[data-paperloom-ocr-source="text-layer"]')).toHaveText('本机文本层')
  await expect(dialog.getByRole('textbox', { name: '服务地址', exact: true })).toHaveCount(0)
  await expect(dialog.getByRole('button', { name: '工作流', exact: true })).toHaveCount(0)
  await expect(dialog.getByRole('button', { name: '关闭', exact: true })).toHaveCount(0)
  await expect(dialog.getByRole('button', { name: '取消', exact: true })).toBeInViewport()
  await expect(dialog.getByRole('button', { name: '开始处理', exact: true })).toBeInViewport()
  await dialog.getByRole('button', { name: '取消', exact: true }).click()
  await expect(dialog).toHaveCount(0)
  expect(requests.filter((item) => item.path.endsWith('/ocr'))).toHaveLength(0)

  await page.getByRole('button', { name: '处理论文', exact: true }).click()
  await dialog.getByRole('button', { name: '开始处理', exact: true }).click()
  await expect(dialog).toHaveCount(0)
  const submitted = requests.filter((item) => item.method === 'POST' && item.path.endsWith('/ocr'))
  expect(JSON.parse(submitted[0]?.body ?? '{}')).toEqual({
    workflow: 'ocr',
    source: {},
    ocr: { provider: 'a3s_gateway' }
  })
  console.log(`ocr-body=${submitted[0]?.body}`)
  console.log('actions-1280=取消,开始处理')
  await expect(reader(page).getByRole('heading', { name: '仅 OCR', exact: true })).toBeVisible()
  await expect(reader(page).getByText('已完成', { exact: true }).first()).toBeVisible()
  await expect(reader(page)).toContainText('8/8')
  await expect(reader(page)).toContainText('已跳过')
  await expect(reader(page).getByRole('button', { name: '停止', exact: true })).toHaveCount(0)
  await expect(reader(page).getByRole('button', { name: '恢复任务', exact: true })).toHaveCount(0)

  await tools.getByRole('radio', { name: '阅读', exact: true }).click()
  await expect(spin).toHaveValue('8')
  await page.getByRole('button', { name: '阅读视图', exact: true }).click()
  await page.getByRole('option', { name: 'Markdown', exact: true }).click()
  await expect.poll(async () => reader(page).locator('pre').textContent()).toBe(complexCapture.markdown)
  expect(servedMarkdown).toBe(complexCapture.markdown)
  console.log('replayed-markdown=handler-capture')
  await expect(pageRail(page)).toHaveCount(0)

  await page.getByRole('button', { name: '阅读视图', exact: true }).click()
  await page.getByRole('option', { name: '原文', exact: true }).click()
  const search = reader(page).getByRole('searchbox', { name: '搜索论文正文', exact: true })
  await search.fill('图1说明戊')
  await search.press('Enter')
  const figureHit = complexCapture.searches['图1说明戊'][0]
  await reader(page)
    .getByRole('button', { name: new RegExp(`第 ${figureHit.page_idx + 1} 页`) })
    .click()
  await expect(spin).toHaveValue(String(figureHit.page_idx + 1))
  await expect.poll(() => shownPageText(page)).toContain('图1说明戊')
  await search.fill('alphatoken')
  await search.press('Enter')
  await expect(reader(page).getByRole('button', { name: /第 \d+ 页/ })).toHaveCount(0)
  await search.fill('重复词辛')
  await search.press('Enter')
  for (const hit of complexCapture.searches['重复词辛']) {
    await expect(reader(page).getByRole('button', { name: new RegExp(`第 ${hit.page_idx + 1} 页`) })).toBeVisible()
  }

  const streams = await ipcCount(page, 'ai.stream.open')
  await page.getByRole('button', { name: '论文问答', exact: true }).click()
  const question = reader(page).getByRole('textbox', { name: '论文问题', exact: true })
  await question.fill('图1说明戊')
  await reader(page).getByRole('button', { name: '发送', exact: true }).click()
  await expect(reader(page)).toContainText('本轮回答未可靠保存，请保留所需内容后再离开。')
  console.log('unsaved-answer=本轮回答未可靠保存，请保留所需内容后再离开。')
  await expect(reader(page)).toContainText(complexCapture.asks['图1说明戊'].answer)
  expect(await ipcCount(page, 'ai.stream.open')).toBe(streams)
  await reader(page).getByRole('button', { name: '[1] 第 6 页', exact: true }).click()
  await expect(spin).toHaveValue('6')
  await question.fill('这句话不在论文里')
  await reader(page).getByRole('button', { name: '发送', exact: true }).click()
  await expect(reader(page)).toContainText('正文中没有这句话。')
  await expect(spin).toHaveValue('6')
  await question.fill('页眉丙')
  await reader(page).getByRole('button', { name: '发送', exact: true }).click()
  for (const citation of complexCapture.asks['页眉丙'].citations) {
    await expect(
      reader(page).getByRole('button', { name: `[${citation.ref}] 第 ${citation.page_idx + 1} 页`, exact: true })
    ).toBeVisible()
  }
  await reader(page).getByRole('button', { name: '[1] 第 2 页', exact: true }).click()
  await expect(spin).toHaveValue('2')

  await tools.getByRole('radio', { name: '产物', exact: true }).click()
  await page.getByRole('button', { name: '处理版本', exact: true }).click()
  await page.getByRole('option', { name: '原始文件', exact: true }).click()
  await expect(reader(page)).toContainText('选择一个处理版本查看产物。')
  await expect(reader(page).getByRole('button', { name: '存入工作区', exact: true })).toHaveCount(0)
  await page.getByRole('button', { name: '处理版本', exact: true }).click()
  await page.getByRole('option', { name: 'complex-job-1 · 已完成', exact: true }).click()
  await expect(reader(page).getByRole('group', { name: '复杂论文.pdf', exact: true })).toHaveCount(1)
  await expect(reader(page).getByRole('group', { name: 'paper.md', exact: true })).toHaveCount(1)
  await expect(reader(page)).toContainText(complexCapture.artifacts[1].checksum)
  console.log(
    `version-scope=complex-job-1 files=复杂论文.pdf,paper.md checksum=${complexCapture.artifacts[1].checksum}`
  )

  await page.getByRole('button', { name: '处理论文', exact: true }).click()
  await dialog.getByRole('button', { name: '开始处理', exact: true }).click()
  await expect(dialog).toHaveCount(0)
  const again = requests.filter((item) => item.method === 'POST' && item.path.endsWith('/ocr'))
  expect(again).toHaveLength(2)
  expect(JSON.parse(again[1]?.body ?? '{}')).toEqual(JSON.parse(again[0]?.body ?? '{}'))
  await tools.getByRole('radio', { name: '产物', exact: true }).click()
  await page.getByRole('button', { name: '处理版本', exact: true }).click()
  await page.getByRole('option', { name: 'complex-job-1 · 已完成', exact: true }).click()
  await expect(reader(page).getByRole('group', { name: 'paper.md', exact: true })).toHaveCount(1)
  await page.getByRole('button', { name: '处理版本', exact: true }).click()
  await page.getByRole('option', { name: 'complex-job-2 · 已完成', exact: true }).click()
  await expect(reader(page).getByRole('group', { name: 'paper.md', exact: true })).toHaveCount(1)
  await expect(reader(page).getByRole('group', { name: '复杂论文.pdf', exact: true })).toHaveCount(1)

  await page.setViewportSize({ width: 520, height: 720 })
  await page.getByRole('button', { name: '收起侧边栏', exact: true }).click()
  await tools.getByRole('radio', { name: '阅读', exact: true }).click()
  await expect(pageRail(page)).toHaveCount(0)
  await expect(page.getByRole('button', { name: '上一页', exact: true })).toBeVisible()
  await expect(page.getByRole('button', { name: '下一页', exact: true })).toBeVisible()
  await expect(spin).toBeVisible()
  await page.getByRole('button', { name: '处理论文', exact: true }).click()
  await expect(dialog.getByRole('button', { name: '取消', exact: true })).toBeInViewport()
  await expect(dialog.getByRole('button', { name: '开始处理', exact: true })).toBeInViewport()
  await dialog.getByRole('button', { name: '取消', exact: true }).click()
  expect(requests.filter((item) => item.method === 'POST' && item.path.endsWith('/ocr'))).toHaveLength(2)
  console.log('actions-520=取消,开始处理 pager=上一页,页码,下一页')
})
