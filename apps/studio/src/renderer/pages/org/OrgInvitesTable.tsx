import { PopConfirm, rowMotionClassName, rowMotionStyle, SearchInput, SearchSelect } from '@a3s-lab/ui'
import { RefreshCw, Undo2 } from 'lucide-react'
import { type ReactNode, useEffect, useRef, useState } from 'react'
import { useTranslation } from 'react-i18next'

import { CollectionPagination } from '@renderer/components/collection/CollectionPagination'
import { beginOptimistic, useOptimisticRows } from '@renderer/components/collection/optimisticRows'
import { useCollectionPage } from '@renderer/components/collection/useCollectionPage'
import { orgAllowsInvite, type MemberOrg } from '@renderer/data/memberOrgs'
import { toast } from '@renderer/services/toast'
import { cn } from '@renderer/utils/style'

import { adminErrorKey, revokeMemberInvite, type MemberInvite } from './orgAdminApi'
import { orgSearchClassName } from './OrgCatalog'
import { ghostIconButtonClassName, rowActionClassName } from './orgUi'

export type InviteFilter = 'pending' | 'accepted' | 'revoked' | 'all'
const INVITE_FILTERS: InviteFilter[] = ['pending', 'accepted', 'revoked', 'all']
const DASH = '—'
const inviteId = (row: MemberInvite) => row.id

/** 手机号只露前三后四。 */
export function maskPhone(phone: string): string {
  const digits = phone.replace(/^\+?86/, '')
  return digits.length === 11 ? `${digits.slice(0, 3)}****${digits.slice(7)}` : phone
}

