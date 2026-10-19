import { Combobox, type ComboboxOption, ConfirmDialog, Dialog, DialogContent, DialogTitle, Input } from '@a3s-lab/ui'
import { useEffect, useMemo, useRef, useState } from 'react'
import { useTranslation } from 'react-i18next'
import useSWR from 'swr'

import { TeamLogoField } from '@renderer/components/account/AccountDialogs'
import { textLogo } from '@renderer/components/account/entityImage'
import { SettingGroup, SettingRow, SettingRowTitle, SettingTitle } from '@renderer/components/SettingsPrimitives'
import { orgAllows, PERSONAL_CENTER, writeCenterId, type MemberOrg } from '@renderer/data/memberOrgs'
import { toast } from '@renderer/services/toast'
import { cn } from '@renderer/utils/style'

import {
  dissolveOrg,
  leaveOrg,
  listTransferCandidates,
  settingsErrorKey,
  transferOrg,
  updateOrgProfile,
  type OrgMemberRow
} from './orgApi'
import { OrgDirectory } from './OrgCatalog'
import { filledButtonClassName, orgComboboxClassName, orgFieldClassName, quietButtonClassName } from './orgUi'
import { OrgVerificationCard } from './OrgVerificationCard'

export const ORG_INDUSTRIES = [
  'internet',
  'software',
  'manufacturing',
  'finance',
  'education',
  'healthcare',
  'retail',
  'media',
  'government',
  'services',
  'other'
] as const

export const ORG_SCALES = ['1-20', '21-99', '100-499', '500-999', '1000+'] as const

const NAME_MAX = 64

/** 生成的文字 Logo 是 SVG；上传的图片是位图。改名时文字 Logo 跟着名称重新生成。 */
function isTextLogo(logo: string) {
  return !logo || logo.startsWith('data:image/svg+xml')
}

function choiceOptions(codes: readonly string[], current: string, label: (code: string) => string) {
  const options = codes.map((code) => ({ value: code, label: label(code) }))
  if (current && !codes.includes(current)) options.push({ value: current, label: current })
  return options
}

