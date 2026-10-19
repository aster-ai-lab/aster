import { act, cleanup, render, screen, waitFor, within } from '@testing-library/react'
import userEvent from '@testing-library/user-event'
import { SWRConfig, useSWRConfig } from 'swr'
import { afterEach, describe, expect, it, vi } from 'vitest'

import { EMPTY_COMPOSE, teamMailOrgId } from '../mailCompose'
import { MailComposeSurface } from '../MailComposeSurface'

const TOKEN = 'cd'.repeat(32)
const ORG = '0197e5a2-89f1-7d33-a11c-5f53d9b3af10'
const ADA = '0197e5a2-89f1-7d33-a11c-5f53d9b3af21'
const teamPolicy = {
  id: ORG,
  name: 'Team',
  can_read_mail_recipients: true,
  can_read_mail: true,
  can_create_mail: true,
  can_send_mail: true,
  can_update_mail: true,
  can_delete_mail: true,
  rules: [
    { action: 'read', subject: 'MailRecipient' },
    ...['read', 'create', 'send', 'update', 'delete'].map((action) => ({ action, subject: 'MailMessage' }))
  ],
  data_scope: { kind: 'department_tree', department_ids: [] }
}
let policy: Record<string, unknown> = teamPolicy
let refreshTeams: () => Promise<unknown>
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

function envelope(data: unknown): Response {
  return new Response(JSON.stringify({ success: true, code: 0, message: 'ok', data }), { status: 200 })
}

afterEach(async () => {
  cleanup()
  // Tiptap defers editor destruction; finish it before the DOM environment exits.
  await new Promise<void>((resolve) => setTimeout(resolve, 1))
  vi.unstubAllGlobals()
  delete (globalThis as { __A3S_STUDIO_RUNTIME__?: unknown }).__A3S_STUDIO_RUNTIME__
  localStorage.removeItem('mail.signatures.v1')
  policy = teamPolicy
})

function install(fetchMock: (url: string, init?: RequestInit) => Promise<Response>): void {
  Object.defineProperty(globalThis, '__A3S_STUDIO_RUNTIME__', {
    configurable: true,
    value: { apiBaseUrl: 'http://127.0.0.1:9', token: TOKEN }
  })
  vi.stubGlobal('fetch', (url: Parameters<typeof fetch>[0], init?: RequestInit) =>
    String(url).endsWith('/api/member-orgs') ? Promise.resolve(envelope([policy])) : fetchMock(String(url), init)
  )
}

function ComposeAuthority({ children }: { children: React.ReactNode }) {
  const { mutate } = useSWRConfig()
  refreshTeams = () => mutate('studio/member-orgs')
  return children
}

function renderCompose(teamOrgId?: string, onSent: () => void = () => undefined) {
  return render(
    <SWRConfig value={{ provider: () => new Map(), dedupingInterval: 0 }}>
      <ComposeAuthority>
        <MailComposeSurface
          address="roy@mail.towork"
          initial={EMPTY_COMPOSE}
          focusField="to"
          showTitle={false}
          teamOrgId={teamOrgId}
          onSent={onSent}
          onClose={() => undefined}
        />
      </ComposeAuthority>
    </SWRConfig>
  )
}

describe('teamMailOrgId', () => {
  it('accepts an organization id and refuses the personal center', () => {
    expect(teamMailOrgId(ORG)).toBe(ORG)
    expect(teamMailOrgId('personal')).toBeUndefined()
    expect(teamMailOrgId('')).toBeUndefined()
    expect(teamMailOrgId('not-a-uuid')).toBeUndefined()
  })
})

