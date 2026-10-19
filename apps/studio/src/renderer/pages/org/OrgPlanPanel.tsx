import { Dialog, DialogContent, DialogTitle, Input, SearchSelect } from '@a3s-lab/ui'
import { useEffect, useState } from 'react'
import { useTranslation } from 'react-i18next'
import useSWR from 'swr'

import { PageEntry } from '@renderer/components/layout/PageEntry'
import { orgAllows, type MemberOrg } from '@renderer/data/memberOrgs'
import { toast } from '@renderer/services/toast'
import { cn } from '@renderer/utils/style'

import {
  adminErrorKey,
  fetchOrgSubscription,
  isPlanId,
  listOrgEntitlements,
  PLAN_SEATS,
  SELF_SERVE_PLANS,
  updateOrgSubscription,
  usedSeats,
  type PlanId
} from './orgAdminApi'
import { OrgAdminFrame, OrgCard } from './OrgAdminFrame'
import type { OrgMemberRow } from './orgApi'
import { filledButtonClassName, orgFieldClassName, quietButtonClassName } from './orgUi'

const DASH = '—'

/** 套餐与服务：所有成员可看；有「管理计费」且部署开放自助变更时才能改。企业版找销售。 */
export function OrgPlanPanel({
  orgId,
  org,
  members
}: {
  orgId: string
  org: MemberOrg
  members: readonly Pick<OrgMemberRow, 'status'>[]
}) {
  const { t } = useTranslation()
  const {
    data: subscription,
    error,
    mutate
  } = useSWR(`studio/org-subscription:${orgId}`, () => fetchOrgSubscription(orgId))
  const { data: routes, error: routesError } = useSWR(`studio/org-entitlements:${orgId}`, () =>
    listOrgEntitlements(orgId)
  )
  const [changing, setChanging] = useState(false)
  const canManage = orgAllows(org, 'manage', 'Billing')
  const used = usedSeats(members)
  const planId = subscription?.plan_id ?? ''
  const enterprise = planId === 'enterprise'
  const planName = isPlanId(planId) ? t(`org.plan.name.${planId}`) : planId || DASH

  let changeHint: string | null = null
  if (subscription && canManage) {
    if (enterprise) changeHint = t('org.plan.enterpriseSales')
    else if (!subscription.self_serve) changeHint = t('org.plan.selfServeOff')
  }

  return (
    <OrgAdminFrame
      ui="org.plan"
      title={t('org.nav.plan')}
      description={t('org.plan.description')}
      actions={
        subscription && canManage && subscription.self_serve && !enterprise ? (
          <button
            type="button"
            data-ui="org.plan.change"
            className={filledButtonClassName(30)}
            onClick={() => setChanging(true)}>
            {t('org.plan.change')}
          </button>
        ) : undefined
      }>
      {error ? (
        <div className="flex flex-1 flex-col items-center justify-center gap-3">
          <p className="m-0 text-[13px] text-[#8a8a8a]">{t('org.error.generic')}</p>
          <button type="button" onClick={() => void mutate()} className={quietButtonClassName(30)}>
            {t('common.retry')}
          </button>
        </div>
      ) : !subscription ? (
        <div className="relative min-h-40 flex-1">
          <PageEntry id="org.plan" ready={false} />
        </div>
      ) : (
        <div className="grid max-w-4xl gap-4">
          <OrgCard ui="org.plan.subscription" title={t('org.plan.current')}>
            <dl className="m-0 grid grid-cols-[repeat(auto-fit,minmax(160px,1fr))] gap-4">
              <Fact label={t('org.plan.col.plan')} value={planName} />
              <Fact
                label={t('org.plan.col.status')}
                value={t(subscription.status === 'active' ? 'org.plan.status.active' : 'org.plan.status.cancelled')}
              />
              <Fact
                label={t('org.plan.col.seats')}
                value={t('org.plan.seatsUsed', { used, total: subscription.seats })}
              />
              <Fact label={t('org.plan.col.since')} value={subscription.period_start.slice(0, 10) || DASH} />
            </dl>
            <div
              className="mt-4 h-1.5 overflow-hidden rounded-full bg-[#f2f3f5]"
              role="meter"
              aria-label={t('org.plan.col.seats')}
              aria-valuemin={0}
              aria-valuemax={subscription.seats}
              aria-valuenow={Math.min(used, subscription.seats)}>
              <div
                className={cn('h-full rounded-full', used >= subscription.seats ? 'bg-[#d97706]' : 'bg-[#171717]')}
                style={{ width: `${Math.min(100, (used / Math.max(1, subscription.seats)) * 100)}%` }}
              />
            </div>
            <p className="m-0 mt-2 text-[12px] leading-5 text-[#8a8a8a]">{t('org.plan.seatsRule')}</p>
            {changeHint ? (
              <p role="note" className="m-0 mt-3 text-[13px] leading-5 text-[#3f3f3f]">
                {changeHint}
              </p>
            ) : null}
          </OrgCard>
          <OrgCard ui="org.plan.entitlements" title={t('org.plan.entitlements')}>
            {routesError ? (
              <p className="m-0 text-[13px] text-[#8a8a8a]">{t('org.error.generic')}</p>
            ) : !routes ? (
              <p className="m-0 text-[13px] text-[#8a8a8a]">{t('common.loading')}</p>
            ) : routes.length === 0 ? (
              <p className="m-0 text-[13px] text-[#8a8a8a]">{t('org.plan.noEntitlements')}</p>
            ) : (
              <table className="w-full table-fixed border-collapse text-left">
                <thead>
                  <tr className="border-b border-[#efefef] text-[12px] text-[#8a8a8a]">
                    {['route', 'project', 'workload', 'state'].map((column) => (
                      <th key={column} scope="col" className="h-9 font-normal">
                        {t(`org.plan.route.${column}`)}
                      </th>
                    ))}
                  </tr>
                </thead>
                <tbody>
                  {routes.map((route) => (
                    <tr key={`${route.hostname}${route.path_prefix}`} className="text-[13px] text-[#3f3f3f]">
                      <td className="h-10 truncate">{`${route.hostname}${route.path_prefix}`}</td>
                      <td className="h-10 truncate">{`${route.project_name} / ${route.environment_name}`}</td>
                      <td className="h-10 truncate">
                        {route.workload_name ?? DASH}
                        {route.workload_health ? ` · ${route.workload_health}` : ''}
                      </td>
                      <td className="h-10 truncate">{route.state}</td>
                    </tr>
                  ))}
                </tbody>
              </table>
            )}
          </OrgCard>
        </div>
      )}
      {subscription ? (
        <PlanChangeDialog
          open={changing && canManage && subscription.self_serve && !enterprise}
          orgId={orgId}
          currentPlan={isPlanId(planId) ? planId : 'free'}
          currentSeats={subscription.seats}
          used={used}
          onOpenChange={setChanging}
          onChanged={async () => {
            await mutate()
          }}
        />
      ) : null}
    </OrgAdminFrame>
  )
}

