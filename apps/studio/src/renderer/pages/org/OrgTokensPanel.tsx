import {
  Checkbox,
  Dialog,
  DialogContent,
  DialogTitle,
  Input,
  PopConfirm,
  rowMotionClassName,
  rowMotionStyle,
  SearchSelect
} from '@a3s-lab/ui'
import { Copy, KeyRound, Plus, Trash2 } from 'lucide-react'
import { useEffect, useRef, useState } from 'react'
import { useTranslation } from 'react-i18next'
import useSWR from 'swr'

import { beginOptimistic, useOptimisticRows } from '@renderer/components/collection/optimisticRows'
import { PageEntry } from '@renderer/components/layout/PageEntry'
import { toast } from '@renderer/services/toast'
import { cn } from '@renderer/utils/style'

import {
  adminErrorKey,
  createAccessToken,
  listAccessTokens,
  revokeAccessToken,
  TOKEN_EXPIRY_DAYS,
  TOKEN_SCOPES,
  tokenState,
  type AccessToken,
  type IssuedAccessToken,
  type TokenScope
} from './orgAdminApi'
import { OrgAdminFrame } from './OrgAdminFrame'
import { filledButtonClassName, orgFieldClassName, quietButtonClassName, rowActionClassName } from './orgUi'

const DASH = '—'
const TOKENS_KEY = 'studio/member-access-tokens'
const tokenId = (row: AccessToken) => row.id
/** i18next 把冒号当命名空间分隔符。 */
const scopeKey = (scope: string) => scope.replace(':', '_')

