import { SegmentedControl } from '@a3s-lab/ui'
import { useEffect, useMemo, useState } from 'react'
import { useTranslation } from 'react-i18next'

import { UsageSettings } from '@renderer/pages/settings/UsageSettings'
import type { OrgUsageScope } from '@renderer/pages/settings/UsageSettings/orgUsage'

import type { OrgDepartment, OrgMemberRow } from './orgApi'
import { DepartmentUsagePicker, MemberUsagePicker } from './OrgUsagePickers'

type UsageMode = 'team' | 'department' | 'member'

function rootDepartmentId(rows: readonly OrgDepartment[]): string | null {
  return rows.find((row) => row.parent_id === null)?.id ?? rows[0]?.id ?? null
}

function firstMemberId(rows: readonly OrgMemberRow[]): string | null {
  const sorted = [...rows].sort((left, right) => left.name.localeCompare(right.name, 'zh'))
  return sorted[0]?.id ?? null
}

/** 创建人查看团队 Token 用量，并按当前部门树或一名成员筛选。 */
export function OrgUsagePanel({
  departments,
  members
}: {
  departments: readonly OrgDepartment[]
  members: readonly OrgMemberRow[]
}) {
  const { t } = useTranslation()
  const [mode, setMode] = useState<UsageMode>('team')
  const [departmentId, setDepartmentId] = useState<string | null>(() => rootDepartmentId(departments))
  const [memberId, setMemberId] = useState<string | null>(() => firstMemberId(members))
  useEffect(() => {
    if (departmentId && departments.some((row) => row.id === departmentId)) return
    setDepartmentId(rootDepartmentId(departments))
  }, [departmentId, departments])
  useEffect(() => {
    if (memberId && members.some((row) => row.id === memberId)) return
    setMemberId(firstMemberId(members))
  }, [memberId, members])

  const scope = useMemo<OrgUsageScope | undefined>(() => {
    if (mode === 'department') return departmentId ? { departmentId } : undefined
    if (mode === 'member') return memberId ? { userId: memberId } : undefined
    return undefined
  }, [departmentId, memberId, mode])
  const waiting = (mode === 'department' && !departmentId) || (mode === 'member' && !memberId)

  return (
    <div data-ui="org.usage" className="flex min-h-0 min-w-0 flex-1 flex-col">
      <header className="flex shrink-0 flex-wrap items-start justify-between gap-4 px-6 pt-4 pb-4">
        <div className="min-w-0">
          <h2 className="m-0 truncate text-[24px] leading-tight font-bold tracking-[-0.01em] text-foreground">
            {t('org.nav.usage')}
          </h2>
          <p className="m-0 mt-1 max-w-2xl text-[15px] leading-6 text-[#8a8a8a]">{t('org.usage.description')}</p>
        </div>
        <div className="flex min-w-0 flex-wrap items-center gap-2">
          <SegmentedControl
            aria-label={t('org.usage.scope')}
            size="sm"
            value={mode}
            options={[
              { value: 'team', label: t('org.usage.team') },
              { value: 'department', label: t('org.usage.department') },
              { value: 'member', label: t('org.usage.member') }
            ]}
            onValueChange={(value) => {
              if (value === 'team' || value === 'department' || value === 'member') setMode(value)
            }}
          />
          {mode === 'department' && departmentId ? (
            <DepartmentUsagePicker departments={departments} value={departmentId} onChange={setDepartmentId} />
          ) : null}
          {mode === 'member' && memberId ? (
            <MemberUsagePicker members={members} value={memberId} onChange={setMemberId} />
          ) : null}
        </div>
      </header>
      <div className="min-h-0 flex-1 overflow-auto px-6 pb-6">{waiting ? null : <UsageSettings scope={scope} />}</div>
    </div>
  )
}