describe('MailComposeSurface', () => {
  it('allows saving drafts independently of sending and refuses shortcuts after revocation', async () => {
    const user = userEvent.setup()
    const posts: Array<Record<string, unknown>> = []
    policy = { ...teamPolicy, can_send_mail: false }
    install(async (_url, init) => {
      if (init?.method === 'POST') posts.push(JSON.parse(String(init.body)))
      return envelope({ id: 'saved', created_at: '2026-10-09T00:00:00Z', body: '' })
    })
    renderCompose(ORG)
    await screen.findByRole('button', { name: '选择团队成员' })
    await user.type(screen.getByRole('textbox', { name: '收件人' }), 'ada@mail.towork{Enter}')
    expect(screen.getByRole('button', { name: '发送' })).toBeDisabled()
    expect(screen.getByRole('button', { name: '存草稿' })).toBeEnabled()
    await user.keyboard('{Meta>}{Enter}{/Meta}')
    expect(posts).toEqual([])
    await user.click(screen.getByRole('button', { name: '存草稿' }))
    await waitFor(() => expect(posts).toHaveLength(1))
    expect(posts[0]).toMatchObject({ org_id: ORG, draft: true, to: ['ada@mail.towork'] })
    policy = { ...teamPolicy, can_create_mail: false }
    await act(async () => {
      await refreshTeams()
    })
    await user.type(screen.getByRole('textbox', { name: '收件人' }), 'ada@mail.towork{Enter}')
    expect(screen.getByRole('button', { name: '发送' })).toBeEnabled()
    expect(screen.getByRole('button', { name: '存草稿' })).toBeDisabled()
    await user.keyboard('{Meta>}s{/Meta}')
    expect(posts).toHaveLength(1)
  })

  it('disposes a revoked letter and refuses a late send result even after regranting', async () => {
    const user = userEvent.setup()
    let finish: ((response: Response) => void) | undefined
    let accepted = 0
    install(async (_url, init) => {
      if (init?.method === 'POST')
        return new Promise<Response>((done) => {
          finish = done
        })
      return envelope([])
    })
    renderCompose(ORG, () => {
      accepted += 1
    })
    await screen.findByRole('button', { name: '选择团队成员' })
    await user.type(screen.getByRole('textbox', { name: '收件人' }), 'ada@mail.towork{Enter}')
    await user.type(screen.getByRole('textbox', { name: '主题' }), 'Old letter')
    await user.click(screen.getByRole('button', { name: '发送' }))
    await waitFor(() => expect(finish).toBeDefined())
    policy = { ...teamPolicy, can_create_mail: false, can_send_mail: false }
    await act(async () => {
      await refreshTeams()
    })
    expect(screen.getByRole('textbox', { name: '主题' })).toHaveValue('')
    policy = teamPolicy
    await act(async () => {
      await refreshTeams()
    })
    await act(async () => {
      finish?.(envelope({ id: 'old-result', body: 'Old body', created_at: '2026-10-09T00:00:00Z' }))
    })
    expect(accepted).toBe(0)
    expect(screen.getByRole('textbox', { name: '主题' })).toHaveValue('')
    expect(screen.getByRole('button', { name: '发送' })).toBeDisabled()
  })

  it('discards late directory rows after an allowed department scope changes', async () => {
    const user = userEvent.setup()
    let complete: ((response: Response) => void) | undefined
    let requests = 0
    install(
      vi.fn(async () => {
        requests += 1
        if (requests === 1)
          return new Promise<Response>((done) => {
            complete = done
          })
        return envelope([{ user_id: 'other-user', name: '王五', position: '财务' }])
      })
    )
    renderCompose(ORG)
    await user.click(await screen.findByRole('button', { name: '选择团队成员' }))
    await waitFor(() => expect(complete).toBeDefined())
    policy = { ...teamPolicy, data_scope: { kind: 'custom', department_ids: ['finance'] } }
    await act(async () => {
      await refreshTeams()
    })
    await waitFor(() => expect(screen.queryByRole('dialog', { name: '选择收件人' })).not.toBeInTheDocument())
    await act(async () => {
      complete?.(envelope([{ user_id: ADA, name: '李四', position: '工程' }]))
    })
    await user.click(await screen.findByRole('button', { name: '选择团队成员' }))
    const picker = await screen.findByRole('dialog', { name: '选择收件人' })
    expect(await within(picker).findByRole('button', { name: /王五/ })).toBeInTheDocument()
    expect(within(picker).queryByRole('button', { name: /李四/ })).not.toBeInTheDocument()
  })

  it('refuses missing server capability and admin flags without recipient permission', async () => {
    const network = vi.fn()
    policy = { ...teamPolicy, can_read_mail_recipients: undefined, membership: 'admin' }
    install(network)
    renderCompose(ORG)
    await act(async () => {
      await refreshTeams()
    })
    expect(screen.queryByRole('button', { name: '选择团队成员' })).not.toBeInTheDocument()
    policy = { ...teamPolicy, rules: [], membership: 'admin' }
    await act(async () => {
      await refreshTeams()
    })
    expect(screen.queryByRole('button', { name: '选择团队成员' })).not.toBeInTheDocument()
    expect(network).not.toHaveBeenCalled()
  })

  it('closes a revoked picker and ignores a late mailbox resolution', async () => {
    const user = userEvent.setup()
    let resolve: ((response: Response) => void) | undefined
    install(
      vi.fn(async (_url: string, init?: RequestInit) => {
        if (init?.method === 'POST')
          return new Promise<Response>((done) => {
            resolve = done
          })
        return envelope([{ user_id: ADA, name: '李四', position: '工程' }])
      })
    )
    renderCompose(ORG)
    await user.click(await screen.findByRole('button', { name: '选择团队成员' }))
    const picker = await screen.findByRole('dialog', { name: '选择收件人' })
    await user.click(await within(picker).findByRole('button', { name: /李四/ }))
    await user.click(within(picker).getByRole('button', { name: '添加' }))
    await waitFor(() => expect(resolve).toBeDefined())
    policy = { ...teamPolicy, can_read_mail_recipients: false }
    await act(async () => {
      await refreshTeams()
    })
    await waitFor(() => expect(screen.queryByRole('dialog', { name: '选择收件人' })).not.toBeInTheDocument())
    expect(screen.queryByRole('button', { name: '选择团队成员' })).not.toBeInTheDocument()
    await act(async () => {
      resolve?.(envelope([{ user_id: ADA, name: '李四', address: 'lisi@mail.towork' }]))
    })
    expect(screen.queryByText('李四')).not.toBeInTheDocument()
    policy = teamPolicy
    await act(async () => {
      await refreshTeams()
    })
    await user.click(await screen.findByRole('button', { name: '选择团队成员' }))
    expect(await screen.findByRole('dialog', { name: '选择收件人' })).toBeInTheDocument()
    expect(screen.queryByRole('button', { name: '添加' })).toBeDisabled()
  })

  it('labels the address fields and keeps teammate choice out of the personal center', async () => {
    const user = userEvent.setup()
    const fetchMock = vi.fn()
    install(fetchMock)
    renderCompose()

    expect(screen.getByRole('textbox', { name: '收件人' })).toHaveAttribute('placeholder', '姓名或 name@mail.towork')
    expect(screen.getByRole('textbox', { name: '主题' })).toHaveAttribute('placeholder', '这封邮件的主题')
    expect(screen.queryByRole('textbox', { name: '抄送' })).not.toBeInTheDocument()
    expect(screen.queryByRole('button', { name: '选择团队成员' })).not.toBeInTheDocument()

    await user.click(screen.getByRole('button', { name: '抄送' }))
    expect(screen.getByRole('textbox', { name: '抄送' })).toHaveAttribute('placeholder', 'name@mail.towork')
    expect(fetchMock).not.toHaveBeenCalled()
  })

  it('resolves a selected teammate into a named recipient', async () => {
    const user = userEvent.setup()
    const posts: string[] = []
    install(
      vi.fn(async (url: string, init?: RequestInit) => {
        const text = String(url)
        if (text.includes('/api/mail/colleagues') && init?.method === 'POST') {
          posts.push(String(init.body))
          return envelope([{ user_id: ADA, name: '李四', address: 'lisi@mail.towork' }])
        }
        if (text.includes('/api/mail/colleagues')) {
          return envelope([
            { user_id: ADA, name: '李四', position: '工程' },
            { user_id: '0197e5a2-89f1-7d33-a11c-5f53d9b3af22', name: '王五', position: '设计' }
          ])
        }
        return envelope({})
      })
    )
    renderCompose(ORG)

    await user.click(await screen.findByRole('button', { name: '选择团队成员' }))
    const picker = await screen.findByRole('dialog', { name: '选择收件人' })
    expect(within(picker).getByRole('button', { name: /李四/ })).toBeInTheDocument()
    expect(within(picker).getByText('工程')).toBeInTheDocument()
    await user.click(within(picker).getByRole('button', { name: /李四/ }))
    await user.click(within(picker).getByRole('button', { name: '添加' }))

    await waitFor(() => expect(posts).toHaveLength(1))
    expect(JSON.parse(posts[0] ?? '{}')).toEqual({ org_id: ORG, user_ids: [ADA] })
    const chip = await screen.findByText('李四')
    expect(chip).toHaveAttribute('title', 'lisi@mail.towork')
    await waitFor(() => expect(screen.queryByRole('dialog', { name: '选择收件人' })).not.toBeInTheDocument())
    expect(screen.queryByText('lisi@mail.towork')).not.toBeInTheDocument()
  })

  it('shows a saved photo or the two-character mark for recipients and copies', async () => {
    const user = userEvent.setup()
    const photo = 'data:image/webp;base64,AA'
    install(
      vi.fn(async (url: string) => {
        if (String(url).includes('/api/mail/colleagues')) {
          return envelope([
            { user_id: ADA, name: '温和的风筝', position: '', avatar: photo },
            { user_id: '0197e5a2-89f1-7d33-a11c-5f53d9b3af22', name: '林书豪', position: '财务总监' },
            {
              user_id: '0197e5a2-89f1-7d33-a11c-5f53d9b3af23',
              name: 'Leon',
              position: '',
              avatar: 'https://example.com/a.png'
            }
          ])
        }
        return envelope({})
      })
    )
    renderCompose(ORG)

    const expectFaces = async () => {
      const picker = await screen.findByRole('dialog', { name: '选择收件人' })
      const image = within(picker).getByRole('img', { name: '温和的风筝' })
      expect(image).toHaveAttribute('data-kind', 'image')
      expect(image.querySelector('img')).toHaveAttribute('src', photo)
      const lin = within(picker).getByRole('img', { name: '林书豪' })
      expect(lin).toHaveAttribute('data-kind', 'text')
      expect(lin).toHaveTextContent('林书')
      expect(within(picker).getByRole('img', { name: 'Leon' })).toHaveTextContent('LE')
      expect(within(picker).queryByRole('img', { name: '温' })).not.toBeInTheDocument()
      await user.click(within(picker).getByRole('button', { name: '取消' }))
    }

    await user.click(await screen.findByRole('button', { name: '选择团队成员' }))
    await expectFaces()
    await user.click(screen.getByRole('button', { name: '抄送' }))
    await user.click(screen.getByRole('button', { name: '抄送 选择团队成员' }))
    await expectFaces()
  })

  it('offers a matching teammate while the name is typed', async () => {
    const user = userEvent.setup()
    install(
      vi.fn(async (url: string, init?: RequestInit) => {
        if (String(url).includes('/api/mail/colleagues') && init?.method === 'POST') {
          return envelope([{ user_id: ADA, name: '李四', address: 'lisi@mail.towork' }])
        }
        if (String(url).includes('/api/mail/colleagues')) {
          return envelope([{ user_id: ADA, name: '李四', position: '工程' }])
        }
        return envelope({})
      })
    )
    renderCompose(ORG)
    await screen.findByRole('button', { name: '选择团队成员' })
    await user.type(screen.getByRole('textbox', { name: '收件人' }), '李')
    const list = await screen.findByRole('listbox')
    await user.click(within(list).getByRole('option', { name: /李四/ }))
    expect(await screen.findByText('李四')).toHaveAttribute('title', 'lisi@mail.towork')
  })

  it('saves a draft with the shortcut and opens copy with its shortcut', async () => {
    const user = userEvent.setup()
    const posts: string[] = []
    install(
      vi.fn(async (url: string, init?: RequestInit) => {
        if (String(url).includes('/api/mail/messages') && init?.method === 'POST') {
          posts.push(String(init.body))
          return envelope({ id: 'm-1', created_at: '2026-10-07T00:00:00Z', body: '' })
        }
        return envelope({})
      })
    )
    renderCompose(ORG)
    await screen.findByRole('button', { name: '选择团队成员' })
    await user.type(screen.getByRole('textbox', { name: '主题' }), 'hello')
    await user.keyboard('{Meta>}{Shift>}c{/Shift}{/Meta}')
    expect(screen.getByRole('textbox', { name: '抄送' })).toBeInTheDocument()
    await user.keyboard('{Meta>}s{/Meta}')
    await waitFor(() => expect(posts).toHaveLength(1))
    expect(JSON.parse(posts[0] ?? '{}')).toMatchObject({ subject: 'hello', draft: true })
  })

  it('appends a signature after the letter and replaces only the signature', async () => {
    const user = userEvent.setup()
    renderCompose()
    const body = screen.getByRole('textbox', { name: '正文' })
    await user.click(body)
    await user.keyboard('林书豪')
    await user.click(screen.getByRole('button', { name: '签名' }))
    await user.click(screen.getByRole('button', { name: '新建签名' }))
    await user.type(screen.getByRole('textbox', { name: '签名名称' }), '工作')
    await user.type(screen.getByRole('textbox', { name: '签名内容' }), 'Roy')
    await user.click(screen.getByRole('button', { name: '保存签名' }))
    await user.click(screen.getByRole('button', { name: /插入/ }))

    expect(body).toHaveTextContent('林书豪')
    expect(body).toHaveTextContent('Roy')
    expect((body.textContent ?? '').indexOf('林书豪')).toBeLessThan((body.textContent ?? '').indexOf('Roy'))

    await user.click(screen.getByRole('button', { name: '签名' }))
    await user.click(screen.getByRole('button', { name: '新建签名' }))
    await user.type(screen.getByRole('textbox', { name: '签名名称' }), '个人')
    await user.type(screen.getByRole('textbox', { name: '签名内容' }), 'Lin')
    await user.click(screen.getByRole('button', { name: '保存签名' }))
    const inserts = screen.getAllByRole('button', { name: /插入/ })
    await user.click(inserts[1] ?? inserts[0])
    expect(body).toHaveTextContent('林书豪')
    expect(body).toHaveTextContent('Lin')
    expect(body).not.toHaveTextContent('Roy')
  })

  it('keeps signature editing behind manage', async () => {
    const user = userEvent.setup()
    renderCompose()
    await user.click(screen.getByRole('button', { name: '签名' }))
    await user.click(screen.getByRole('button', { name: '新建签名' }))
    await user.type(screen.getByRole('textbox', { name: '签名名称' }), 'AI Lab')
    await user.type(screen.getByRole('textbox', { name: '签名内容' }), 'Roy')
    await user.click(screen.getByRole('button', { name: '保存签名' }))

    expect(screen.getByRole('button', { name: /插入/ })).toBeInTheDocument()
    expect(screen.queryByRole('button', { name: '删除签名' })).not.toBeInTheDocument()
    await user.click(screen.getByRole('button', { name: '管理签名' }))
    expect(screen.getByRole('button', { name: '删除签名' })).toBeInTheDocument()
    expect(screen.getByRole('button', { name: '编辑签名' })).toBeInTheDocument()
  })
})