/** 个人访问令牌：给 git 和脚本用。令牌属于账号，不属于某个团队；成员停用或离职后对本团队立即失效。 */
export function OrgTokensPanel() {
  const { t } = useTranslation()
  const { data: tokens, error, mutate } = useSWR(TOKENS_KEY, () => listAccessTokens())
  const [creating, setCreating] = useState(false)
  const [issued, setIssued] = useState<IssuedAccessToken | null>(null)
  const [pendingId, setPendingId] = useState<string | null>(null)
  const mounted = useRef(true)
  useEffect(() => {
    mounted.current = true
    return () => {
      mounted.current = false
    }
  }, [])
  const collection = 'member/access-tokens'
  const optimistic = useOptimisticRows(collection, tokens ?? [], tokenId)

  async function revoke(token: AccessToken) {
    if (pendingId !== null || tokenState(token) !== 'active') return
    setPendingId(token.id)
    const handle = beginOptimistic<AccessToken>(collection, {
      patch: { [token.id]: (row) => ({ ...row, revoked_at: new Date().toISOString() }) }
    })
    try {
      await revokeAccessToken(token.id)
      handle.confirm()
      await mutate()
      if (mounted.current) toast.success(t('org.tokens.revoked'))
    } catch (err) {
      handle.rollback()
      if (mounted.current) toast.error(t(adminErrorKey(err)))
    } finally {
      handle.release()
      if (mounted.current) setPendingId(null)
    }
  }

  return (
    <OrgAdminFrame
      ui="org.tokens"
      title={t('org.nav.tokens')}
      description={t('org.tokens.description')}
      actions={
        <button
          type="button"
          data-ui="org.tokens.create"
          className={filledButtonClassName(30)}
          onClick={() => setCreating(true)}>
          <Plus size={16} className="shrink-0 text-current" strokeWidth={1.7} aria-hidden />
          {t('org.tokens.create')}
        </button>
      }>
      {error ? (
        <div className="flex flex-1 flex-col items-center justify-center gap-3">
          <p className="m-0 text-[13px] text-[#8a8a8a]">{t('org.error.generic')}</p>
          <button type="button" onClick={() => void mutate()} className={quietButtonClassName(30)}>
            {t('common.retry')}
          </button>
        </div>
      ) : !tokens ? (
        <div className="relative min-h-40 flex-1">
          <PageEntry id="org.tokens" ready={false} />
        </div>
      ) : optimistic.rows.length === 0 ? (
        <div className="flex flex-1 flex-col items-center justify-center gap-2 py-16 text-center">
          <KeyRound className="size-6 text-[#8a8a8a]" strokeWidth={1.5} aria-hidden />
          <p className="m-0 text-[13px] text-[#3f3f3f]">{t('org.tokens.empty')}</p>
          <p className="m-0 max-w-sm text-[12px] leading-5 text-[#8a8a8a]">{t('org.tokens.emptyHint')}</p>
        </div>
      ) : (
        <table
          data-ui="org.tokens-table"
          aria-label={t('org.nav.tokens')}
          className="w-full min-w-[720px] table-fixed border-collapse text-left">
          <colgroup>
            <col style={{ width: '24%' }} />
            <col style={{ width: '24%' }} />
            <col style={{ width: '12%' }} />
            <col style={{ width: '13%' }} />
            <col style={{ width: '13%' }} />
            <col style={{ width: '14%' }} />
          </colgroup>
          <thead className="sticky top-0 z-[1] bg-white">
            <tr className="border-b border-[#efefef] text-[12px] text-[#8a8a8a]">
              {['name', 'scopes', 'state', 'expires', 'lastUsed', 'actions'].map((column, index, columns) => (
                <th
                  key={column}
                  scope="col"
                  className={cn(
                    'h-9 font-normal',
                    index === 0 && 'pl-2.5',
                    index === columns.length - 1 && 'pr-2.5 text-right'
                  )}>
                  {t(`org.tokens.col.${column}`)}
                </th>
              ))}
            </tr>
          </thead>
          <tbody>
            {optimistic.rows.map((row, index) => {
              const motion = optimistic.motion(row.id)
              const state = tokenState(row)
              return (
                <tr
                  key={row.id}
                  data-motion={motion}
                  aria-busy={motion === 'pending' || undefined}
                  style={rowMotionStyle(index, motion)}
                  className={cn('group hover:bg-[#f7f7f7]', rowMotionClassName(motion))}>
                  <td className="h-12 truncate pl-2.5 text-[13px] font-medium text-[#1a1a1a]">{row.name}</td>
                  <td className="h-12 truncate text-[13px] text-[#3f3f3f]">
                    {row.scopes.map((scope) => t(`org.tokens.scope.${scopeKey(scope)}`)).join('、')}
                  </td>
                  <td
                    className={cn(
                      'h-12 text-[13px] whitespace-nowrap',
                      state === 'active' ? 'text-[#171717]' : 'text-[#8a8a8a]'
                    )}>
                    {t(`org.tokens.state.${state}`)}
                  </td>
                  <td className="h-12 text-[13px] whitespace-nowrap text-[#8a8a8a]">{row.expires_at.slice(0, 10)}</td>
                  <td className="h-12 text-[13px] whitespace-nowrap text-[#8a8a8a]">
                    {row.last_used_at?.slice(0, 10) || t('org.tokens.neverUsed')}
                  </td>
                  <td className="h-12 pr-2.5 text-right">
                    {state === 'active' ? (
                      <PopConfirm
                        tone="danger"
                        title={t('org.tokens.revokeTitle', { name: row.name })}
                        description={t('org.tokens.revokeDesc')}
                        confirmLabel={t('org.tokens.revoke')}
                        cancelLabel={t('common.cancel')}
                        disabled={pendingId !== null}
                        onConfirm={() => void revoke(row)}>
                        <button
                          type="button"
                          aria-label={`${t('org.tokens.revoke')} ${row.name}`}
                          title={t('org.tokens.revoke')}
                          disabled={pendingId !== null}
                          className={rowActionClassName}>
                          <Trash2 className="size-3.5 text-current" strokeWidth={1.7} />
                        </button>
                      </PopConfirm>
                    ) : (
                      DASH
                    )}
                  </td>
                </tr>
              )
            })}
          </tbody>
        </table>
      )}
      <CreateTokenDialog
        open={creating}
        onOpenChange={setCreating}
        onIssued={async (token) => {
          setCreating(false)
          setIssued(token)
          await mutate()
        }}
      />
      <IssuedTokenDialog token={issued} onClose={() => setIssued(null)} />
    </OrgAdminFrame>
  )
}

