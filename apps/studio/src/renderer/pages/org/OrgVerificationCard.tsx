import { Dialog, DialogContent, DialogTitle, Input } from '@a3s-lab/ui'
import { useEffect, useRef, useState } from 'react'
import { useTranslation } from 'react-i18next'
import useSWR from 'swr'

import { SettingGroup, SettingRow, SettingRowTitle, SettingTitle } from '@renderer/components/SettingsPrimitives'
import type { MemberOrg } from '@renderer/data/memberOrgs'
import { toast } from '@renderer/services/toast'
import { cn } from '@renderer/utils/style'

import {
  adminErrorKey,
  fetchOrgVerification,
  fetchPersonalVerification,
  isCreditCode,
  submitOrgVerification,
  submitPersonalVerification,
  type OrgVerification,
  type VerificationStatus
} from './orgAdminApi'
import { filledButtonClassName, orgFieldClassName, quietButtonClassName } from './orgUi'

const STATUS_TONE: Record<VerificationStatus, string> = {
  unverified: 'bg-[#f2f3f5] text-[#5c5c5c]',
  pending: 'bg-[#fff6e5] text-[#a15c00]',
  verified: 'bg-[#e8f7ee] text-[#18794e]',
  rejected: 'bg-[#fdecec] text-[#b42318]'
}

/** 18 位居民身份证的形状；出生日期与校验位由服务端判定。 */
const RESIDENT_ID = /^\d{17}[\dX]$/

function StatusBadge({ status }: { status: VerificationStatus }) {
  const { t } = useTranslation()
  return (
    <span
      data-ui="org.verification.status"
      data-status={status}
      className={cn('inline-flex h-6 items-center rounded-full px-2.5 text-[12px] font-medium', STATUS_TONE[status])}>
      {t(`org.verification.status.${status}`)}
    </span>
  )
}

function FactRow({ label, value }: { label: string; value: string }) {
  const { t } = useTranslation()
  return (
    <SettingRow>
      <dt>
        <SettingRowTitle>{label}</SettingRowTitle>
      </dt>
      <dd className="m-0 min-w-0 truncate text-[13px] text-[#171717]">{value.trim() || t('org.settings.unset')}</dd>
    </SettingRow>
  )
}

/** 企业认证：任何在职成员可见状态；只有创建者在个人认证通过后可以提交。 */
export function OrgVerificationCard({ org }: { org: MemberOrg }) {
  const { t } = useTranslation()
  const creator = org.creator === true
  const { data, error, isLoading, mutate } = useSWR(`studio/org-verification:${org.id}`, () =>
    fetchOrgVerification(org.id)
  )
  const needsPersonal = creator && (data?.status === 'unverified' || data?.status === 'rejected')
  const personal = useSWR(needsPersonal ? 'studio/member-verification' : null, fetchPersonalVerification)
  const [submitOpen, setSubmitOpen] = useState(false)
  const [personalOpen, setPersonalOpen] = useState(false)

  useEffect(() => {
    setSubmitOpen(false)
    setPersonalOpen(false)
  }, [org.id])

  const status = data?.status
  const personalStatus = personal.data?.status
  const canSubmit = needsPersonal && personalStatus === 'verified'

  let body: React.ReactNode
  if (error && !data) {
    body = (
      <div className="flex items-center justify-between gap-3">
        <p className="m-0 text-[13px] text-destructive">{t(adminErrorKey(error))}</p>
        <button type="button" className={quietButtonClassName(30)} onClick={() => void mutate()}>
          {t('common.retry')}
        </button>
      </div>
    )
  } else if (isLoading || !data) {
    body = <p className="m-0 text-[13px] text-[#8a8a8a]">{t('common.loading')}</p>
  } else {
    const showFacts = data.status !== 'unverified'
    body = (
      <div className="grid gap-3">
        <p className="m-0 text-[13px] leading-5 text-[#8a8a8a]">{t(`org.verification.hint.${data.status}`)}</p>
        {data.status === 'rejected' && data.reject_reason ? (
          <p
            data-ui="org.verification.reject-reason"
            className="m-0 rounded-[8px] bg-[#fdf3f2] px-3 py-2 text-[13px] leading-5 text-[#b42318]">
            {t('org.verification.rejectReason', { reason: data.reject_reason })}
          </p>
        ) : null}
        {showFacts ? (
          <dl className="m-0 grid gap-0.5">
            <FactRow label={t('org.verification.enterpriseName')} value={data.enterprise_name} />
            <FactRow label={t('org.verification.creditCode')} value={data.credit_code} />
            <FactRow label={t('org.verification.legalPerson')} value={data.legal_person} />
          </dl>
        ) : null}
        {needsPersonal ? (
          <PersonalGate
            loading={personal.isLoading && !personal.data}
            failed={Boolean(personal.error) && !personal.data}
            status={personalStatus}
            resubmit={data.status === 'rejected'}
            onPersonal={() => setPersonalOpen(true)}
            onSubmit={() => setSubmitOpen(true)}
          />
        ) : !creator && (data.status === 'unverified' || data.status === 'rejected') ? (
          <p className="m-0 text-[12px] leading-5 text-[#8a8a8a]">{t('org.verification.creatorOnlyHint')}</p>
        ) : null}
      </div>
    )
  }

  return (
    <SettingGroup data-ui="org.settings.verification" className="mt-0 grid gap-3">
      <div className="flex items-center justify-between gap-3">
        <SettingTitle className="text-[15px] font-medium text-[#171717]">{t('org.verification.title')}</SettingTitle>
        {status ? <StatusBadge status={status} /> : null}
      </div>
      {body}
      <SubmitDialog
        key={`${org.id}:${submitOpen}`}
        open={submitOpen && canSubmit}
        orgId={org.id}
        previous={data}
        onOpenChange={setSubmitOpen}
        onSubmitted={(next) => mutate(next, { revalidate: false })}
      />
      <PersonalDialog
        key={`personal:${org.id}:${personalOpen}`}
        open={personalOpen && needsPersonal && personalStatus !== 'verified' && personalStatus !== 'pending'}
        onOpenChange={setPersonalOpen}
        onSubmitted={(next) => personal.mutate(next, { revalidate: false })}
      />
    </SettingGroup>
  )
}

