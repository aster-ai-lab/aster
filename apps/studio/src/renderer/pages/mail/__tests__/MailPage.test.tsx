import { cleanup, render, screen, waitFor, within } from '@testing-library/react'
import userEvent from '@testing-library/user-event'
import { SWRConfig } from 'swr'
import { afterEach, describe, expect, it, vi } from 'vitest'

import { InboxHome } from '@renderer/components/workInbox/InboxHome'
import { writeCenterId } from '@renderer/data/memberOrgs'
import { toast } from '@renderer/services/toast'

import MailPage from '../MailPage'

const ORG = '0197e5a2-89f1-7d33-a11c-5f53d9b3af20'
const teamPolicy = {
  id: ORG,
  name: 'Team',
  can_read_mail: true,
  can_create_mail: true,
  can_send_mail: true,
  can_update_mail: true,
  can_delete_mail: true,
  rules: ['read', 'create', 'send', 'update', 'delete'].map((action) => ({ action, subject: 'MailMessage' })),
  data_scope: { kind: 'all', department_ids: [] }
}
const TOKEN = 'cd'.repeat(32)
const layoutBox = () => new DOMRect(0, 0, 120, 24)
const layoutRects = () => {
  const list = [layoutBox()]
  ;(list as unknown as DOMRectList).item = (index: number) => list[index] ?? null
  return list as unknown as DOMRectList
}
Range.prototype.getBoundingClientRect = layoutBox
Range.prototype.getClientRects = layoutRects
Element.prototype.getClientRects = layoutRects
document.elementFromPoint = () => document.body
window.scrollBy = () => undefined

const summary = {
  id: '0197e5a2-89f1-7d33-a11c-5f53d9b3af10',
  folder: 'inbox',
  from_address: 'crates@mail.towork',
  from_name: 'crates.io',
  to: ['roy@mail.towork'],
  cc: [],
  subject: 'Successfully published a3s@0.22.1',
  preview: 'A new version',
  unread: true,
  flagged: false,
  thread_id: '0197e5a2-89f1-7d33-a11c-5f53d9b3af11',
  created_at: '2026-10-06T11:14:11Z',
  body: 'Hello ZhiXiao-Lin',
  can_update: true,
  can_delete: true
}

function visibleMail(value: string): string {
  return value
    .replace(/<[^>]+>/g, ' ')
    .replace(/&nbsp;/gi, ' ')
    .replace(/\s+/g, ' ')
    .trim()
}

function envelope(data: unknown): Response {
  return new Response(JSON.stringify({ success: true, code: 0, message: 'ok', data }), { status: 200 })
}

afterEach(() => {
  cleanup()
  writeCenterId('personal')
  vi.unstubAllGlobals()
  delete (globalThis as { __A3S_STUDIO_RUNTIME__?: unknown }).__A3S_STUDIO_RUNTIME__
  delete (globalThis as { __TAURI_INTERNALS__?: unknown }).__TAURI_INTERNALS__
  localStorage.removeItem('mail.signatures.v1')
  localStorage.removeItem('mail.tobot.v1')
  localStorage.removeItem('work.tobot.v1')
})

function install(fetchMock: (url: string, init?: RequestInit) => Promise<Response>): void {
  Object.defineProperty(globalThis, '__A3S_STUDIO_RUNTIME__', {
    configurable: true,
    value: { apiBaseUrl: 'http://127.0.0.1:9', token: TOKEN }
  })
  writeCenterId(ORG)
  vi.stubGlobal('fetch', (url: Parameters<typeof fetch>[0], init?: RequestInit) =>
    new URL(String(url)).pathname === '/api/member-orgs'
      ? Promise.resolve(envelope([teamPolicy]))
      : fetchMock(String(url), init)
  )
}