function CreateTokenDialog({
  open,
  onOpenChange,
  onIssued
}: {
  open: boolean
  onOpenChange: (open: boolean) => void
  onIssued: (token: IssuedAccessToken) => Promise<void>
}) {
  const { t } = useTranslation()
  const [name, setName] = useState('')
  const [scopes, setScopes] = useState<TokenScope[]>(['repository:read'])
  const [days, setDays] = useState<number>(30)
  const [busy, setBusy] = useState(false)
  const [error, setError] = useState<string | null>(null)
  useEffect(() => {
    if (!open) return
    setName('')
    setScopes(['repository:read'])
    setDays(30)
    setError(null)
  }, [open])
  const trimmed = name.trim()
  const valid = trimmed.length > 0 && [...trimmed].length <= 64 && scopes.length > 0

  async function submit() {
    if (busy || !valid) return
    setBusy(true)
    setError(null)
    try {
      const token = await createAccessToken({ name: trimmed, scopes, expires_in_days: days })
      await onIssued(token)
    } catch (err) {
      setError(t(adminErrorKey(err)))
    } finally {
      setBusy(false)
    }
  }

  return (
    <Dialog open={open} onOpenChange={onOpenChange}>
      <DialogContent aria-describedby={undefined} className="sm:max-w-[420px]">
        <DialogTitle className="text-[15px] leading-5 font-medium">{t('org.tokens.create')}</DialogTitle>
        <form
          data-ui="org.tokens.dialog"
          className="grid gap-3"
          onSubmit={(event) => {
            event.preventDefault()
            void submit()
          }}>
          <label className="grid gap-1.5 text-[13px] text-[#3f3f3f]" htmlFor="org-token-name">
            {t('org.tokens.col.name')}
            <Input
              id="org-token-name"
              value={name}
              autoFocus
              autoCapitalize="off"
              autoCorrect="off"
              spellCheck={false}
              maxLength={64}
              disabled={busy}
              placeholder={t('org.tokens.namePlaceholder')}
              className={orgFieldClassName}
              onChange={(event) => setName(event.target.value)}
            />
          </label>
          <fieldset className="m-0 grid gap-2 border-0 p-0">
            <legend className="mb-1.5 p-0 text-[13px] text-[#3f3f3f]">{t('org.tokens.col.scopes')}</legend>
            {TOKEN_SCOPES.map((scope) => (
              <label key={scope} className="flex items-center gap-2 text-[13px] text-[#171717]">
                <Checkbox
                  checked={scopes.includes(scope)}
                  disabled={busy}
                  onCheckedChange={(checked) =>
                    setScopes((current) =>
                      checked === true ? [...new Set([...current, scope])] : current.filter((item) => item !== scope)
                    )
                  }
                />
                <span>{t(`org.tokens.scope.${scopeKey(scope)}`)}</span>
                <span className="text-[12px] text-[#8a8a8a]">{t(`org.tokens.scopeHint.${scopeKey(scope)}`)}</span>
              </label>
            ))}
          </fieldset>
          <label className="grid gap-1.5 text-[13px] text-[#3f3f3f]">
            {t('org.tokens.col.expires')}
            <SearchSelect
              searchable={false}
              allowClear={false}
              aria-label={t('org.tokens.col.expires')}
              value={String(days)}
              options={TOKEN_EXPIRY_DAYS.map((value) => ({
                value: String(value),
                label: t('org.tokens.days', { count: value })
              }))}
              onChange={(next) => {
                const value = Number(next)
                if ((TOKEN_EXPIRY_DAYS as readonly number[]).includes(value)) setDays(value)
              }}
            />
          </label>
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
            <button type="submit" disabled={busy || !valid} className={filledButtonClassName(36)}>
              {t('org.tokens.create')}
            </button>
          </div>
        </form>
      </DialogContent>
    </Dialog>
  )
}

/** 新令牌只显示这一次。关闭前必须先复制或确认已保存。 */
function IssuedTokenDialog({ token, onClose }: { token: IssuedAccessToken | null; onClose: () => void }) {
  const { t } = useTranslation()
  const [saved, setSaved] = useState(false)
  useEffect(() => setSaved(false), [token])

  async function copy() {
    if (!token) return
    try {
      await navigator.clipboard.writeText(token.token)
      setSaved(true)
      toast.success(t('org.tokens.copied'))
    } catch {
      toast.error(t('org.invite.copyFailed'))
    }
  }

  return (
    <Dialog
      open={token !== null}
      onOpenChange={(open) => {
        if (!open && saved) onClose()
      }}>
      <DialogContent aria-describedby={undefined} className="sm:max-w-[480px]">
        <DialogTitle className="text-[15px] leading-5 font-medium">{t('org.tokens.issuedTitle')}</DialogTitle>
        <div data-ui="org.tokens.issued" className="grid gap-3">
          <p className="m-0 text-[13px] leading-5 text-[#3f3f3f]">{t('org.tokens.issuedOnce')}</p>
          <div className="flex items-center gap-2">
            <Input
              readOnly
              value={token?.token ?? ''}
              aria-label={t('org.tokens.secret')}
              className={cn(orgFieldClassName, 'font-mono')}
              onFocus={(event) => event.currentTarget.select()}
            />
            <button type="button" className={filledButtonClassName(36)} onClick={() => void copy()}>
              <Copy size={14} className="shrink-0 text-current" strokeWidth={1.7} aria-hidden />
              {t('org.tokens.copy')}
            </button>
          </div>
          <p className="m-0 text-[12px] leading-5 text-[#8a8a8a]">{t('org.tokens.cloneHint')}</p>
          <label className="flex items-center gap-2 text-[13px] text-[#171717]">
            <Checkbox checked={saved} onCheckedChange={(checked) => setSaved(checked === true)} />
            {t('org.tokens.savedConfirm')}
          </label>
          <div className="flex justify-end">
            <button type="button" disabled={!saved} className={filledButtonClassName(36)} onClick={onClose}>
              {t('org.tokens.done')}
            </button>
          </div>
        </div>
      </DialogContent>
    </Dialog>
  )
}
