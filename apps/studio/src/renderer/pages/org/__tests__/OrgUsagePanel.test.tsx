import { fireEvent, render, screen, within } from '@testing-library/react'
import { describe, expect, it, vi } from 'vitest'

import type { OrgDepartment, OrgMemberRow } from '../orgApi'
import { OrgUsagePanel } from '../OrgUsagePanel'

vi.mock('react-i18next', () => ({
  useTranslation: () => ({
    t: (key: string) => key,
    i18n: { language: 'zh-CN' }
  })
}))

vi.mock('@renderer/pages/settings/UsageSettings', () => ({
  UsageSettings: ({ scope }: { scope?: { departmentId?: string; userId?: string } }) => (
    <div data-ui="org.usage.charts">{scope?.departmentId ?? scope?.userId ?? 'team'}</div>
  )
}))

function department(id: string, parent_id: string | null, name: string): OrgDepartment {
  return { id, parent_id, name, sort_order: 0 }
}

function member(id: string, patch: Partial<OrgMemberRow> = {}): OrgMemberRow {
  return {
    id,
    name: id,
    email: `${id}@example.com`,
    phone: null,
    status: 'active',
    admin: false,
    creator: false,
    department_id: 'dept-root',
    joined_at: '2026-01-02T00:00:00Z',
    position_id: null,
    position_name: null,
    role_code: 'member',
    role_name: '成员',
    role_built_in: true,
    ...patch
  }
}

const departments = [
  department('dept-root', null, '总部'),
  department('dept-rd', 'dept-root', '研发'),
  department('dept-mkt', 'dept-root', '市场')
]

const members = [
  member('user-lin', { name: '林书豪', avatar: 'data:image/webp;base64,AA', phone: '19165061825' }),
  member('user-leon', { name: 'Leon', phone: '17370859985' })
]

describe('OrgUsagePanel filters', () => {
  it('searches a department tree and keeps the native select off the page', () => {
    render(<OrgUsagePanel departments={departments} members={members} />)
    expect(document.querySelector('select')).toBeNull()

    fireEvent.click(screen.getByRole('button', { name: 'org.usage.department' }))
    const trigger = screen.getByRole('combobox', { name: 'org.usage.pickDepartment' })
    expect(trigger).toHaveClass('h-9', 'rounded-[8px]', 'bg-[#f5f5f5]')
    expect(trigger).toHaveTextContent('总部')
    expect(screen.queryByRole('treeitem', { name: '研发' })).not.toBeInTheDocument()

    fireEvent.click(trigger)
    const tree = screen.getByRole('tree', { name: 'org.usage.pickDepartment' })
    expect(tree).toHaveClass('gap-0.5')
    expect(within(tree).getByRole('treeitem', { name: '总部' })).toHaveAttribute('aria-selected', 'true')
    expect(within(tree).getByRole('treeitem', { name: '研发' })).toHaveClass('h-9', 'rounded-[8px]')
    expect(within(tree).getByRole('treeitem', { name: '市场' })).toBeInTheDocument()

    fireEvent.change(screen.getByRole('textbox', { name: 'org.usage.searchDepartment' }), { target: { value: '研发' } })
    expect(screen.getByRole('treeitem', { name: '研发' })).toBeInTheDocument()
    expect(screen.getByRole('treeitem', { name: '总部' })).toBeInTheDocument()
    expect(screen.queryByRole('treeitem', { name: '市场' })).not.toBeInTheDocument()

    fireEvent.click(screen.getByRole('treeitem', { name: '研发' }))
    expect(screen.queryByRole('tree')).not.toBeInTheDocument()
    expect(screen.getByRole('combobox', { name: 'org.usage.pickDepartment' })).toHaveTextContent('研发')
    expect(document.querySelector('[data-ui="org.usage.charts"]')).toHaveTextContent('dept-rd')
    expect(document.querySelector('select')).toBeNull()
  })

  it('searches members and shows each avatar in the list', () => {
    render(<OrgUsagePanel departments={departments} members={members} />)
    fireEvent.click(screen.getByRole('button', { name: 'org.usage.member' }))
    const trigger = screen.getByRole('combobox', { name: 'org.usage.pickMember' })
    expect(trigger).toHaveClass('h-9', 'rounded-[8px]', 'bg-[#f5f5f5]')

    fireEvent.click(trigger)
    const list = screen.getByRole('listbox', { name: 'org.usage.pickMember' })
    const lin = within(list).getByRole('option', { name: /林书豪/ })
    expect(lin).toHaveClass('h-9', 'rounded-[8px]')
    expect(within(lin).getByRole('img', { name: '林书豪' }).querySelector('img')).toHaveAttribute(
      'src',
      'data:image/webp;base64,AA'
    )
    const leon = within(list).getByRole('option', { name: /Leon/ })
    expect(within(leon).getByRole('img', { name: 'Leon' }).querySelector('img')).toBeNull()
    expect(within(leon).getByRole('img', { name: 'Leon' })).toHaveTextContent('L')
    expect(screen.getByPlaceholderText('org.usage.searchMember').closest('label')).toHaveClass(
      'h-[30px]',
      'rounded-[8px]',
      'bg-[#f5f5f5]'
    )

    fireEvent.change(screen.getByRole('textbox', { name: 'org.usage.searchMember' }), {
      target: { value: '17370859985' }
    })
    expect(screen.getByRole('option', { name: /Leon/ })).toBeInTheDocument()
    expect(screen.queryByRole('option', { name: /林书豪/ })).not.toBeInTheDocument()

    fireEvent.click(screen.getByRole('option', { name: /Leon/ }))
    expect(screen.queryByRole('listbox')).not.toBeInTheDocument()
    expect(screen.getByRole('combobox', { name: 'org.usage.pickMember' })).toHaveTextContent('Leon')
    expect(document.querySelector('[data-ui="org.usage.charts"]')).toHaveTextContent('user-leon')
    expect(document.querySelector('select')).toBeNull()
  })
})