/** 团队设置：基本信息、企业认证、危险操作（退出、转交、解散）。全部直连 a3s os 的成员面接口。 */
export function OrgSettingsPanel({
  org,
  members = [],
  onChanged
}: {
  org: MemberOrg
  members?: ReadonlyArray<Pick<OrgMemberRow, 'id' | 'avatar'>>
  onChanged: () => Promise<unknown>
}) {
  const { t } = useTranslation()
  const canEdit = org.canUpdateOrg === true && org.dataScope?.kind === 'all' && orgAllows(org, 'update', 'Organization')
  const canTransfer = org.canTransferOrg === true && org.creator === true && orgAllows(org, 'transfer', 'Organization')
  const canDissolve = org.canDeleteOrg === true && org.creator === true && orgAllows(org, 'delete', 'Organization')
  const canLeave = org.canLeaveOrg === true && !org.creator && org.membership !== 'pending'
  const authority = JSON.stringify([
    org.id,
    org.membership,
    org.creator,
    org.rules,
    org.dataScope,
    org.canUpdateOrg,
    org.canTransferOrg,
    org.canDeleteOrg,
    org.canLeaveOrg
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
  const current = () => mounted.current && epoch.current === generation && policy.current === authority

  const [name, setName] = useState(org.name)
  const [logo, setLogo] = useState(isTextLogo(org.logo) ? '' : org.logo)
  const [industry, setIndustry] = useState(org.industry ?? '')
  const [scale, setScale] = useState(org.scale ?? '')
  const [nameError, setNameError] = useState<string | null>(null)
  const [saving, setSaving] = useState(false)
  const [exiting, setExiting] = useState(false)
  const [leaveOpen, setLeaveOpen] = useState(false)
  const [dissolveOpen, setDissolveOpen] = useState(false)
  const [dissolveName, setDissolveName] = useState('')
  const [transferOpen, setTransferOpen] = useState(false)

  useEffect(() => {
    setName(org.name)
    setLogo(isTextLogo(org.logo) ? '' : org.logo)
    setIndustry(org.industry ?? '')
    setScale(org.scale ?? '')
    setNameError(null)
    setSaving(false)
    setExiting(false)
    setLeaveOpen(false)
    setDissolveOpen(false)
    setDissolveName('')
    setTransferOpen(false)
  }, [authority, org.name, org.logo, org.industry, org.scale])

  const trimmed = name.trim()
  const savedLogo = isTextLogo(org.logo) ? '' : org.logo
  const dirty =
    trimmed !== org.name || logo !== savedLogo || industry !== (org.industry ?? '') || scale !== (org.scale ?? '')

  const industryOptions = useMemo(
    () => choiceOptions(ORG_INDUSTRIES, industry, (code) => t(`org.settings.industries.${code}`)),
    [industry, t]
  )
  const scaleOptions = useMemo(
    () => choiceOptions(ORG_SCALES, scale, (code) => t(`org.settings.scales.${code}`)),
    [scale, t]
  )

  async function save() {
    if (!canEdit || saving || !current()) return
    if (!trimmed) {
      setNameError(t('org.settings.nameRequired'))
      return
    }
    setSaving(true)
    try {
      await updateOrgProfile(org.id, { name: trimmed, industry, scale, logo: logo || textLogo(trimmed) })
      if (!current()) return
      await onChanged()
      if (!current()) return
      toast.success(t('org.settings.saved'))
    } catch (err) {
      if (current()) toast.error(t(settingsErrorKey(err)))
    } finally {
      if (current()) setSaving(false)
    }
  }

  async function exitTeam(kind: 'leave' | 'transfer' | 'delete', action: () => Promise<unknown>, successKey: string) {
    const allowed = kind === 'leave' ? canLeave : kind === 'transfer' ? canTransfer : canDissolve
    if (!allowed || exiting || !current()) return false
    setExiting(true)
    try {
      await action()
      if (!current()) return false
      writeCenterId(PERSONAL_CENTER)
      toast.success(t(successKey))
      await onChanged()
      return true
    } catch (err) {
      if (current()) toast.error(t(settingsErrorKey(err)))
      return false
    } finally {
      if (current()) setExiting(false)
    }
  }

  return (
    <div data-ui="org.settings" className="flex min-h-0 min-w-0 flex-1 flex-col">
      <OrgDirectory
        title={t('org.nav.settings')}
        description={t('org.settings.description')}
        treeLabel={t('org.nav.settings')}
        detailLabel={t('org.nav.settings')}
        detail={
          <div className="h-full overflow-y-auto">
            <div className="grid max-w-[720px] gap-4 pb-6">
              <SettingGroup data-ui="org.settings.profile" className="mt-0 grid gap-4">
                <SettingTitle className="text-[15px] font-medium text-[#171717]">
                  {t('org.settings.profile')}
                </SettingTitle>
                {canEdit ? (
                  <form
                    className="grid gap-4"
                    onSubmit={(event) => {
                      event.preventDefault()
                      void save()
                    }}>
                    <TeamLogoField
                      name={trimmed || org.name}
                      logo={logo}
                      disabled={saving}
                      onChange={setLogo}
                      onError={(message) => toast.error(message)}
                    />
                    <label className="grid gap-1.5 text-[13px] text-[#3f3f3f]" htmlFor="org-settings-name">
                      {t('org.settings.name')}
                      <Input
                        id="org-settings-name"
                        value={name}
                        disabled={saving}
                        maxLength={NAME_MAX}
                        aria-invalid={nameError ? true : undefined}
                        className={orgFieldClassName}
                        onChange={(event) => {
                          setName(event.target.value)
                          setNameError(null)
                        }}
                      />
                      {nameError ? (
                        <span role="alert" className="text-[12px] text-destructive">
                          {nameError}
                        </span>
                      ) : null}
                    </label>
                    <div className="grid gap-4 sm:grid-cols-2">
                      <div className="grid gap-1.5 text-[13px] text-[#3f3f3f]">
                        <span id="org-settings-industry">{t('org.settings.industry')}</span>
                        <Combobox
                          aria-labelledby="org-settings-industry"
                          options={industryOptions}
                          value={industry}
                          searchable={false}
                          disabled={saving}
                          placeholder={t('org.settings.unset')}
                          className={orgComboboxClassName}
                          onChange={(value) => setIndustry(typeof value === 'string' ? value : '')}
                        />
                      </div>
                      <div className="grid gap-1.5 text-[13px] text-[#3f3f3f]">
                        <span id="org-settings-scale">{t('org.settings.scale')}</span>
                        <Combobox
                          aria-labelledby="org-settings-scale"
                          options={scaleOptions}
                          value={scale}
                          searchable={false}
                          disabled={saving}
                          placeholder={t('org.settings.unset')}
                          className={orgComboboxClassName}
                          onChange={(value) => setScale(typeof value === 'string' ? value : '')}
                        />
                      </div>
                    </div>
                    <div className="flex justify-end gap-2">
                      <button
                        type="button"
                        disabled={saving || !dirty}
                        className={quietButtonClassName(30)}
                        onClick={() => {
                          setName(org.name)
                          setLogo(savedLogo)
                          setIndustry(org.industry ?? '')
                          setScale(org.scale ?? '')
                          setNameError(null)
                        }}>
                        {t('common.reset')}
                      </button>
                      <button
                        type="submit"
                        disabled={saving || !dirty || !trimmed}
                        className={filledButtonClassName(30)}>
                        {t('common.save')}
                      </button>
                    </div>
                  </form>
                ) : (
                  <dl className="m-0 grid gap-0.5">
                    <ReadOnlyRow label={t('org.settings.name')} value={org.name} />
                    <ReadOnlyRow
                      label={t('org.settings.industry')}
                      value={org.industry ? industryOptions.find((item) => item.value === org.industry)?.label : ''}
                    />
                    <ReadOnlyRow
                      label={t('org.settings.scale')}
                      value={org.scale ? scaleOptions.find((item) => item.value === org.scale)?.label : ''}
                    />
                  </dl>
                )}
                <SettingRow className="border-t border-[#f0f0f0] pt-3">
                  <SettingRowTitle>{t('org.settings.memberCount')}</SettingRowTitle>
                  <span className="text-[13px] text-[#171717]">
                    {t('org.members.count', { n: org.memberCount ?? 0 })}
                  </span>
                </SettingRow>
              </SettingGroup>

              <OrgVerificationCard org={org} />

              {canLeave || canTransfer || canDissolve ? (
                <SettingGroup data-ui="org.settings.danger" className="mt-0 grid gap-1">
                  <SettingTitle className="mb-2 text-[15px] font-medium text-[#171717]">
                    {t('org.settings.danger')}
                  </SettingTitle>
                  {canLeave ? (
                    <ActionRow
                      title={t('org.settings.leave')}
                      description={t('org.settings.leaveHint')}
                      action={
                        <button type="button" className={quietButtonClassName(30)} onClick={() => setLeaveOpen(true)}>
                          {t('org.settings.leaveCta')}
                        </button>
                      }
                    />
                  ) : null}
                  {canTransfer ? (
                    <ActionRow
                      title={t('org.settings.transfer')}
                      description={t('org.settings.transferHint')}
                      action={
                        <button
                          type="button"
                          className={quietButtonClassName(30)}
                          onClick={() => setTransferOpen(true)}>
                          {t('org.settings.transferCta')}
                        </button>
                      }
                    />
                  ) : null}
                  {canDissolve ? (
                    <ActionRow
                      title={t('org.settings.dissolve')}
                      description={t('org.settings.dissolveHint')}
                      action={
                        <button
                          type="button"
                          className={cn(quietButtonClassName(30), 'text-[#b42318] hover:text-[#b42318]')}
                          onClick={() => {
                            setDissolveName('')
                            setDissolveOpen(true)
                          }}>
                          {t('org.settings.dissolveCta')}
                        </button>
                      }
                    />
                  ) : null}
                </SettingGroup>
              ) : null}
            </div>
          </div>
        }
      />

      <ConfirmDialog
        open={leaveOpen && canLeave}
        operationKey={`${org.id}:${generation}:leave`}
        failureText={t('common.save_failed')}
        confirmDisabled={exiting || !canLeave}
        onOpenChange={setLeaveOpen}
        destructive
        title={t('org.settings.leaveConfirmTitle', { name: org.name })}
        description={t('org.settings.leaveConfirmBody')}
        confirmText={t('org.settings.leaveCta')}
        cancelText={t('common.cancel')}
        onConfirm={() => exitTeam('leave', () => leaveOrg(org.id), 'org.settings.left')}
      />

      <ConfirmDialog
        open={dissolveOpen && canDissolve}
        operationKey={`${org.id}:${generation}:dissolve`}
        failureText={t('common.delete_failed')}
        onOpenChange={setDissolveOpen}
        destructive
        title={t('org.settings.dissolveConfirmTitle', { name: org.name })}
        description={t('org.settings.dissolveConfirmBody')}
        confirmText={t('org.settings.dissolveCta')}
        cancelText={t('common.cancel')}
        confirmDisabled={exiting || !canDissolve}
        contentClassName="sm:max-w-sm"
        content={
          <label className="grid gap-1.5 text-[13px] text-[#3f3f3f]" htmlFor="org-dissolve-name">
            {t('org.settings.dissolveTypeName', { name: org.name })}
            <Input
              id="org-dissolve-name"
              value={dissolveName}
              autoComplete="off"
              className={orgFieldClassName}
              onChange={(event) => setDissolveName(event.target.value)}
            />
          </label>
        }
        onConfirm={() => {
          const field = document.getElementById('org-dissolve-name')
          const shown = field instanceof HTMLInputElement ? field.value : dissolveName
          return shown.trim() === org.name
            ? exitTeam('delete', () => dissolveOrg(org.id), 'org.settings.dissolved')
            : false
        }}
      />

      <TransferDialog
        key={`${authority}:${generation}`}
        authority={`${authority}:${generation}`}
        open={transferOpen && canTransfer}
        orgId={org.id}
        members={members}
        onOpenChange={setTransferOpen}
        onTransfer={(userId) => exitTeam('transfer', () => transferOrg(org.id, userId), 'org.settings.transferred')}
      />
    </div>
  )
}

function ReadOnlyRow({ label, value }: { label: string; value?: string }) {
  const { t } = useTranslation()
  return (
    <SettingRow>
      <dt>
        <SettingRowTitle>{label}</SettingRowTitle>
      </dt>
      <dd className="m-0 min-w-0 truncate text-[13px] text-[#171717]">{value?.trim() || t('org.settings.unset')}</dd>
    </SettingRow>
  )
}

function ActionRow({ title, description, action }: { title: string; description: string; action: React.ReactNode }) {
  return (
    <SettingRow className="py-2">
      <div className="grid min-w-0 flex-1 gap-0.5">
        <span className="text-[13px] leading-5 text-[#171717]">{title}</span>
        <span className="text-[12px] leading-5 text-[#8a8a8a]">{description}</span>
      </div>
      {action}
    </SettingRow>
  )
}

type TransferOption = ComboboxOption<{ phone?: string; avatar?: string }>

function transferPhoto(avatar: string | undefined): string {
  const value = avatar?.trim() ?? ''
  return value.startsWith('data:image/') && value.includes(';base64,') ? value : ''
}

function TransferFace({ name, avatar }: { name: string; avatar?: string }) {
  const photo = transferPhoto(avatar)
  const initial = [...name.trim()][0] ?? ''
  return (
    <span
      data-ui="org.transfer.avatar"
      role="img"
      aria-label={name.trim() || undefined}
      className="flex size-6 shrink-0 items-center justify-center overflow-hidden rounded-[8px] bg-[#f2f3f5] text-[12px] font-medium text-[#3f3f3f]">
      {photo ? <img src={photo} alt="" draggable={false} className="size-full object-cover" /> : initial}
    </span>
  )
}

function matchesTransferOption(option: TransferOption, search: string): boolean {
  const needle = search.trim().toLowerCase()
  if (!needle) return true
  const phone = option.phone ?? option.description ?? ''
  return option.label.toLowerCase().includes(needle) || phone.toLowerCase().includes(needle)
}

function TransferDialog({
  open,
  orgId,
  authority,
  members,
  onOpenChange,
  onTransfer
}: {
  open: boolean
  orgId: string
  authority: string
  members: ReadonlyArray<Pick<OrgMemberRow, 'id' | 'avatar'>>
  onOpenChange: (open: boolean) => void
  onTransfer: (userId: string) => Promise<boolean>
}) {
  const { t } = useTranslation()
  const { data, error, isLoading } = useSWR(
    open ? `studio/org-transfer:${orgId}:${authority}` : null,
    () => listTransferCandidates(orgId),
    { dedupingInterval: 0 }
  )
  const [target, setTarget] = useState('')
  const [busy, setBusy] = useState(false)
  const mounted = useRef(true)
  const requestEpoch = useRef(0)
  const isOpen = useRef(open)
  if (isOpen.current !== open) {
    isOpen.current = open
    requestEpoch.current += 1
  }
  useEffect(() => {
    mounted.current = true
    return () => {
      mounted.current = false
    }
  }, [])

  useEffect(() => {
    setTarget('')
    setBusy(false)
  }, [open])

  const directoryAvatars = useMemo(() => {
    const byId = new Map<string, string>()
    for (const member of members) {
      const avatar = transferPhoto(member.avatar)
      if (avatar) byId.set(member.id, avatar)
    }
    return byId
  }, [members])

  const options: TransferOption[] = (data ?? []).map((row) => ({
    value: row.id,
    label: row.name,
    description: row.phone ?? undefined,
    phone: row.phone ?? undefined,
    avatar: transferPhoto(row.avatar) || directoryAvatars.get(row.id)
  }))

  return (
    <Dialog open={open} onOpenChange={onOpenChange}>
      <DialogContent aria-describedby={undefined} className="sm:max-w-sm">
        <DialogTitle className="text-[15px] leading-5 font-medium">{t('org.settings.transfer')}</DialogTitle>
        <form
          className="grid gap-3"
          onSubmit={async (event) => {
            event.preventDefault()
            if (!open || busy || !options.some((option) => option.value === target)) return
            const generation = requestEpoch.current
            setBusy(true)
            const done = await onTransfer(target)
            if (!mounted.current || requestEpoch.current !== generation) return
            setBusy(false)
            if (done) onOpenChange(false)
          }}>
          <p className="m-0 text-[13px] leading-5 text-[#8a8a8a]">{t('org.settings.transferBody')}</p>
          {error && !data ? (
            <p className="m-0 text-[13px] text-destructive">{t(settingsErrorKey(error))}</p>
          ) : isLoading && !data ? (
            <p className="m-0 text-[13px] text-[#8a8a8a]">{t('common.loading')}</p>
          ) : options.length === 0 ? (
            <p className="m-0 text-[13px] text-[#8a8a8a]">{t('org.settings.transferEmpty')}</p>
          ) : (
            <div className="grid gap-1.5 text-[13px] text-[#3f3f3f]">
              <span id="org-transfer-target">{t('org.settings.transferTarget')}</span>
              <Combobox
                aria-labelledby="org-transfer-target"
                options={options}
                value={target}
                disabled={busy}
                filterOption={matchesTransferOption}
                searchPlaceholder={t('org.members.search')}
                emptyText={t('org.members.noMatch')}
                placeholder={t('org.settings.transferPick')}
                className={orgComboboxClassName}
                popoverAlign="start"
                popoverClassName="w-(--radix-popover-trigger-width) rounded-[8px] border border-[#e5e5e5] bg-white p-0 shadow-[0_12px_32px_rgba(0,0,0,0.12)] dark:border-[#3a3a3a] dark:bg-[#1c1c1c]"
                commandClassName="rounded-[8px] bg-white dark:bg-[#1c1c1c]"
                searchWrapperClassName="mx-1.5 mt-1.5 h-[30px] gap-2 rounded-[8px] border-0 border-b-0 bg-[#f5f5f5] px-2.5 dark:bg-[#2c2c2c] [&_svg]:size-3.5 [&_svg]:text-[#8a8a8a] [&_svg]:opacity-100"
                searchClassName="h-[30px] bg-transparent py-0 text-[13px] placeholder:text-[#8a8a8a]"
                itemClassName="h-9 gap-2.5 rounded-[8px] px-2.5 py-0 text-[13px] data-[selected=true]:font-medium"
                emptyClassName="py-6 text-center text-[13px] text-[#8a8a8a]"
                renderValue={(value, rows) => {
                  const selected = rows.find((row) => row.value === value)
                  if (!selected || typeof value !== 'string' || !value) {
                    return <span className="truncate text-[#8a8a8a]">{t('org.settings.transferPick')}</span>
                  }
                  return (
                    <span className="flex min-w-0 items-center gap-2.5">
                      <TransferFace name={selected.label} avatar={selected.avatar} />
                      <span className="truncate">{selected.label}</span>
                    </span>
                  )
                }}
                renderOption={(option) => (
                  <>
                    <TransferFace name={option.label} avatar={option.avatar} />
                    <span className="min-w-0 flex-1 truncate">{option.label}</span>
                    {option.phone ? (
                      <span className="shrink-0 text-[12px] font-normal text-[#8a8a8a]">{option.phone}</span>
                    ) : null}
                  </>
                )}
                onChange={(value) => setTarget(typeof value === 'string' ? value : '')}
              />
            </div>
          )}
          <div className="flex justify-end gap-2">
            <button
              type="button"
              disabled={busy}
              className={quietButtonClassName(36)}
              onClick={() => onOpenChange(false)}>
              {t('common.cancel')}
            </button>
            <button type="submit" disabled={busy || !target} className={filledButtonClassName(36)}>
              {t('org.settings.transferCta')}
            </button>
          </div>
        </form>
      </DialogContent>
    </Dialog>
  )
}
