import { ConfirmDialog, Dialog, DialogContent, DialogTitle, Input, SearchSelect } from '@a3s-lab/ui'
import { Check, Plus, X } from 'lucide-react'
import { QRCodeSVG } from 'qrcode.react'
import { useEffect, useRef, useState } from 'react'
import { useTranslation } from 'react-i18next'

import {
  orgAllows,
  orgAllowsConfigureInvite,
  orgAllowsInvite,
  orgAllowsJoin,
  type MemberOrg
} from '@renderer/data/memberOrgs'
import { toast } from '@renderer/services/toast'
import { cn } from '@renderer/utils/style'

import { adminErrorKey, createMemberInvite, isInvitePhone, type SeatSummary } from './orgAdminApi'
import {
  approveJoinRequest,
  configureJoinInvite,
  fetchJoinInvite,
  isInvitationRevision,
  listJoinRequests,
  listOrgRoles,
  readOrgOrigin,
  rejectJoinRequest,
  type OrgJoinInvite,
  type OrgRole,
  type PendingApplicant
} from './orgApi'
import { filledButtonClassName, orgFieldClassName, quietButtonClassName, rowActionClassName } from './orgUi'

/** 组织架构页页头的邀请入口：加入链接二维码、启用 / 轮换，以及待审核队列。 */
export function OrgInviteTools({
  orgId,
  org,
  seats,
  onOpenPlan,
  onMembershipChanged,
  onInvited
}: {
  orgId: string
  org: MemberOrg
  seats?: SeatSummary | null
  onOpenPlan?: () => void
  onMembershipChanged: () => Promise<void>
  onInvited?: () => Promise<void>
}) {
  const { t } = useTranslation()
  const [open, setOpen] = useState(false)
  if (!orgAllowsInvite(org)) return null
  return (
    <>
      <button
        type="button"
        data-ui="org.invite-trigger"
        onClick={() => setOpen(true)}
        className={filledButtonClassName(30)}>
        <Plus size={16} className="shrink-0 text-current" strokeWidth={1.7} aria-hidden />
        {t('org.invite.open')}
      </button>
      <OrgInviteDialog
        open={open && orgAllowsInvite(org)}
        orgId={orgId}
        org={org}
        seats={seats ?? null}
        onOpenPlan={
          onOpenPlan
            ? () => {
                setOpen(false)
                onOpenPlan()
              }
            : undefined
        }
        onOpenChange={setOpen}
        onMembershipChanged={onMembershipChanged}
        onInvited={onInvited}
      />
    </>
  )
}