function PersonalGate({
  loading,
  failed,
  status,
  resubmit,
  onPersonal,
  onSubmit
}: {
  loading: boolean
  failed: boolean
  status: VerificationStatus | undefined
  resubmit: boolean
  onPersonal: () => void
  onSubmit: () => void
}) {
  const { t } = useTranslation()
  if (loading) return <p className="m-0 text-[13px] text-[#8a8a8a]">{t('common.loading')}</p>
  if (failed) return <p className="m-0 text-[13px] text-destructive">{t('org.error.generic')}</p>
  if (status === 'verified') {
    return (
      <div className="flex justify-end">
        <button type="button" className={filledButtonClassName(30)} onClick={onSubmit}>
          {t(resubmit ? 'org.verification.resubmit' : 'org.verification.submit')}
        </button>
      </div>
    )
  }
  return (
    <div
      data-ui="org.verification.personal-gate"
      className="flex items-center justify-between gap-3 rounded-[8px] bg-[#f7f7f8] px-3 py-2.5">
      <span className="text-[13px] leading-5 text-[#3f3f3f]">
        {t(status === 'pending' ? 'org.verification.personalPending' : 'org.verification.personalRequired')}
      </span>
      {status === 'pending' ? null : (
        <button type="button" className={quietButtonClassName(30)} onClick={onPersonal}>
          {t('org.verification.goPersonal')}
        </button>
      )}
    </div>
  )
}

function Field({
  id,
  label,
  value,
  error,
  disabled,
  onChange,
  maxLength
}: {
  id: string
  label: string
  value: string
  error: string | null
  disabled: boolean
  onChange: (value: string) => void
  maxLength: number
}) {
  const errorId = `${id}-error`
  return (
    <div className="grid gap-1.5 text-[13px] text-[#3f3f3f]">
      <label htmlFor={id}>{label}</label>
      <Input
        id={id}
        value={value}
        disabled={disabled}
        maxLength={maxLength}
        autoComplete="off"
        aria-invalid={error ? true : undefined}
        aria-describedby={error ? errorId : undefined}
        className={orgFieldClassName}
        onChange={(event) => onChange(event.target.value)}
      />
      {error ? (
        <span id={errorId} role="alert" className="text-[12px] text-destructive">
          {error}
        </span>
      ) : null}
    </div>
  )
}

function useMounted() {
  const mounted = useRef(true)
  useEffect(() => {
    mounted.current = true
    return () => {
      mounted.current = false
    }
  }, [])
  return mounted
}

