// @vitest-environment jsdom
import '@testing-library/jest-dom/vitest'
import { cleanup, fireEvent, render, screen, waitFor } from '@testing-library/react'
import { afterEach, describe, expect, it, vi } from 'vitest'

import { type DiscoverableOrg, type MemberAccount, type MemberOrg, MemberOrgError } from '@renderer/data/memberOrgs'

import { AccountCenterMenu } from './AccountCenterMenu'
import { textLogo } from './entityImage'

const copy: Record<string, string> = {
  'common.cancel': '取消',
  'common.loading': '加载中...',
  'common.retry': '重试',
  'common.save': '保存',
  'sidebar.center.edit_profile': '编辑资料',
  'sidebar.center.nickname': '昵称',
  'sidebar.center.nickname_required': '请填写昵称',
  'sidebar.center.session_expired': '登录已失效，请重新登录',
  'sidebar.center.sign_in_again': '重新登录',
  'sidebar.center.admin': '管理员',
  'sidebar.center.already_member': '你已经在这个团队里',
  'sidebar.center.create': '创建团队',
  'sidebar.center.create_title': '创建团队',
  'sidebar.center.join': '加入团队',
  'sidebar.center.join_apply': '申请加入',
  'sidebar.center.join_apply_done': '等待管理员通过后即可加入',
  'sidebar.center.join_apply_done_title': '申请已提交',
  'sidebar.center.join_apply_pending': '管理员拒绝后才能再次申请',
  'sidebar.center.join_apply_pending_title': '无法再次申请',
  'sidebar.center.join_apply_error': '申请失败，请稍后重试',
  'sidebar.center.join_applied': '已申请',
  'sidebar.center.join_code': '邀请码',
  'sidebar.center.join_joined': '已加入',
  'sidebar.center.join_members': '{{count}} 人',
  'sidebar.center.join_search': '团队名称',
  'sidebar.center.join_search_empty': '没有找到团队',
  'sidebar.center.join_search_hint': '输入名称查找可加入的团队',
  'sidebar.center.join_search_placeholder': '输入团队名称',
  'sidebar.center.join_search_tab': '搜索团队',
  'sidebar.center.join_disabled': '邀请已停用或已过期',
  'sidebar.center.join_error': '加入失败，请稍后重试',
  'sidebar.center.join_invalid': '请输入有效的邀请码或邀请链接',
  'sidebar.center.join_not_found': '找不到这个团队',
  'sidebar.center.join_placeholder': '粘贴邀请码或邀请链接',
  'sidebar.center.join_required': '请填写邀请码',
  'sidebar.center.join_submit': '加入',
  'sidebar.center.join_title': '加入团队',
  'sidebar.center.creator': '创建人',
  'sidebar.center.empty': '还没有团队',
  'sidebar.center.error': '无法加载团队',
  'sidebar.center.member': '成员',
  'sidebar.center.name': '名称',
  'sidebar.center.name_placeholder': '例如：某某科技有限公司',
  'sidebar.center.name_required': '请填写名称',
  'sidebar.center.name_taken': '这个名称已被使用',
  'sidebar.center.logout': '退出登录',
  'sidebar.center.personal': '个人',
  'sidebar.center.personal_label': '个人中心',
  'sidebar.center.phone': '手机号',
  'sidebar.center.switch': '切换中心',
  'sidebar.center.teams': '团队'
}

vi.mock('react-i18next', () => ({
  useTranslation: () => ({
    t: (key: string, values?: Record<string, unknown>) => {
      const template = copy[key] ?? key
      if (!values) return template
      return template.replace(/\{\{(\w+)\}\}/g, (_match, name: string) => String(values[name] ?? ''))
    }
  })
}))

const orgs: MemberOrg[] = [
  { id: 'org-1', name: '通友科技', logo: '', membership: 'admin', creator: true },
  { id: 'org-2', name: '协作组', logo: '', membership: 'member' },
  { id: 'org-3', name: '待审核', logo: '', membership: 'pending' }
]