function OrgInviteDialog({
  open,
  orgId,
  org,
  seats,
  onOpenPlan,
  onOpenChange,
  onMembershipChanged,
  onInvited
}: {
  open: boolean
  orgId: string
  org: MemberOrg
  seats: SeatSummary | null
  onOpenPlan?: () => void
  onOpenChange: (open: boolean) => void
  onMembershipChanged: () => Promise<void>
  onInvited?: () => Promise<void>
}) {
  const { t } = useTranslation()
  const canInvite = orgAllowsInvite(org)
  const canConfigure = orgAllowsConfigureInvite(org)
  const canApprove = orgAllowsJoin(org, 'approve')
  const canReject = orgAllowsJoin(org, 'reject')
  const [invite, setInvite] = useState<OrgJoinInvite | null>(null)
  const [pending, setPending] = useState<PendingApplicant[] | null>(null)
  const [orgOrigin, setOrgOrigin] = useState('')
  const [failed, setFailed] = useState(false)
  const [busy, setBusy] = useState(false)
  const [pendingId, setPendingId] = useState<string | null>(null)
  const [rotateRevision, setRotateRevision] = useState<string | null>(null)
  const [attempt, setAttempt] = useState(0)
  const invitationRevision = useRef(invite?.revision)
  invitationRevision.current = invite?.revision

  const authority = JSON.stringify([
    orgId,
    open,
    org.canInviteMembers,
    org.canReviewJoin,
    org.creator,
    org.rules,
    org.dataScope
  ])
  const policy = useRef(authority)
  const epoch = useRef(0)
  if (policy.current !== authority) {
    policy.current = authority
    epoch.current += 1
  }
  const mounted = useRef(true)
  useEffect(() => {
    mounted.current = true
    return () => {
      mounted.current = false
    }
  }, [])
  const generation = epoch.current
  const current = (request: number) => mounted.current && epoch.current === request && policy.current === authority

  useEffect(() => {
    setBusy(false)
    setPendingId(null)
    setRotateRevision(null)
    setInvite(null)
    setPending(null)
    if (!open || !canInvite) return
    let cancelled = false
    setFailed(false)
    Promise.all([fetchJoinInvite(orgId), readOrgOrigin().catch(() => '')])
      .then(([nextInvite, origin]) => {
        if (cancelled) return
        setInvite(nextInvite)
        setOrgOrigin(origin)
      })
      .catch(() => {
        if (!cancelled) setFailed(true)
      })
    void (canApprove || canReject ? listJoinRequests(orgId) : Promise.resolve([] as PendingApplicant[]))
      .then((rows) => {
        if (!cancelled) setPending(rows)
      })
      .catch(() => {
        if (!cancelled) setPending([])
      })
    return () => {
      cancelled = true
    }
  }, [open, orgId, attempt, canApprove, canReject, authority, canInvite])

  function withdraw() {
    epoch.current += 1
    setInvite(null)
    setPending(null)
    setRotateRevision(null)
    setBusy(false)
    setPendingId(null)
    setFailed(true)
  }

  async function retry() {
    if (!current(generation) || !open || !canInvite || busy) return
    const request = epoch.current
    setBusy(true)
    try {
      await onMembershipChanged()
      if (current(request)) setAttempt((n) => n + 1)
    } catch {
      if (current(request)) toast.error(t('org.error.generic'))
    } finally {
      if (current(request)) setBusy(false)
    }
  }

  async function configure(input: { enabled?: boolean; rotate?: boolean }, reviewedRevision = invite?.revision) {
    if (
      !current(generation) ||
      !open ||
      !canConfigure ||
      !isInvitationRevision(reviewedRevision) ||
      reviewedRevision !== invitationRevision.current
    )
      return false
    if (busy) return false
    const request = epoch.current
    setBusy(true)
    try {
      const next = await configureJoinInvite(orgId, { ...input, expected_revision: reviewedRevision })
      if (!current(request)) return false
      setInvite(next)
      setRotateRevision(null)
      return true
    } catch {
      if (current(request)) {
        withdraw()
        toast.error(t('org.error.generic'))
      }
      return false
    } finally {
      if (current(request)) setBusy(false)
    }
  }

  async function decide(memberId: string, approve: boolean) {
    if (!current(generation) || !open || !canInvite || pendingId !== null || (approve ? !canApprove : !canReject))
      return false
    const request = epoch.current
    setPendingId(memberId)
    try {
      if (approve) await approveJoinRequest(orgId, memberId)
      else await rejectJoinRequest(orgId, memberId)
      if (!current(request)) return false
      setPending((rows) => (rows ?? []).filter((row) => row.member_id !== memberId))
      await onMembershipChanged()
      if (!current(request)) return false
      toast.success(t(approve ? 'org.invite.approved' : 'org.invite.rejected'))
      return true
    } catch (err) {
      if (current(request)) {
        withdraw()
        toast.error(t(adminErrorKey(err)))
      }
      return false
    } finally {
      if (current(request)) setPendingId(null)
    }
  }

  async function copy() {
    if (!current(generation) || !open || !canInvite || !invite || !invite.enabled) return
    const request = epoch.current
    try {
      await navigator.clipboard.writeText(`${orgOrigin}/join/${invite.code}`)
      if (current(request)) toast.success(t('org.invite.copied'))
    } catch {
      if (current(request)) toast.error(t('org.invite.copyFailed'))
    }
  }

  const link = invite ? `${orgOrigin}/join/${invite.code}` : ''

  return (
    <>
      <Dialog open={open} onOpenChange={onOpenChange}>
        <DialogContent aria-describedby={undefined} className="sm:max-w-[440px]">
          <DialogTitle className="text-[15px] leading-5 font-medium">{t('org.invite.open')}</DialogTitle>
          {failed ? (
            <div className="flex flex-col items-center gap-3 py-6">
              <p className="m-0 text-[13px] text-[#8a8a8a]">{t('org.error.generic')}</p>
              <button type="button" className={quietButtonClassName(30)} disabled={busy} onClick={() => void retry()}>
                {t('common.retry')}
              </button>
            </div>
          ) : !invite || pending === null ? (
            <p className="m-0 py-8 text-center text-[13px] text-[#8a8a8a]">{t('common.loading')}</p>
          ) : (
            <div data-ui="org.invite" className="flex flex-col gap-4">
              {seats ? <SeatLine seats={seats} org={org} onOpenPlan={onOpenPlan} /> : null}
              <PhoneInviteForm key={`${orgId}:${generation}`} orgId={orgId} org={org} onInvited={onInvited} />
              <div className="flex items-start gap-4">
                <div className="grid size-[112px] shrink-0 place-items-center rounded-[8px] bg-[#f5f5f5]">
                  {invite.enabled ? <QRCodeSVG value={link} size={96} level="M" title={t('org.invite.link')} /> : null}
                </div>
                <div className="flex min-w-0 flex-1 flex-col gap-2">
                  <span
                    className={cn(
                      'inline-flex h-5 w-fit items-center rounded-[4px] px-1.5 text-[11px]',
                      invite.enabled ? 'bg-[#f2f3f5] font-medium text-[#171717]' : 'bg-[#f2f3f5] text-[#8a8a8a]'
                    )}>
                    {invite.enabled ? t('org.invite.statusOn') : t('org.invite.statusOff')}
                  </span>
                  <Input readOnly value={link} aria-label={t('org.invite.link')} className={orgFieldClassName} />
                </div>
              </div>
              <div className="flex flex-wrap items-center gap-2">
                <button
                  type="button"
                  className={filledButtonClassName(30)}
                  disabled={busy || !invite.enabled}
                  onClick={() => void copy()}>
                  {t('org.invite.copy')}
                </button>
                {canConfigure && isInvitationRevision(invite.revision) ? (
                  <button
                    type="button"
                    className={quietButtonClassName(30)}
                    disabled={busy}
                    onClick={() => void configure({ enabled: !invite.enabled })}>
                    {invite.enabled ? t('org.invite.disable') : t('org.invite.enable')}
                  </button>
                ) : null}
                {canConfigure && isInvitationRevision(invite.revision) ? (
                  <button
                    type="button"
                    className={quietButtonClassName(30)}
                    disabled={busy}
                    onClick={() => {
                      if (isInvitationRevision(invite.revision)) setRotateRevision(invite.revision)
                    }}>
                    {t('org.join.rotateCta')}
                  </button>
                ) : null}
              </div>
              <p className="m-0 text-[12px] leading-5 text-[#8a8a8a]">{t('org.invite.authorityHint')}</p>
              <section className="flex flex-col gap-2">
                <h3 className="m-0 flex items-center gap-1.5 text-[13px] font-medium text-[#171717]">
                  {t('org.invite.pending')}
                  <span className="text-[12px] font-normal text-[#8a8a8a]">{pending.length}</span>
                </h3>
                {pending.length === 0 ? (
                  <p className="m-0 text-[13px] text-[#8a8a8a]">{t('org.invite.empty')}</p>
                ) : (
                  <ul className="m-0 flex list-none flex-col gap-0.5 p-0">
                    {pending.map((row) => (
                      <li
                        key={row.member_id}
                        className="flex h-9 items-center gap-2 rounded-[8px] px-2 hover:bg-[#f7f7f7]">
                        <span className="min-w-0 flex-1 truncate text-[13px] text-[#171717]">{row.name}</span>
                        <div className="flex items-center gap-1">
                          {canApprove ? (
                            <button
                              type="button"
                              aria-label={t('org.invite.approve')}
                              title={t('org.invite.approve')}
                              disabled={pendingId !== null}
                              onClick={() => void decide(row.member_id, true)}
                              className={rowActionClassName}>
                              <Check className="size-3.5" strokeWidth={1.7} />
                            </button>
                          ) : null}
                          {canReject ? (
                            <button
                              type="button"
                              aria-label={t('org.invite.reject')}
                              title={t('org.invite.reject')}
                              disabled={pendingId !== null}
                              onClick={() => void decide(row.member_id, false)}
                              className={rowActionClassName}>
                              <X className="size-3.5" strokeWidth={1.7} />
                            </button>
                          ) : null}
                        </div>
                      </li>
                    ))}
                  </ul>
                )}
              </section>
            </div>
          )}
        </DialogContent>
      </Dialog>

      <ConfirmDialog
        open={open && canConfigure && isInvitationRevision(rotateRevision) && rotateRevision === invite?.revision}
        operationKey={`${orgId}:${rotateRevision}`}
        failureText={t('common.save_failed')}
        onOpenChange={(next) => {
          if (!next) setRotateRevision(null)
        }}
        title={t('org.join.rotateCta')}
        description={t('org.invite.rotateConfirm')}
        confirmText={t('org.join.rotateCta')}
        cancelText={t('common.cancel')}
        onConfirm={() => configure({ rotate: true }, rotateRevision ?? '')}
      />
    </>
  )
}