function SubmitDialog({
  open,
  orgId,
  previous,
  onOpenChange,
  onSubmitted
}: {
  open: boolean
  orgId: string
  previous: OrgVerification | undefined
  onOpenChange: (open: boolean) => void
  onSubmitted: (next: OrgVerification) => unknown
}) {
  const { t } = useTranslation()
  const mounted = useMounted()
  const [enterpriseName, setEnterpriseName] = useState(previous?.enterprise_name ?? '')
  const [creditCode, setCreditCode] = useState(previous?.credit_code ?? '')
  const [legalPerson, setLegalPerson] = useState(previous?.legal_person ?? '')
  const [creditError, setCreditError] = useState<string | null>(null)
  const [busy, setBusy] = useState(false)
  const complete = enterpriseName.trim().length >= 2 && creditCode.trim() !== '' && legalPerson.trim() !== ''

  async function submit() {
    if (busy || !complete) return
    if (!isCreditCode(creditCode)) {
      setCreditError(t('org.verification.creditInvalid'))
      return
    }
    setBusy(true)
    try {
      const next = await submitOrgVerification(orgId, {
        enterprise_name: enterpriseName.trim(),
        credit_code: creditCode.trim().toUpperCase(),
        legal_person: legalPerson.trim()
      })
      if (!mounted.current) return
      await onSubmitted(next)
      toast.success(t('org.verification.submitted'))
      onOpenChange(false)
    } catch (err) {
      if (mounted.current) toast.error(t(adminErrorKey(err)))
    } finally {
      if (mounted.current) setBusy(false)
    }
  }

  return (
    <Dialog open={open} onOpenChange={onOpenChange}>
      <DialogContent aria-describedby={undefined} className="sm:max-w-md">
        <DialogTitle className="text-[15px] leading-5 font-medium">{t('org.verification.submitTitle')}</DialogTitle>
        <form
          data-ui="org.verification.form"
          className="grid gap-3"
          onSubmit={(event) => {
            event.preventDefault()
            void submit()
          }}>
          <p className="m-0 text-[13px] leading-5 text-[#8a8a8a]">{t('org.verification.submitBody')}</p>
          <Field
            id="org-verification-name"
            label={t('org.verification.enterpriseName')}
            value={enterpriseName}
            error={null}
            disabled={busy}
            maxLength={64}
            onChange={setEnterpriseName}
          />
          <Field
            id="org-verification-credit"
            label={t('org.verification.creditCode')}
            value={creditCode}
            error={creditError}
            disabled={busy}
            maxLength={18}
            onChange={(value) => {
              setCreditCode(value)
              setCreditError(null)
            }}
          />
          <Field
            id="org-verification-legal"
            label={t('org.verification.legalPerson')}
            value={legalPerson}
            error={null}
            disabled={busy}
            maxLength={32}
            onChange={setLegalPerson}
          />
          <div className="flex justify-end gap-2">
            <button
              type="button"
              disabled={busy}
              className={quietButtonClassName(36)}
              onClick={() => onOpenChange(false)}>
              {t('common.cancel')}
            </button>
            <button type="submit" disabled={busy || !complete} className={filledButtonClassName(36)}>
              {t('org.verification.submit')}
            </button>
          </div>
        </form>
      </DialogContent>
    </Dialog>
  )
}

function PersonalDialog({
  open,
  onOpenChange,
  onSubmitted
}: {
  open: boolean
  onOpenChange: (open: boolean) => void
  onSubmitted: (next: Awaited<ReturnType<typeof submitPersonalVerification>>) => unknown
}) {
  const { t } = useTranslation()
  const mounted = useMounted()
  const [realName, setRealName] = useState('')
  const [idNumber, setIdNumber] = useState('')
  const [idError, setIdError] = useState<string | null>(null)
  const [busy, setBusy] = useState(false)
  const complete = realName.trim().length >= 2 && idNumber.trim() !== ''

  async function submit() {
    if (busy || !complete) return
    const id = idNumber.trim().toUpperCase()
    if (!RESIDENT_ID.test(id)) {
      setIdError(t('org.verification.idInvalid'))
      return
    }
    setBusy(true)
    try {
      const next = await submitPersonalVerification({ real_name: realName.trim(), id_number: id })
      if (!mounted.current) return
      setIdNumber('')
      await onSubmitted(next)
      toast.success(t('org.verification.personalSubmitted'))
      onOpenChange(false)
    } catch (err) {
      if (mounted.current) toast.error(t(adminErrorKey(err)))
    } finally {
      if (mounted.current) setBusy(false)
    }
  }

  return (
    <Dialog open={open} onOpenChange={onOpenChange}>
      <DialogContent aria-describedby={undefined} className="sm:max-w-sm">
        <DialogTitle className="text-[15px] leading-5 font-medium">{t('org.verification.personalTitle')}</DialogTitle>
        <form
          data-ui="org.verification.personal-form"
          className="grid gap-3"
          onSubmit={(event) => {
            event.preventDefault()
            void submit()
          }}>
          <p className="m-0 text-[13px] leading-5 text-[#8a8a8a]">{t('org.verification.personalBody')}</p>
          <Field
            id="org-verification-real-name"
            label={t('org.verification.realName')}
            value={realName}
            error={null}
            disabled={busy}
            maxLength={32}
            onChange={setRealName}
          />
          <Field
            id="org-verification-id-number"
            label={t('org.verification.idNumber')}
            value={idNumber}
            error={idError}
            disabled={busy}
            maxLength={18}
            onChange={(value) => {
              setIdNumber(value)
              setIdError(null)
            }}
          />
          <div className="flex justify-end gap-2">
            <button
              type="button"
              disabled={busy}
              className={quietButtonClassName(36)}
              onClick={() => onOpenChange(false)}>
              {t('common.cancel')}
            </button>
            <button type="submit" disabled={busy || !complete} className={filledButtonClassName(36)}>
              {t('org.verification.submit')}
            </button>
          </div>
        </form>
      </DialogContent>
    </Dialog>
  )
}