function Fact({ label, value }: { label: string; value: string }) {
  return (
    <div className="min-w-0">
      <dt className="text-[12px] leading-4 text-[#8a8a8a]">{label}</dt>
      <dd className="m-0 mt-1 truncate text-[15px] leading-6 font-medium text-[#171717]">{value}</dd>
    </div>
  )
}

/** 变更套餐。席位低于已用时要再确认一次：超出的人不会被移出，但新成员加不进来。 */
function PlanChangeDialog({
  open,
  orgId,
  currentPlan,
  currentSeats,
  used,
  onOpenChange,
  onChanged
}: {
  open: boolean
  orgId: string
  currentPlan: PlanId
  currentSeats: number
  used: number
  onOpenChange: (open: boolean) => void
  onChanged: () => Promise<void>
}) {
  const { t } = useTranslation()
  const [plan, setPlan] = useState<PlanId>(currentPlan === 'enterprise' ? 'team' : currentPlan)
  const [seats, setSeats] = useState(String(currentSeats))
  const [busy, setBusy] = useState(false)
  const [error, setError] = useState<string | null>(null)
  useEffect(() => {
    if (!open) return
    setPlan(currentPlan === 'enterprise' ? 'team' : currentPlan)
    setSeats(String(currentSeats))
    setError(null)
  }, [open, currentPlan, currentSeats])

  const bounds = PLAN_SEATS[plan]
  const count = Number.parseInt(seats, 10)
  const valid = Number.isInteger(count) && count >= bounds.min && count <= bounds.max
  const belowActive = valid && count < used
  const unchanged = plan === currentPlan && count === currentSeats

  async function submit() {
    if (busy || !valid || belowActive || unchanged) return
    setBusy(true)
    setError(null)
    try {
      await updateOrgSubscription(orgId, plan, count)
      await onChanged()
      toast.success(t('org.plan.changed'))
      onOpenChange(false)
    } catch (err) {
      const key = adminErrorKey(err)
      setError(
        key === 'org.error.generic' && err instanceof Error && err.message.startsWith('seats must be')
          ? t('org.plan.seatsInvalid', bounds)
          : t(key)
      )
    } finally {
      setBusy(false)
    }
  }

  return (
    <Dialog open={open} onOpenChange={onOpenChange}>
      <DialogContent aria-describedby={undefined} className="sm:max-w-[420px]">
        <DialogTitle className="text-[15px] leading-5 font-medium">{t('org.plan.change')}</DialogTitle>
        <form
          data-ui="org.plan.dialog"
          className="grid gap-3"
          onSubmit={(event) => {
            event.preventDefault()
            void submit()
          }}>
          <label className="grid gap-1.5 text-[13px] text-[#3f3f3f]">
            {t('org.plan.col.plan')}
            <SearchSelect
              searchable={false}
              allowClear={false}
              aria-label={t('org.plan.col.plan')}
              value={plan}
              options={SELF_SERVE_PLANS.map((id) => ({ value: id, label: t(`org.plan.name.${id}`) }))}
              onChange={(next) => {
                if (!isPlanId(next) || next === 'enterprise') return
                setPlan(next)
                setSeats(String(Math.min(PLAN_SEATS[next].max, Math.max(PLAN_SEATS[next].min, count || 0))))
              }}
            />
          </label>
          <label className="grid gap-1.5 text-[13px] text-[#3f3f3f]" htmlFor="org-plan-seats">
            {t('org.plan.col.seats')}
            <Input
              id="org-plan-seats"
              inputMode="numeric"
              value={seats}
              disabled={busy || bounds.min === bounds.max}
              aria-invalid={!valid || undefined}
              className={orgFieldClassName}
              onChange={(event) => setSeats(event.target.value.replace(/\D/g, ''))}
            />
            <span className="text-[12px] text-[#8a8a8a]">{t('org.plan.seatsRange', bounds)}</span>
          </label>
          {belowActive ? (
            <p role="alert" className="m-0 rounded-[8px] bg-[#fff7e6] px-3 py-2 text-[12px] leading-5 text-[#8a5a00]">
              {t('org.plan.belowActive', { used })}
            </p>
          ) : null}
          {error ? (
            <p role="alert" className="m-0 text-[12px] text-[#b42318]">
              {error}
            </p>
          ) : null}
          <div className="flex justify-end gap-2">
            <button
              type="button"
              disabled={busy}
              onClick={() => onOpenChange(false)}
              className={quietButtonClassName(36)}>
              {t('common.cancel')}
            </button>
            <button
              type="submit"
              disabled={busy || !valid || belowActive || unchanged}
              className={filledButtonClassName(36)}>
              {t('org.plan.confirm')}
            </button>
          </div>
        </form>
      </DialogContent>
    </Dialog>
  )
}