/** 席位提示：已用 = 在职成员 + 待接受邀请。满员时只提示，不在客户端拦截。 */
function SeatLine({ seats, org, onOpenPlan }: { seats: SeatSummary; org: MemberOrg; onOpenPlan?: () => void }) {
  const { t } = useTranslation()
  const full = seats.used >= seats.total
  return (
    <div
      data-ui="org.invite.seats"
      className={cn(
        'flex items-center gap-2 rounded-[8px] px-3 py-2 text-[12px] leading-5',
        full ? 'bg-[#fff7e6] text-[#8a5a00]' : 'bg-[#f7f7f7] text-[#3f3f3f]'
      )}>
      <span role="status" className="min-w-0 flex-1">
        {t('org.invite.seats', { used: seats.used, total: seats.total, plan: t(`org.plan.name.${seats.planId}`) })}
        {full ? ` · ${t('org.invite.seatsFull')}` : ''}
      </span>
      {onOpenPlan && orgAllows(org, 'manage', 'Billing') ? (
        <button
          type="button"
          className="shrink-0 text-[12px] font-medium text-[#171717] underline-offset-2 hover:underline"
          onClick={onOpenPlan}>
          {t('org.invite.adjustSeats')}
        </button>
      ) : null}
    </div>
  )
}

/** 手机号邀请。只有创建人能指定角色；其他人邀请的成员先是普通成员。 */
function PhoneInviteForm({
  orgId,
  org,
  onInvited
}: {
  orgId: string
  org: MemberOrg
  onInvited?: () => Promise<void>
}) {
  const { t } = useTranslation()
  const [phone, setPhone] = useState('')
  const [roleCode, setRoleCode] = useState('')
  const [roles, setRoles] = useState<OrgRole[] | null>(null)
  const [error, setError] = useState<string | null>(null)
  const [busy, setBusy] = useState(false)
  const mounted = useRef(true)
  useEffect(() => {
    mounted.current = true
    return () => {
      mounted.current = false
    }
  }, [])
  useEffect(() => {
    if (!org.creator) return
    let cancelled = false
    listOrgRoles(orgId)
      .then((rows) => {
        if (!cancelled) setRoles(rows)
      })
      .catch(() => {
        if (!cancelled) setRoles(null)
      })
    return () => {
      cancelled = true
    }
  }, [orgId, org.creator])

  async function submit() {
    if (busy) return
    const value = phone.replace(/\s+/g, '')
    if (!isInvitePhone(value)) {
      setError(t('org.invites.phoneInvalid'))
      return
    }
    setBusy(true)
    setError(null)
    try {
      await createMemberInvite(orgId, value, org.creator && roleCode ? roleCode : undefined)
      if (!mounted.current) return
      setPhone('')
      setRoleCode('')
      toast.success(t('org.invites.sent'))
      await onInvited?.()
    } catch (err) {
      if (mounted.current) setError(t(adminErrorKey(err)))
    } finally {
      if (mounted.current) setBusy(false)
    }
  }

  return (
    <form
      data-ui="org.invite.phone"
      className="grid gap-2"
      onSubmit={(event) => {
        event.preventDefault()
        void submit()
      }}>
      <h3 className="m-0 text-[13px] font-medium text-[#171717]">{t('org.invites.phoneTitle')}</h3>
      <div className="flex items-center gap-2">
        <Input
          value={phone}
          inputMode="tel"
          maxLength={16}
          disabled={busy}
          aria-label={t('org.invites.phone')}
          placeholder={t('org.invites.phonePlaceholder')}
          aria-invalid={error ? true : undefined}
          className={cn(orgFieldClassName, 'min-w-0 flex-1')}
          onChange={(event) => {
            setPhone(event.target.value)
            setError(null)
          }}
        />
        {org.creator && roles && roles.length > 0 ? (
          <SearchSelect
            searchable={roles.length > 8}
            allowClear={false}
            aria-label={t('org.invites.role')}
            value={roleCode}
            options={[
              { value: '', label: t('org.members.role.member') },
              ...roles.filter((role) => role.code !== 'member').map((role) => ({ value: role.code, label: role.name }))
            ]}
            onChange={(next) => setRoleCode(next ?? '')}
            className="w-[120px]"
          />
        ) : null}
        <button type="submit" disabled={busy || !phone.trim()} className={filledButtonClassName(36)}>
          {t('org.invites.send')}
        </button>
      </div>
      {error ? (
        <p className="m-0 text-[12px] text-[#b42318]" role="alert">
          {error}
        </p>
      ) : (
        <p className="m-0 text-[12px] leading-5 text-[#8a8a8a]">{t('org.invites.phoneHint')}</p>
      )}
    </form>
  )
}