/** 成员页「邀请」分段：手机号邀请的状态筛选、搜索和撤销。 */
export function OrgInvitesTable({
  orgId,
  org,
  invites,
  segments,
  onRefresh
}: {
  orgId: string
  org: MemberOrg
  invites: MemberInvite[]
  segments: ReactNode
  onRefresh: () => Promise<void>
}) {
  const { t } = useTranslation()
  const [filter, setFilter] = useState<InviteFilter>('pending')
  const [query, setQuery] = useState('')
  const [pendingId, setPendingId] = useState<string | null>(null)
  const [refreshing, setRefreshing] = useState(false)
  const mounted = useRef(true)
  useEffect(() => {
    mounted.current = true
    return () => {
      mounted.current = false
    }
  }, [])

  const collection = `org/${orgId}/invites`
  const optimistic = useOptimisticRows(collection, invites, inviteId)
  const needle = query.replace(/\s+/g, '')
  const visible = optimistic.rows.filter(
    (row) => (filter === 'all' || row.status === filter) && (!needle || row.phone.includes(needle))
  )
  const pagination = useCollectionPage(visible, `invites:${orgId}`, 20, JSON.stringify([filter, needle]))
  const canRevoke = orgAllowsInvite(org)

  async function refreshAll() {
    if (refreshing) return
    setRefreshing(true)
    try {
      await onRefresh()
    } finally {
      if (mounted.current) setRefreshing(false)
    }
  }

  async function revoke(invite: MemberInvite) {
    if (!canRevoke || pendingId !== null || invite.status !== 'pending') return
    setPendingId(invite.id)
    const handle = beginOptimistic<MemberInvite>(collection, {
      patch: { [invite.id]: (row) => ({ ...row, status: 'revoked' }) }
    })
    try {
      await revokeMemberInvite(invite)
      handle.confirm()
      await onRefresh()
      if (mounted.current) toast.success(t('org.invites.revoked'))
    } catch (err) {
      handle.rollback()
      const key = adminErrorKey(err)
      if (key === 'org.invites.changed') {
        await onRefresh().catch(() => undefined)
        if (mounted.current) toast.error(t('org.invites.changed'))
      } else if (mounted.current) toast.error(t(key))
    } finally {
      handle.release()
      if (mounted.current) setPendingId(null)
    }
  }

  const pendingTotal = invites.filter((row) => row.status === 'pending').length

  return (
    <div data-ui="org.invites" className="flex min-h-0 min-w-0 flex-1 flex-col">
      <div className="flex shrink-0 flex-wrap items-center gap-2">
        <SearchInput
          value={query}
          onChange={(event) => setQuery(event.target.value)}
          onClear={() => setQuery('')}
          clearLabel={t('common.clear')}
          aria-label={t('org.invites.search')}
          placeholder={t('org.invites.search')}
          containerClassName={orgSearchClassName}
        />
        {segments}
        <SearchSelect
          size="sm"
          searchable={false}
          allowClear={false}
          aria-label={t('org.invites.filter.label')}
          value={filter}
          options={INVITE_FILTERS.map((value) => ({ value, label: t(`org.invites.filter.${value}`) }))}
          onChange={(next) =>
            setFilter((INVITE_FILTERS as string[]).includes(next) ? (next as InviteFilter) : 'pending')
          }
          className="w-[112px]"
        />
        <button
          type="button"
          aria-label={t('common.refresh')}
          title={t('common.refresh')}
          aria-busy={refreshing || undefined}
          disabled={refreshing || pendingId !== null}
          onClick={() => void refreshAll()}
          className={`${ghostIconButtonClassName} ml-auto`}>
          <RefreshCw
            className={cn('size-4 text-current', refreshing && 'animate-spin')}
            style={refreshing ? { animationDuration: '0.7s' } : undefined}
            strokeWidth={1.7}
            aria-hidden
          />
        </button>
      </div>
      <div className="mt-3 min-h-0 flex-1 overflow-auto pb-6">
        {visible.length === 0 ? (
          <p className="m-0 py-16 text-center text-[13px] text-[#8a8a8a]">
            {invites.length === 0
              ? t('org.invites.empty')
              : filter === 'pending' && pendingTotal === 0 && !needle
                ? t('org.invites.noPending')
                : t('org.members.noMatch')}
          </p>
        ) : (
          <table
            data-ui="org.invites-table"
            aria-label={t('org.invites.title')}
            className="w-full min-w-[640px] table-fixed border-collapse text-left">
            <colgroup>
              <col style={{ width: '30%' }} />
              <col style={{ width: '18%' }} />
              <col style={{ width: '16%' }} />
              <col style={{ width: '22%' }} />
              <col style={{ width: '14%' }} />
            </colgroup>
            <thead className="sticky top-0 z-[1] bg-white">
              <tr className="border-b border-[#efefef] text-[12px] text-[#8a8a8a]">
                {['phone', 'role', 'status', 'created', 'actions'].map((column, index, columns) => (
                  <th
                    key={column}
                    scope="col"
                    className={cn(
                      'h-9 font-normal',
                      index === 0 && 'pl-2.5',
                      index === columns.length - 1 && 'pr-2.5 text-right'
                    )}>
                    {t(`org.invites.col.${column}`)}
                  </th>
                ))}
              </tr>
            </thead>
            <tbody>
              {pagination.visible.map((row, index) => {
                const motion = optimistic.motion(row.id)
                return (
                  <tr
                    key={row.id}
                    data-motion={motion}
                    aria-busy={motion === 'pending' || undefined}
                    style={rowMotionStyle(index, motion)}
                    className={cn('group hover:bg-[#f7f7f7]', rowMotionClassName(motion))}>
                    <td className="h-12 pl-2.5 text-[13px] font-medium whitespace-nowrap text-[#1a1a1a]">
                      {maskPhone(row.phone)}
                    </td>
                    <td className="h-12 overflow-hidden text-[13px] whitespace-nowrap text-[#3f3f3f]">
                      {row.role_code?.trim() || t('org.members.role.member')}
                    </td>
                    <td className="h-12 text-[13px] whitespace-nowrap text-[#8a8a8a]">
                      {t(`org.invites.status.${row.status}`)}
                    </td>
                    <td className="h-12 text-[13px] whitespace-nowrap text-[#8a8a8a]">
                      {row.created_at.slice(0, 10) || DASH}
                    </td>
                    <td className="h-12 pr-2.5 text-right">
                      {canRevoke && row.status === 'pending' ? (
                        <PopConfirm
                          tone="danger"
                          title={t('org.invites.revokeTitle', { phone: maskPhone(row.phone) })}
                          description={t('org.invites.revokeDesc')}
                          confirmLabel={t('org.invites.revoke')}
                          cancelLabel={t('common.cancel')}
                          disabled={pendingId !== null}
                          onConfirm={() => void revoke(row)}>
                          <button
                            type="button"
                            aria-label={`${t('org.invites.revoke')} ${maskPhone(row.phone)}`}
                            title={t('org.invites.revoke')}
                            disabled={pendingId !== null}
                            className={rowActionClassName}>
                            <Undo2 className="size-3.5 text-current" strokeWidth={1.7} />
                          </button>
                        </PopConfirm>
                      ) : null}
                    </td>
                  </tr>
                )
              })}
            </tbody>
          </table>
        )}
      </div>
      {visible.length > 0 ? (
        <div className="shrink-0 pb-4">
          <CollectionPagination {...pagination} busy={refreshing || pendingId !== null} />
        </div>
      ) : null}
    </div>
  )
}