describe('MailPage', () => {
  it('shows the assigned address and switches the list when a folder is chosen', async () => {
    const user = userEvent.setup()
    const folders: string[] = []
    install(
      vi.fn(async (url: string) => {
        const text = String(url)
        if (new URL(text).pathname === '/api/mail/mailbox') {
          return envelope({ id: 'mb-1', address: 'roy@mail.towork', unread: 1 })
        }
        const folder = new URL(text).searchParams.get('folder') ?? ''
        folders.push(folder)
        if (folder === 'drafts') return envelope({ items: [], unread: 1 })
        return envelope({ items: [summary], unread: 1 })
      })
    )

    render(
      <SWRConfig value={{ provider: () => new Map(), dedupingInterval: 0 }}>
        <MailPage />
      </SWRConfig>
    )

    expect(await screen.findByText('roy@mail.towork')).toBeInTheDocument()
    expect(await screen.findByText('选择一封邮件')).toBeInTheDocument()
    expect(await screen.findByText('Successfully published a3s@0.22.1')).toBeInTheDocument()
    expect(screen.getByRole('button', { name: '收件箱' })).toHaveAttribute('aria-current', 'page')

    await user.click(screen.getByRole('button', { name: '草稿箱' }))
    await waitFor(() => expect(folders).toContain('drafts'))
    expect(screen.getByRole('button', { name: '草稿箱' })).toHaveAttribute('aria-current', 'page')
    await waitFor(() => expect(screen.queryByText('Successfully published a3s@0.22.1')).not.toBeInTheDocument())
    expect(
      within(document.querySelector('[data-ui="mail.list"]') as HTMLElement).getByText('没有邮件')
    ).toBeInTheDocument()
  })

  it('posts a composed message', async () => {
    const user = userEvent.setup()
    const posts: string[] = []
    install(
      vi.fn(async (url: string, init?: RequestInit) => {
        const text = String(url)
        if (init?.method === 'POST') {
          posts.push(String(init.body))
          return envelope({ ...summary, folder: 'sent', unread: false, subject: 'hello', body: 'note' })
        }
        if (new URL(text).pathname === '/api/mail/mailbox') {
          return envelope({ id: 'mb-1', address: 'roy@mail.towork', unread: 0 })
        }
        const folder = new URL(text).searchParams.get('folder')
        if (folder === 'sent') {
          return envelope({
            items: [{ ...summary, folder: 'sent', unread: false, subject: 'hello', body: 'note' }],
            unread: 0
          })
        }
        return envelope({ items: [], unread: 0 })
      })
    )

    render(
      <SWRConfig value={{ provider: () => new Map(), dedupingInterval: 0 }}>
        <MailPage />
      </SWRConfig>
    )
    await screen.findByText('roy@mail.towork')
    await user.click(screen.getByRole('button', { name: '写邮件' }))
    const dialog = screen.getByRole('dialog')
    expect(dialog).toHaveAttribute('data-ui', 'mail.compose')
    expect(within(dialog).getByRole('button', { name: '发送' })).toBeDisabled()
    await user.type(screen.getByRole('textbox', { name: '收件人' }), 'crates@mail.towork')
    await user.keyboard('{Enter}')
    expect(posts).toHaveLength(0)
    await user.keyboard('{Enter}')
    expect(screen.getByRole('textbox', { name: '主题' })).toHaveFocus()
    await user.type(screen.getByRole('textbox', { name: '主题' }), 'hello')
    await user.type(screen.getByRole('textbox', { name: '正文' }), 'note')
    expect(screen.getByRole('button', { name: '发送' })).toBeEnabled()
    await user.click(screen.getByRole('button', { name: '发送' }))

    await waitFor(() => expect(posts).toHaveLength(1))
    const sent = JSON.parse(posts[0] ?? '{}') as { body?: string; to?: string[]; subject?: string; draft?: boolean }
    expect(sent).toMatchObject({
      to: ['crates@mail.towork'],
      subject: 'hello',
      draft: false
    })
    expect(visibleMail(sent.body ?? '')).toBe('note')
    expect(sent.body ?? '').toContain('<')
    expect(await screen.findByRole('heading', { name: 'hello' })).toBeInTheDocument()
    expect(screen.queryByRole('dialog')).not.toBeInTheDocument()
  })

  it('keeps an open message and asks before discarding a draft', async () => {
    const user = userEvent.setup()
    install(
      vi.fn(async (url: string) => {
        const text = String(url)
        if (new URL(text).pathname === '/api/mail/mailbox') {
          return envelope({ id: 'mb-1', address: 'roy@mail.towork', unread: 1 })
        }
        if (text.includes('/api/mail/messages/')) return envelope(summary)
        return envelope({ items: [summary], unread: 1 })
      })
    )

    render(
      <SWRConfig value={{ provider: () => new Map(), dedupingInterval: 0 }}>
        <MailPage />
      </SWRConfig>
    )
    await user.click(await screen.findByRole('button', { name: /Successfully published/ }))
    expect(await screen.findByRole('heading', { name: 'Successfully published a3s@0.22.1' })).toBeInTheDocument()

    await user.click(screen.getByRole('button', { name: '写邮件' }))
    const dialog = screen.getByRole('dialog')
    expect(dialog).toHaveAttribute('data-ui', 'mail.compose')
    expect(screen.queryByRole('textbox', { name: '抄送' })).not.toBeInTheDocument()
    await user.click(within(dialog).getByRole('button', { name: '抄送' }))
    expect(screen.getByRole('textbox', { name: '抄送' })).toBeInTheDocument()
    expect(screen.getByRole('heading', { name: 'Successfully published a3s@0.22.1', hidden: true })).toBeInTheDocument()

    await user.keyboard('{Escape}')
    await waitFor(() => expect(screen.queryByRole('dialog')).not.toBeInTheDocument())
    expect(screen.queryByRole('dialog', { name: '放弃这封邮件？' })).not.toBeInTheDocument()

    await user.click(screen.getByRole('button', { name: '写邮件' }))
    await user.type(screen.getByRole('textbox', { name: '主题' }), 'draft')
    await user.keyboard('{Escape}')
    expect(await screen.findByRole('dialog', { name: '放弃这封邮件？' })).toBeInTheDocument()
    await user.click(screen.getByRole('button', { name: '取消' }))
    expect(screen.getByRole('textbox', { name: '主题' })).toHaveValue('draft')

    await user.keyboard('{Escape}')
    await user.click(await screen.findByRole('button', { name: '放弃' }))
    await waitFor(() => expect(screen.queryByRole('dialog')).not.toBeInTheDocument())
    expect(screen.getByRole('heading', { name: 'Successfully published a3s@0.22.1' })).toBeInTheDocument()
  })

  it('sends a reply with the quoted message and saves a draft', async () => {
    const user = userEvent.setup()
    const posts: string[] = []
    install(
      vi.fn(async (url: string, init?: RequestInit) => {
        const text = String(url)
        if (init?.method === 'POST' && text.endsWith('/api/mail/messages')) {
          posts.push(String(init.body))
          const body = JSON.parse(String(init.body)) as { draft?: boolean; subject?: string }
          return envelope({
            ...summary,
            folder: body.draft ? 'drafts' : 'sent',
            unread: false,
            subject: body.subject ?? 'hello',
            body: 'note'
          })
        }
        if (new URL(text).pathname === '/api/mail/mailbox') {
          return envelope({ id: 'mb-1', address: 'roy@mail.towork', unread: 1 })
        }
        if (text.includes('/api/mail/messages/')) return envelope(summary)
        const folder = new URL(text).searchParams.get('folder')
        if (folder === 'sent' || folder === 'drafts') {
          return envelope({ items: [{ ...summary, folder, unread: false, subject: 'kept' }], unread: 0 })
        }
        return envelope({ items: [summary], unread: 1 })
      })
    )

    render(
      <SWRConfig value={{ provider: () => new Map(), dedupingInterval: 0 }}>
        <MailPage />
      </SWRConfig>
    )
    await user.click(await screen.findByRole('button', { name: /Successfully published/ }))
    await screen.findByRole('heading', { name: 'Successfully published a3s@0.22.1' })
    await user.click(screen.getByRole('button', { name: '回复' }))

    const dialog = screen.getByRole('dialog')
    expect(within(dialog).getByText('crates@mail.towork')).toBeInTheDocument()
    expect(within(dialog).getByRole('textbox', { name: '收件人' })).toHaveValue('')
    expect(within(dialog).getByRole('textbox', { name: '主题' })).toHaveValue('Re: Successfully published a3s@0.22.1')
    const replyBody = within(dialog).getByRole('textbox', { name: '正文' })
    expect(replyBody).toHaveTextContent('crates.io')
    expect(replyBody).toHaveTextContent('Hello ZhiXiao-Lin')
    await user.click(within(dialog).getByRole('button', { name: '存草稿' }))

    await waitFor(() => expect(posts).toHaveLength(1))
    const drafted = JSON.parse(posts[0] ?? '{}') as { body?: string }
    expect(drafted).toMatchObject({
      to: ['crates@mail.towork'],
      subject: 'Re: Successfully published a3s@0.22.1',
      draft: true,
      thread_id: summary.thread_id
    })
    expect(drafted.body ?? '').toContain('<blockquote>')
    expect(visibleMail(drafted.body ?? '')).toContain('Hello ZhiXiao-Lin')
    expect(screen.queryByRole('dialog')).not.toBeInTheDocument()
  })

  it('rejects an address the mailbox cannot deliver and caps the recipient list', async () => {
    const user = userEvent.setup()
    install(
      vi.fn(async (url: string, init?: RequestInit) => {
        const text = String(url)
        if (init?.method === 'POST') {
          return new Response(JSON.stringify({ success: false, code: 400, message: 'unknown recipient', data: null }), {
            status: 400
          })
        }
        if (new URL(text).pathname === '/api/mail/mailbox')
          return envelope({ id: 'mb-1', address: 'roy@mail.towork', unread: 0 })
        return envelope({ items: [], unread: 0 })
      })
    )

    render(
      <SWRConfig value={{ provider: () => new Map(), dedupingInterval: 0 }}>
        <MailPage />
      </SWRConfig>
    )
    await screen.findByText('roy@mail.towork')
    await user.click(screen.getByRole('button', { name: '写邮件' }))
    await user.type(screen.getByRole('textbox', { name: '收件人' }), 'nope{Enter}')
    expect(screen.getByRole('button', { name: '发送' })).toBeDisabled()
    expect(screen.getByText('请写成 name@mail.towork')).toBeInTheDocument()
    await user.click(screen.getByRole('button', { name: '移除 nope' }))
    expect(screen.queryByText('请写成 name@mail.towork')).not.toBeInTheDocument()

    await user.type(screen.getByRole('textbox', { name: '收件人' }), '18770221825@163.com{Enter}')
    expect(screen.getByRole('button', { name: '发送' })).toBeDisabled()
    expect(screen.getByText('这个邮箱只发给 @mail.towork。外部地址发不出去。')).toBeInTheDocument()
    expect(screen.queryByText('请写成 name@mail.towork')).not.toBeInTheDocument()
    await user.click(screen.getByRole('button', { name: '移除 18770221825@163.com' }))
    expect(screen.queryByText(/外部地址发不出去/)).not.toBeInTheDocument()

    await user.click(screen.getByRole('textbox', { name: '收件人' }))
    await user.paste(
      Array.from({ length: 21 }, (_, index) => `user-${String(index + 1).padStart(2, '0')}@mail.towork`).join(', ')
    )
    expect(screen.getByText('最多 20 个收件人')).toBeInTheDocument()
    expect(screen.getByRole('button', { name: '发送' })).toBeDisabled()
    await user.click(screen.getByRole('button', { name: '移除 user-21@mail.towork' }))
    expect(screen.queryByText('最多 20 个收件人')).not.toBeInTheDocument()
    await user.keyboard('{Escape}')
    await user.click(await screen.findByRole('button', { name: '放弃' }))
    await waitFor(() => expect(screen.queryByRole('dialog')).not.toBeInTheDocument())

    await user.click(screen.getByRole('button', { name: '写邮件' }))
    await user.type(screen.getByRole('textbox', { name: '收件人' }), 'missing@mail.towork')
    await user.click(screen.getByRole('button', { name: '发送' }))
    expect(await screen.findByText('没有这个邮箱')).toBeInTheDocument()
    expect(screen.getByRole('dialog')).toHaveAttribute('data-ui', 'mail.compose')
  })

  it('replies to everyone except the sender and continues a draft in place', async () => {
    const user = userEvent.setup()
    const posts: Array<{ url: string; body: string }> = []
    const thread = {
      ...summary,
      to: ['roy@mail.towork', 'ada@mail.towork'],
      cc: ['bea@mail.towork']
    }
    const draft = {
      ...summary,
      id: '0197e5a2-89f1-7d33-a11c-5f53d9b3af12',
      folder: 'drafts',
      subject: 'half',
      body: 'later',
      to: ['crates@mail.towork'],
      cc: ['ada@mail.towork'],
      unread: false
    }
    install(
      vi.fn(async (url: string, init?: RequestInit) => {
        const text = String(url)
        if (init?.method === 'POST') {
          posts.push({ url: text, body: String(init.body) })
          return envelope({ ...draft, id: '0197e5a2-89f1-7d33-a11c-5f53d9b3af99', folder: 'drafts' })
        }
        if (new URL(text).pathname === '/api/mail/mailbox')
          return envelope({ id: 'mb-1', address: 'roy@mail.towork', unread: 1 })
        if (new URL(text).pathname === `/api/mail/messages/${draft.id}`) return envelope(draft)
        if (text.includes('/api/mail/messages/')) return envelope(thread)
        const folder = new URL(text).searchParams.get('folder')
        if (folder === 'drafts') return envelope({ items: [draft], unread: 0 })
        return envelope({ items: [thread], unread: 1 })
      })
    )

    render(
      <SWRConfig value={{ provider: () => new Map(), dedupingInterval: 0 }}>
        <MailPage />
      </SWRConfig>
    )
    await user.click(await screen.findByRole('button', { name: /Successfully published/ }))
    await screen.findByRole('heading', { name: 'Successfully published a3s@0.22.1' })
    await user.click(screen.getByRole('button', { name: '全部回复' }))
    const dialog = screen.getByRole('dialog')
    expect(within(dialog).getByText('crates@mail.towork')).toBeInTheDocument()
    expect(within(dialog).getByText('ada@mail.towork')).toBeInTheDocument()
    expect(within(dialog).getByText('bea@mail.towork')).toBeInTheDocument()
    expect(within(dialog).queryByText('roy@mail.towork')).not.toBeInTheDocument()
    await user.keyboard('{Escape}')
    await user.click(await screen.findByRole('button', { name: '放弃' }))
    await waitFor(() => expect(screen.queryByRole('dialog')).not.toBeInTheDocument())

    await user.click(screen.getByRole('button', { name: '草稿箱' }))
    await user.click(await screen.findByRole('button', { name: /half/ }))
    await user.click(await screen.findByRole('button', { name: '继续写' }))
    const editor = screen.getByRole('dialog')
    expect(within(editor).getByText('crates@mail.towork')).toBeInTheDocument()
    expect(within(editor).getByText('ada@mail.towork')).toBeInTheDocument()
    expect(within(editor).getByRole('textbox', { name: '主题' })).toHaveValue('half')
    expect(within(editor).getByRole('textbox', { name: '正文' })).toHaveTextContent('later')
    await user.click(within(editor).getByRole('button', { name: '存草稿' }))

    await waitFor(() => expect(posts).toHaveLength(2))
    const kept = JSON.parse(posts[0]?.body ?? '{}') as { body?: string }
    expect(kept).toMatchObject({
      to: ['crates@mail.towork'],
      cc: ['ada@mail.towork'],
      subject: 'half',
      draft: true
    })
    expect(visibleMail(kept.body ?? '')).toBe('later')
    expect(posts[1]?.url).toContain(`/api/mail/messages/${draft.id}/state`)
    expect(JSON.parse(posts[1]?.body ?? '{}')).toMatchObject({ folder: 'trash' })
  })

  it('copies the mailbox address', async () => {
    const user = userEvent.setup()
    const writeText = vi.fn(async () => undefined)
    install(
      vi.fn(async (url: string) => {
        if (new URL(String(url)).pathname === '/api/mail/mailbox') {
          return envelope({ id: 'mb-1', address: 'roy@mail.towork', unread: 0 })
        }
        return envelope({ items: [], unread: 0 })
      })
    )
    Object.defineProperty(navigator, 'clipboard', { configurable: true, value: { writeText } })

    render(
      <SWRConfig value={{ provider: () => new Map(), dedupingInterval: 0 }}>
        <MailPage />
      </SWRConfig>
    )
    await screen.findByText('roy@mail.towork')
    await user.click(screen.getByRole('button', { name: '复制邮箱' }))
    expect(writeText).toHaveBeenCalledWith('roy@mail.towork')
    expect(toast.success).toHaveBeenCalledWith({
      title: '复制成功',
      appearance: 'pill',
      key: 'mail.copy-address'
    })
    expect(screen.queryByText('已复制')).not.toBeInTheDocument()
    expect(screen.getByRole('button', { name: '已复制' })).toBeInTheDocument()
  })

  it('uses a different empty state for a search that matches nothing', async () => {
    const user = userEvent.setup()
    install(
      vi.fn(async (url: string) => {
        const text = String(url)
        if (new URL(text).pathname === '/api/mail/mailbox') {
          return envelope({ id: 'mb-1', address: 'roy@mail.towork', unread: 0 })
        }
        const query = new URL(text).searchParams.get('q') ?? ''
        if (query) return envelope({ items: [], unread: 0 })
        return envelope({ items: [summary], unread: 0 })
      })
    )

    render(
      <SWRConfig value={{ provider: () => new Map(), dedupingInterval: 0 }}>
        <MailPage />
      </SWRConfig>
    )
    expect(await screen.findByText('选择一封邮件')).toBeInTheDocument()
    expect(screen.getByText('从列表打开一封，或使用左上角的写邮件')).toBeInTheDocument()
    expect(document.querySelector('[data-ui="mail.reader-empty"]')).not.toBeNull()

    await user.type(screen.getByRole('textbox', { name: '搜索邮件' }), 'missing')
    const empty = await screen.findByRole('button', { name: '清空搜索' })
    expect(
      within(document.querySelector('[data-ui="mail.no-match"]') as HTMLElement).getByText('没有找到相关邮件')
    ).toBeInTheDocument()
    await user.click(empty)
    await waitFor(() => expect(screen.getByRole('textbox', { name: '搜索邮件' })).toHaveValue(''))
    expect(await screen.findByText('Successfully published a3s@0.22.1')).toBeInTheDocument()
  })

  it('formats the message, keeps a safe link, and renders stored html as text', async () => {
    const user = userEvent.setup()
    const html = {
      ...summary,
      id: '0197e5a2-89f1-7d33-a11c-5f53d9b3af13',
      subject: 'rich',
      body: '<p>可见正文</p><script>alert(1)</script><a href="javascript:alert(1)">bad</a>'
    }
    install(
      vi.fn(async (url: string) => {
        const text = String(url)
        if (new URL(text).pathname === '/api/mail/mailbox')
          return envelope({ id: 'mb-1', address: 'roy@mail.towork', unread: 1 })
        if (text.includes(`/api/mail/messages/${html.id}`)) return envelope(html)
        return envelope({ items: [html], unread: 1 })
      })
    )

    render(
      <SWRConfig value={{ provider: () => new Map(), dedupingInterval: 0 }}>
        <MailPage />
      </SWRConfig>
    )
    await user.click(await screen.findByRole('button', { name: /rich/ }))
    const reading = await screen.findByText('可见正文')
    expect(reading.closest('[data-ui="mail.body"]')?.innerHTML ?? '').not.toContain('<script')
    expect(reading.closest('[data-ui="mail.body"]')?.innerHTML ?? '').not.toContain('javascript:')

    await user.click(screen.getByRole('button', { name: '写邮件' }))
    const dialog = screen.getByRole('dialog')
    expect(within(dialog).getByRole('toolbar', { name: '正文' })).toBeInTheDocument()
    const body = within(dialog).getByRole('textbox', { name: '正文' })
    await user.click(body)
    await user.click(within(dialog).getByRole('button', { name: '加粗' }))
    await user.keyboard('note')
    expect(body).toHaveTextContent('note')
    await user.click(within(dialog).getByRole('button', { name: '链接' }))
    const link = within(dialog).getByRole('textbox', { name: '链接地址' })
    await user.type(link, 'javascript:alert(1)')
    await user.click(within(dialog).getByRole('button', { name: '应用' }))
    expect(within(dialog).getByText('请写成 http 或 mailto 链接')).toBeInTheDocument()
    await user.keyboard('{Escape}')
    expect(within(dialog).queryByRole('textbox', { name: '链接地址' })).not.toBeInTheDocument()
    expect(dialog).toHaveAttribute('data-ui', 'mail.compose')

    await user.click(within(dialog).getByRole('button', { name: '链接' }))
    await user.type(within(dialog).getByRole('textbox', { name: '链接地址' }), 'https://example.com')
    await user.keyboard('{Enter}')
    expect(body.innerHTML).toContain('href="https://example.com"')
    expect(body.innerHTML).toMatch(/<strong>[^<]*note/)
    expect(body.innerHTML).not.toContain('javascript:')
  })

  it('opens a compose window instead of the dialog when the shell accepts the draft', async () => {
    const user = userEvent.setup()
    const invoke = vi.fn<(command: string, args?: unknown) => Promise<void>>(async () => undefined)
    ;(globalThis as { __TAURI_INTERNALS__?: { invoke: typeof invoke } }).__TAURI_INTERNALS__ = { invoke }
    install(
      vi.fn(async (url: string) => {
        if (new URL(String(url)).pathname === '/api/mail/mailbox')
          return envelope({ id: 'mb-1', address: 'roy@mail.towork', unread: 0 })
        return envelope({ items: [], unread: 0 })
      })
    )

    render(
      <SWRConfig value={{ provider: () => new Map(), dedupingInterval: 0 }}>
        <MailPage />
      </SWRConfig>
    )
    await screen.findByText('roy@mail.towork')
    await user.click(screen.getByRole('button', { name: '写邮件' }))

    expect(screen.queryByRole('dialog')).not.toBeInTheDocument()
    const payload = invoke.mock.calls[0]?.[1] as { draft?: string } | undefined
    const draft = JSON.parse(payload?.draft ?? '{}') as { kind?: string; from?: string; focus?: string; title?: string }
    expect(invoke.mock.calls[0]?.[0]).toBe('open_mail_compose_window')
    expect(draft).toMatchObject({ kind: 'new', from: 'roy@mail.towork', focus: 'to', title: '写邮件' })
  })

  it('keeps more than one signature and inserts the chosen one', async () => {
    const user = userEvent.setup()
    install(
      vi.fn(async (url: string) => {
        if (new URL(String(url)).pathname === '/api/mail/mailbox')
          return envelope({ id: 'mb-1', address: 'roy@mail.towork', unread: 0 })
        return envelope({ items: [], unread: 0 })
      })
    )

    render(
      <SWRConfig value={{ provider: () => new Map(), dedupingInterval: 0 }}>
        <MailPage />
      </SWRConfig>
    )
    await screen.findByText('roy@mail.towork')
    await user.click(screen.getByRole('button', { name: '写邮件' }))
    const dialog = screen.getByRole('dialog')
    const body = within(dialog).getByRole('textbox', { name: '正文' })
    await user.click(body)
    await user.keyboard('林书豪')
    await user.click(within(dialog).getByRole('button', { name: '签名' }))
    expect(within(dialog).getByText('还没有签名')).toBeInTheDocument()

    await user.click(within(dialog).getByRole('button', { name: '新建签名' }))
    await user.type(within(dialog).getByRole('textbox', { name: '签名名称' }), '工作')
    await user.type(within(dialog).getByRole('textbox', { name: '签名内容' }), 'Roy')
    await user.click(within(dialog).getByRole('button', { name: '保存签名' }))

    await user.click(within(dialog).getByRole('button', { name: '新建签名' }))
    await user.type(within(dialog).getByRole('textbox', { name: '签名名称' }), '个人')
    await user.type(within(dialog).getByRole('textbox', { name: '签名内容' }), 'Lin')
    await user.click(within(dialog).getByRole('button', { name: '保存签名' }))

    expect(within(dialog).getByText('工作')).toBeInTheDocument()
    expect(within(dialog).getByText('个人')).toBeInTheDocument()
    expect(within(dialog).getAllByText('默认')).toHaveLength(1)

    const inserts = within(dialog).getAllByRole('button', { name: /插入/ })
    await user.click(inserts[1] ?? inserts[0])
    expect(body).toHaveTextContent('林书豪')
    expect(body).toHaveTextContent('Lin')
    expect((body.textContent ?? '').indexOf('林书豪')).toBeLessThan((body.textContent ?? '').indexOf('Lin'))
    expect(body.innerHTML).toContain('<mail-signature>')
    expect(JSON.parse(localStorage.getItem('mail.signatures.v1') ?? '[]')).toHaveLength(2)
  })

  it('shows action icons and asks before deleting a message', async () => {
    const user = userEvent.setup()
    const states: string[] = []
    install(
      vi.fn(async (url: string, init?: RequestInit) => {
        const text = String(url)
        if (text.endsWith('/state') && init?.method === 'POST') {
          states.push(String(init.body))
          return envelope({ ...summary, unread: false, folder: 'trash' })
        }
        if (new URL(text).pathname === '/api/mail/mailbox')
          return envelope({ id: 'mb-1', address: 'roy@mail.towork', unread: 1 })
        if (text.includes(`/api/mail/messages/${summary.id}`)) return envelope({ ...summary, unread: false })
        return envelope({ items: [summary], unread: 1 })
      })
    )

    render(
      <SWRConfig value={{ provider: () => new Map(), dedupingInterval: 0 }}>
        <MailPage />
      </SWRConfig>
    )
    await user.click(await screen.findByRole('button', { name: /Successfully published/ }))
    await screen.findByRole('heading', { name: 'Successfully published a3s@0.22.1' })
    const actions = document.querySelector('[data-ui="mail.actions"]') as HTMLElement
    expect(within(actions).getByRole('button', { name: '回复' }).querySelector('svg')).not.toBeNull()
    expect(within(actions).getByRole('button', { name: '标为红旗' }).querySelector('svg')).not.toBeNull()
    expect(within(actions).getByRole('button', { name: '标为未读' }).querySelector('svg')).not.toBeNull()
    expect(within(actions).getByRole('button', { name: '标为垃圾邮件' }).querySelector('svg')).not.toBeNull()

    await user.click(within(actions).getByRole('button', { name: '删除' }))
    expect(states).toEqual([])
    const confirm = await screen.findByRole('dialog', { name: '删除这封邮件？' })
    expect(within(confirm).getByText('邮件会移到已删除。')).toBeInTheDocument()
    await user.click(within(confirm).getByRole('button', { name: '取消' }))
    expect(screen.queryByRole('dialog', { name: '删除这封邮件？' })).not.toBeInTheDocument()
    expect(states).toEqual([])

    await user.click(within(actions).getByRole('button', { name: '删除' }))
    await user.click(
      within(await screen.findByRole('dialog', { name: '删除这封邮件？' })).getByRole('button', { name: '删除' })
    )
    await waitFor(() => expect(states.map((body) => JSON.parse(body))).toEqual([{ folder: 'trash', org_id: ORG }]))
  })

  it('opens ToBot mailbox controls from the task window corner', async () => {
    const user = userEvent.setup()
    install(
      vi.fn(async (url: string) => {
        const text = String(url)
        if (new URL(text).pathname === '/api/mail/mailbox')
          return envelope({ id: 'mb-1', address: 'roy@mail.towork', unread: 1 })
        return envelope({ items: [summary], unread: 1 })
      })
    )

    render(
      <SWRConfig value={{ provider: () => new Map(), dedupingInterval: 0 }}>
        <InboxHome initialLane="mail" />
      </SWRConfig>
    )
    await screen.findByText('roy@mail.towork')
    const nav = document.querySelector('[data-ui="mail.folders"]') as HTMLElement
    expect(within(nav).queryByRole('button', { name: 'ToBot 管理' })).not.toBeInTheDocument()
    const button = screen.getByRole('button', { name: 'ToBot 管理' })
    expect(button).toHaveClass('ml-auto', 'shrink-0', 'whitespace-nowrap')
    await user.click(button)
    const dialog = await screen.findByRole('dialog', { name: '允许 ToBot 管理' })
    expect(within(dialog).getByText('ToBot 不能代发、删除或修改邮件。')).toBeInTheDocument()
    expect(within(dialog).getByText('只有团队角色允许修改邮件状态时，打开未读邮件才会标为已读。')).toBeInTheDocument()
    const allow = await waitFor(() => {
      const control = within(dialog).getByRole('switch', { name: '允许管理' })
      expect(control).toBeEnabled()
      return control
    })
    expect(allow).toHaveAttribute('aria-checked', 'false')
    expect(within(dialog).getByRole('switch', { name: '查看列表' })).toBeDisabled()
    await user.click(allow)
    expect(JSON.parse(localStorage.getItem('mail.tobot.v1') ?? '{}')).toMatchObject({
      allow: true,
      list: true,
      read: false
    })
    expect(within(dialog).getByRole('switch', { name: '查看列表' })).toBeEnabled()
  })

  it('lets ToBot manage every task kind from the same corner', async () => {
    const user = userEvent.setup()
    vi.stubGlobal(
      'fetch',
      vi.fn(async () => new Response(JSON.stringify({ success: true, code: 0, data: [] }), { status: 200 }))
    )

    render(
      <SWRConfig value={{ provider: () => new Map(), dedupingInterval: 0 }}>
        <InboxHome />
      </SWRConfig>
    )
    const button = screen.getByRole('button', { name: 'ToBot 管理' })
    expect(button).toBeEnabled()
    await user.click(button)
    const dialog = await screen.findByRole('dialog', { name: '允许 ToBot 管理' })
    for (const name of ['加入团队', '流程审批', '请假', '出差', '报销', '会议', '用印', '公告']) {
      expect(within(dialog).getByRole('button', { name })).toBeInTheDocument()
    }
    expect(within(dialog).getByText('ToBot 不能代批、驳回、撤销或新建任务。')).toBeInTheDocument()
    expect(within(dialog).getByRole('switch', { name: '允许管理' })).toBeDisabled()
    const allow = within(dialog).getByRole('switch', { name: '管理任务' })
    expect(allow).toHaveAttribute('aria-checked', 'false')
    expect(within(dialog).getByRole('switch', { name: '查看任务' })).toBeDisabled()
    await user.click(allow)
    expect(JSON.parse(localStorage.getItem('work.tobot.v1') ?? '{}')).toEqual({
      allow: true,
      list: true,
      read: false,
      kinds: ['join_request', 'approval', 'leave', 'travel', 'expense', 'meeting', 'seal', 'notice']
    })
    expect(within(dialog).getByRole('switch', { name: '查看任务' })).toBeEnabled()
    await user.click(within(dialog).getByRole('button', { name: '公告' }))
    expect(JSON.parse(localStorage.getItem('work.tobot.v1') ?? '{}').kinds).toEqual([
      'join_request',
      'approval',
      'leave',
      'travel',
      'expense',
      'meeting',
      'seal'
    ])
  })
})