function renderMenu({
  loadOrgs = vi.fn(async () => orgs),
  createOrg = vi.fn(async () => orgs[0]),
  joinOrg = vi.fn(async () => orgs[0]),
  searchOrgs = vi.fn(async () => []),
  requestJoin = vi.fn(async () => undefined),
  onOpenChange = vi.fn(),
  avatar,
  fallbackName,
  loadAccount = vi.fn(async () => ({ displayName: '', phone: '' })),
  logoutAccount = vi.fn(async () => undefined),
  updateAccount = vi.fn(async (profile: { displayName: string; avatar: string }) => ({
    displayName: profile.displayName,
    phone: '',
    avatar: profile.avatar
  })),
  onSignedOut = vi.fn(),
  openWork = vi.fn()
}: {
  openWork?: () => void
  loadOrgs?: () => Promise<MemberOrg[]>
  createOrg?: (name: string, logo?: string) => Promise<MemberOrg>
  joinOrg?: (code: string) => Promise<MemberOrg>
  searchOrgs?: (query: string) => Promise<DiscoverableOrg[]>
  requestJoin?: (orgId: string) => Promise<void>
  updateAccount?: (profile: { displayName: string; avatar: string }) => Promise<MemberAccount>
  onOpenChange?: (open: boolean) => void
  avatar?: string
  fallbackName?: string
  loadAccount?: () => Promise<MemberAccount>
  logoutAccount?: () => Promise<void>
  onSignedOut?: () => void
} = {}) {
  const anchor = document.createElement('button')
  anchor.getBoundingClientRect = () =>
    ({
      top: 20,
      left: 12,
      right: 48,
      bottom: 56,
      width: 36,
      height: 36,
      x: 12,
      y: 20,
      toJSON() {
        return {}
      }
    }) as DOMRect
  document.body.appendChild(anchor)
  const view = render(
    <AccountCenterMenu
      open
      anchor={anchor}
      onOpenChange={onOpenChange}
      loadOrgs={loadOrgs}
      createOrg={createOrg}
      joinOrg={joinOrg}
      searchOrgs={searchOrgs}
      requestJoin={requestJoin}
      avatar={avatar}
      fallbackName={fallbackName}
      loadAccount={loadAccount}
      logoutAccount={logoutAccount}
      updateAccount={updateAccount}
      onSignedOut={onSignedOut}
      openWork={openWork}
    />
  )
  return {
    ...view,
    anchor,
    loadOrgs,
    createOrg,
    joinOrg,
    searchOrgs,
    requestJoin,
    onOpenChange,
    logoutAccount,
    updateAccount,
    onSignedOut,
    openWork
  }
}

afterEach(() => {
  cleanup()
  localStorage.clear()
  document.cookie = 'a3s.org.center=; Path=/; Max-Age=0'
  document.body.innerHTML = ''
})

describe('AccountCenterMenu', () => {
  it('lists joined teams beside the avatar and hides pending memberships', async () => {
    localStorage.setItem('a3s.org.center', 'org-1')
    renderMenu()

    const menu = await screen.findByRole('dialog', { name: '切换中心' })
    expect(menu).toHaveStyle({ left: '56px', top: '20px', width: '248px' })
    expect(screen.getByRole('tab', { name: '团队', selected: true })).toBeInTheDocument()
    expect(screen.getByRole('tab', { name: '个人' })).toBeInTheDocument()
    const current = await screen.findByRole('menuitemradio', { name: /通友科技/ })
    expect(current).toHaveAttribute('aria-checked', 'true')
    expect(current.className).toContain('bg-[#f2f3f5]')
    expect(current.className).toContain('rounded-[8px]')
    expect(menu.className).toContain('rounded-[14px]')
    expect(screen.getByRole('tab', { name: '团队' }).className).toContain('bg-white')
    expect(current).toHaveTextContent('创建人')
    expect(screen.getByRole('menuitemradio', { name: /协作组/ })).toHaveTextContent('成员')
    expect(screen.queryByText('待审核')).not.toBeInTheDocument()
    expect(screen.getByRole('button', { name: '创建团队' })).toBeInTheDocument()
  })

  it('checks the personal center and stores the team that was chosen', async () => {
    const onOpenChange = vi.fn()
    renderMenu({ onOpenChange })

    fireEvent.click(await screen.findByRole('tab', { name: '个人' }))
    expect(screen.getByRole('button', { name: '加入团队' })).toBeInTheDocument()
    expect(screen.queryByRole('button', { name: '创建团队' })).not.toBeInTheDocument()
    const personal = screen.getByRole('menuitemradio', { name: '个人中心' })
    expect(personal).toHaveAttribute('aria-checked', 'true')
    expect(screen.queryByRole('menuitemradio', { name: /通友科技/ })).not.toBeInTheDocument()

    fireEvent.click(screen.getByRole('tab', { name: '团队' }))
    fireEvent.click(await screen.findByRole('menuitemradio', { name: /协作组/ }))
    expect(localStorage.getItem('a3s.org.center')).toBe('org-2')
    expect(onOpenChange).toHaveBeenCalledWith(false)
  })

  it('opens the work page when leaving a team for the personal center', async () => {
    localStorage.setItem('a3s.org.center', 'org-1')
    const { openWork } = renderMenu()

    await screen.findByRole('menuitemradio', { name: /通友科技/ })
    fireEvent.click(screen.getByRole('tab', { name: '个人' }))
    fireEvent.click(screen.getByRole('menuitemradio', { name: '个人中心' }))

    expect(localStorage.getItem('a3s.org.center')).toBe('personal')
    expect(openWork).toHaveBeenCalledTimes(1)
  })

  it('stays on the current page when the personal center is chosen again', async () => {
    localStorage.setItem('a3s.org.center', 'personal')
    const { openWork } = renderMenu()

    fireEvent.click(await screen.findByRole('menuitemradio', { name: '个人中心' }))

    expect(openWork).not.toHaveBeenCalled()
  })

  it('shows an empty team list and closes on Escape', async () => {
    const onOpenChange = vi.fn()
    renderMenu({ loadOrgs: vi.fn(async () => []), onOpenChange })

    fireEvent.click(await screen.findByRole('tab', { name: '团队' }))
    expect(await screen.findByText('还没有团队')).toBeInTheDocument()
    fireEvent.keyDown(document, { key: 'Escape' })
    expect(onOpenChange).toHaveBeenCalledWith(false)
  })

  it('shows a load error without pretending the account has no teams', async () => {
    renderMenu({ loadOrgs: vi.fn(async () => Promise.reject(new Error('offline'))) })
    fireEvent.click(await screen.findByRole('tab', { name: '团队' }))
    expect(await screen.findByText('无法加载团队')).toBeInTheDocument()
  })

  it('retries a failed team list in place', async () => {
    const loadOrgs = vi.fn<() => Promise<MemberOrg[]>>()
    loadOrgs.mockRejectedValueOnce(new Error('offline')).mockResolvedValue(orgs)
    renderMenu({ loadOrgs })
    fireEvent.click(await screen.findByRole('tab', { name: '团队' }))
    fireEvent.click(await screen.findByRole('button', { name: '重试' }))
    expect(await screen.findByRole('menuitemradio', { name: /通友科技/ })).toBeInTheDocument()
  })

  it('asks to sign in again when the member session is denied instead of saying the teams failed', async () => {
    const { logoutAccount, onSignedOut } = renderMenu({
      loadOrgs: vi.fn(async () => Promise.reject(new MemberOrgError('invalid session', 401)))
    })
    fireEvent.click(await screen.findByRole('tab', { name: '团队' }))
    expect(await screen.findByText('登录已失效，请重新登录')).toBeInTheDocument()
    expect(screen.queryByText('无法加载团队')).not.toBeInTheDocument()
    expect(screen.getByRole('button', { name: '创建团队' })).toBeDisabled()
    fireEvent.click(screen.getByRole('button', { name: '重新登录' }))
    await waitFor(() => expect(onSignedOut).toHaveBeenCalledTimes(1))
    expect(logoutAccount).toHaveBeenCalledTimes(1)
  })

  it('edits the nickname from the account header and shows it right away', async () => {
    const { updateAccount, onOpenChange } = renderMenu({
      loadAccount: async () => ({ displayName: 'RoyLin', phone: '13800138000' })
    })
    await screen.findByText('RoyLin')
    fireEvent.click(screen.getByRole('button', { name: '编辑资料' }))
    expect(onOpenChange).toHaveBeenCalledWith(false)
    const field = await screen.findByLabelText('昵称')
    expect(field).toHaveValue('RoyLin')

    fireEvent.change(field, { target: { value: '  ' } })
    fireEvent.submit(field.closest('form')!)
    expect(await screen.findByRole('alert')).toHaveTextContent('请填写昵称')
    expect(updateAccount).not.toHaveBeenCalled()

    fireEvent.change(field, { target: { value: '林知夏' } })
    fireEvent.click(screen.getByRole('button', { name: '保存' }))
    await waitFor(() => expect(updateAccount).toHaveBeenCalledWith({ displayName: '林知夏', avatar: '' }))
    expect(await screen.findByText('林知夏')).toBeInTheDocument()
    expect(screen.getByText('手机号 13800138000')).toBeInTheDocument()
  })

  it('creates a team from the footer after a name is entered', async () => {
    localStorage.setItem('a3s.org.center', 'org-1')
    const createOrg = vi.fn(async () => ({ id: 'org-9', name: '新团队', logo: '', creator: true }))
    const onOpenChange = vi.fn()
    renderMenu({ createOrg, onOpenChange })

    fireEvent.click(await screen.findByRole('button', { name: '创建团队' }))
    expect(onOpenChange).toHaveBeenCalledWith(false)
    const field = await screen.findByLabelText('名称')
    expect(field).toHaveAttribute('placeholder', '例如：某某科技有限公司')
    fireEvent.submit(field.closest('form')!)
    expect(await screen.findByRole('alert')).toHaveTextContent('请填写名称')

    fireEvent.change(field, { target: { value: '新团队' } })
    expect(document.querySelector('[data-ui="account.team-logo"]')).toHaveAttribute('src', textLogo('新团队'))
    fireEvent.submit(field.closest('form')!)
    await waitFor(() => expect(createOrg).toHaveBeenCalledWith('新团队', textLogo('新团队')))
    await waitFor(() => expect(localStorage.getItem('a3s.org.center')).toBe('org-9'))
  })

  it('creates a team from the name currently shown when the field did not emit a change', async () => {
    localStorage.setItem('a3s.org.center', 'org-1')
    const createOrg = vi.fn(async () => ({ id: 'org-shown', name: '字段所示', logo: '', creator: true }))
    renderMenu({ createOrg, onOpenChange: vi.fn() })

    fireEvent.click(await screen.findByRole('button', { name: '创建团队' }))
    const field = await screen.findByLabelText('名称')
    const setter = Object.getOwnPropertyDescriptor(HTMLInputElement.prototype, 'value')?.set
    setter?.call(field, '字段所示')
    fireEvent.submit(field.closest('form')!)
    await waitFor(() => expect(createOrg).toHaveBeenCalledWith('字段所示', textLogo('字段所示')))
  })

  it('joins a team from the personal tab after an invite code or link is entered', async () => {
    localStorage.setItem('a3s.org.center', 'org-1')
    const joinOrg = vi.fn(async () => ({ id: 'org-9', name: '新团队', logo: '' }))
    const onOpenChange = vi.fn()
    renderMenu({ joinOrg, onOpenChange })

    expect(await screen.findByRole('button', { name: '创建团队' })).toBeInTheDocument()
    fireEvent.click(screen.getByRole('tab', { name: '个人' }))
    expect(screen.queryByRole('button', { name: '创建团队' })).not.toBeInTheDocument()
    fireEvent.click(screen.getByRole('button', { name: '加入团队' }))
    expect(onOpenChange).toHaveBeenCalledWith(false)
    const field = await screen.findByLabelText('邀请码')
    expect(field).toHaveAttribute('placeholder', '粘贴邀请码或邀请链接')
    expect(field.closest('[data-testid="dialog-content"]')).toHaveClass('!animate-none', '!opacity-100')

    fireEvent.submit(field.closest('form')!)
    expect(await screen.findByRole('alert')).toHaveTextContent('请填写邀请码')

    fireEvent.change(field, { target: { value: 'not a code' } })
    fireEvent.submit(field.closest('form')!)
    expect(await screen.findByRole('alert')).toHaveTextContent('请输入有效的邀请码或邀请链接')
    expect(joinOrg).not.toHaveBeenCalled()

    fireEvent.change(field, { target: { value: 'https://example.test/join/abc123' } })
    fireEvent.submit(field.closest('form')!)
    await waitFor(() => expect(joinOrg).toHaveBeenCalledWith('abc123'))
    await waitFor(() => expect(localStorage.getItem('a3s.org.center')).toBe('org-9'))
  })

  it('searches for a team and sends a join request without switching the center', async () => {
    localStorage.setItem('a3s.org.center', 'personal')
    const found: DiscoverableOrg[] = [
      { id: 'org-9', name: '通友科技', logo: '', memberCount: 12, joined: false, requested: false },
      { id: 'org-1', name: '通友实验室', logo: '', memberCount: 3, joined: true, requested: false }
    ]
    const searchOrgs = vi.fn(async () => found)
    const requestJoin = vi.fn(async () => undefined)
    renderMenu({ searchOrgs, requestJoin })

    fireEvent.click(await screen.findByRole('button', { name: '加入团队' }))
    fireEvent.click(await screen.findByRole('tab', { name: '搜索团队' }))
    expect(screen.getByText('输入名称查找可加入的团队')).toBeInTheDocument()
    const field = screen.getByLabelText('团队名称')
    expect(field).toHaveAttribute('placeholder', '输入团队名称')

    fireEvent.change(field, { target: { value: '通友' } })
    const apply = await screen.findByRole('button', { name: '申请加入 通友科技' })
    expect(apply).toHaveClass('h-6')
    expect(searchOrgs).toHaveBeenCalledWith('通友')
    expect(screen.getByText('12 人')).toBeInTheDocument()
    expect(screen.getByText('已加入')).toBeInTheDocument()
    expect(screen.queryByRole('button', { name: '申请加入 通友实验室' })).not.toBeInTheDocument()

    fireEvent.click(apply)
    await waitFor(() => expect(requestJoin).toHaveBeenCalledWith('org-9'))
    const status = await screen.findByRole('status')
    expect(status).toHaveTextContent('申请已提交')
    expect(status).toHaveTextContent('等待管理员通过后即可加入')
    expect(screen.getByText('已申请')).toBeInTheDocument()
    expect(localStorage.getItem('a3s.org.center')).toBe('personal')
    expect(screen.queryByRole('button', { name: '申请加入 通友科技' })).not.toBeInTheDocument()
  })

  it('does not offer another request while one is still pending', async () => {
    localStorage.setItem('a3s.org.center', 'personal')
    const searchOrgs = vi.fn(async () => [
      { id: 'org-9', name: '测试企业', logo: '', memberCount: 1, joined: false, requested: true }
    ])
    const requestJoin = vi.fn(async () => undefined)
    renderMenu({ searchOrgs, requestJoin })

    fireEvent.click(await screen.findByRole('button', { name: '加入团队' }))
    fireEvent.click(await screen.findByRole('tab', { name: '搜索团队' }))
    fireEvent.change(screen.getByLabelText('团队名称'), { target: { value: '测试' } })

    expect(await screen.findByText('已申请')).toBeInTheDocument()
    expect(screen.queryByRole('button', { name: '申请加入 测试企业' })).not.toBeInTheDocument()
    expect(requestJoin).not.toHaveBeenCalled()
  })

  it('turns a duplicate request into the already-requested state', async () => {
    localStorage.setItem('a3s.org.center', 'personal')
    const searchOrgs = vi.fn(async () => [
      { id: 'org-9', name: '测试企业', logo: '', memberCount: 1, joined: false, requested: false }
    ])
    const requestJoin = vi.fn(async () => {
      throw new Error('join request already pending')
    })
    renderMenu({ searchOrgs, requestJoin })

    fireEvent.click(await screen.findByRole('button', { name: '加入团队' }))
    fireEvent.click(await screen.findByRole('tab', { name: '搜索团队' }))
    fireEvent.change(screen.getByLabelText('团队名称'), { target: { value: '测试' } })
    const apply = await screen.findByRole('button', { name: '申请加入 测试企业' })
    fireEvent.click(apply)
    fireEvent.click(apply)

    const status = await screen.findByRole('status')
    expect(status).toHaveTextContent('无法再次申请')
    expect(status).toHaveTextContent('管理员拒绝后才能再次申请')
    expect(screen.getByText('已申请')).toBeInTheDocument()
    expect(screen.queryByRole('button', { name: '申请加入 测试企业' })).not.toBeInTheDocument()
    expect(requestJoin).toHaveBeenCalledTimes(1)
  })

  it('shows the account above the switcher and signs out from the bottom', async () => {
    localStorage.setItem('a3s.org.center', 'org-1')
    const logoutAccount = vi.fn(async () => undefined)
    const onSignedOut = vi.fn()
    const { onOpenChange } = renderMenu({
      avatar: 'file:///tmp/avatar.png',
      fallbackName: '本地名',
      loadAccount: async () => ({ displayName: 'RoyLin', phone: '13800138000' }),
      logoutAccount,
      onSignedOut
    })

    const portrait = await screen.findByRole('img', { name: 'RoyLin' })
    expect(portrait).toHaveAttribute('src', 'file:///tmp/avatar.png')
    expect(portrait.className).toContain('rounded-[8px]')
    expect(screen.getByText('RoyLin')).toBeInTheDocument()
    expect(screen.getByText('手机号 13800138000')).toBeInTheDocument()
    const logout = screen.getByRole('button', { name: '退出登录' })
    const create = screen.getByRole('button', { name: '创建团队' })
    expect(create.compareDocumentPosition(logout) & Node.DOCUMENT_POSITION_FOLLOWING).toBeTruthy()

    fireEvent.click(logout)
    await waitFor(() => expect(logoutAccount).toHaveBeenCalledTimes(1))
    expect(onOpenChange).toHaveBeenCalledWith(false)
    expect(onSignedOut).toHaveBeenCalledTimes(1)
  })
})
