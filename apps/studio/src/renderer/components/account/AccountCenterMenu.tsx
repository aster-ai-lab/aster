import { Button, Dialog, DialogContent, DialogTitle, Input } from '@a3s-lab/ui'
import { CircleAlert, Clock, LogIn, LogOut, Pencil, Plus, RotateCw, UserPlus } from 'lucide-react'
import { useCallback, useEffect, useId, useLayoutEffect, useRef, useState, type FormEvent } from 'react'
import { createPortal } from 'react-dom'
import { useTranslation } from 'react-i18next'

import { DefaultAvatar } from '@renderer/components/DefaultAvatar'
import { useRestingDialog } from '@renderer/components/restingDialog'
import {
  createMemberOrg,
  isSessionDenied,
  joinCodeFromInput,
  joinMemberOrg,
  listMemberOrgs,
  logoutMemberAccount,
  PERSONAL_CENTER,
  readCenterId,
  readMemberAccount,
  readStoredCenter,
  reconcileCenter,
  requestJoinMemberOrg,
  returnToLogin,
  searchMemberOrgs,
  updateMemberAccount,
  writeCenterId,
  type DiscoverableOrg,
  type MemberAccount,
  type MemberOrg
} from '@renderer/data/memberOrgs'
import { openRoute } from '@renderer/services/mainWindowNavigation'
import { isEmoji } from '@renderer/utils/naming'
import { cn } from '@renderer/utils/style'

import { ProfileDialog, TeamLogoField } from './AccountDialogs'
import { textLogo } from './entityImage'
import { publishMemberProfile } from './memberProfile'

type Scope = 'orgs' | 'personal'
type LoadStatus = 'loading' | 'ready' | 'error' | 'denied'

const MENU_WIDTH = 248

type AccountCenterMenuProps = {
  open: boolean
  anchor: HTMLElement | null
  onOpenChange: (open: boolean) => void
  avatar?: string
  fallbackName?: string
  loadOrgs?: typeof listMemberOrgs
  createOrg?: typeof createMemberOrg
  joinOrg?: typeof joinMemberOrg
  searchOrgs?: typeof searchMemberOrgs
  requestJoin?: typeof requestJoinMemberOrg
  loadAccount?: typeof readMemberAccount
  logoutAccount?: typeof logoutMemberAccount
  updateAccount?: typeof updateMemberAccount
  onSignedOut?: () => void
  /** Leaving a team for the personal center lands on 工作; team-only pages vanish there. */
  openWork?: () => void
}

const openWorkPage = () => openRoute('/app/agents')

/** ActivityBar 头像弹出的团队 / 个人切换，对齐用户中心的切换中心。 */
export function AccountCenterMenu({
  open,
  anchor,
  onOpenChange,
  avatar,
  fallbackName = '',
  loadOrgs = listMemberOrgs,
  createOrg = createMemberOrg,
  joinOrg = joinMemberOrg,
  searchOrgs = searchMemberOrgs,
  requestJoin = requestJoinMemberOrg,
  loadAccount = readMemberAccount,
  logoutAccount = logoutMemberAccount,
  updateAccount = updateMemberAccount,
  onSignedOut = returnToLogin,
  openWork = openWorkPage
}: AccountCenterMenuProps) {
  const { t } = useTranslation()
  const menuRef = useRef<HTMLDivElement>(null)
  const request = useRef(0)
  const menuId = useId()
  const listId = useId()
  const [scope, setScope] = useState<Scope>(() => (readCenterId() === PERSONAL_CENTER ? 'personal' : 'orgs'))
  const [orgs, setOrgs] = useState<MemberOrg[]>([])
  const [status, setStatus] = useState<LoadStatus>('loading')
  const [centerId, setCenterId] = useState(readCenterId)
  const centerRef = useRef(centerId)
  centerRef.current = centerId
  const [coords, setCoords] = useState<{ top: number; left: number; width: number } | null>(null)
  const coordsHold = useRef(coords)
  const menuPresence = useRestingDialog(Boolean(open && anchor))
  const [createOpen, setCreateOpen] = useState(false)
  const [joinOpen, setJoinOpen] = useState(false)
  const [joinMode, setJoinMode] = useState<'code' | 'search'>('code')
  const [joinCode, setJoinCode] = useState('')
  const [joinError, setJoinError] = useState<string>()
  const [joinFormError, setJoinFormError] = useState<string>()
  const [joinBusy, setJoinBusy] = useState(false)
  const [teamQuery, setTeamQuery] = useState('')
  const [teamResults, setTeamResults] = useState<DiscoverableOrg[]>([])
  const [searchStatus, setSearchStatus] = useState<'idle' | 'loading' | 'ready' | 'error'>('idle')
  const [searchError, setSearchError] = useState<string>()
  const [applyError, setApplyError] = useState<string>()
  const [applyNotice, setApplyNotice] = useState<string>()
  const [applyTitle, setApplyTitle] = useState<string>()
  const [applyingId, setApplyingId] = useState<string>()
  const applyingGuard = useRef(false)
  const searchRequest = useRef(0)
  const tRef = useRef(t)
  tRef.current = t
  const [name, setName] = useState('')
  const [logo, setLogo] = useState('')
  const [profileOpen, setProfileOpen] = useState(false)
  const [nameError, setNameError] = useState<string>()
  const [formError, setFormError] = useState<string>()
  const [busy, setBusy] = useState(false)
  const [account, setAccount] = useState<MemberAccount>({ displayName: '', phone: '' })
  const [signingOut, setSigningOut] = useState(false)
  const [signOutError, setSignOutError] = useState<string>()
  const accountRequest = useRef(0)
  const shownName = account.displayName || fallbackName
  const shownAvatar = account.avatar || avatar

  const joined = orgs.filter((org) => org.membership !== 'pending')
  const current = joined.find((org) => org.id === centerId)
  const onPersonal = centerId === PERSONAL_CENTER || !current

  const load = useCallback(async () => {
    const id = ++request.current
    setStatus('loading')
    try {
      const rows = await loadOrgs()
      if (id !== request.current) return
      const next = reconcileCenter(centerRef.current, rows)
      if (next !== centerRef.current) writeCenterId(next)
      setOrgs(rows)
      setCenterId(next)
      setStatus('ready')
    } catch (error) {
      if (id !== request.current) return
      setStatus(isSessionDenied(error) ? 'denied' : 'error')
    }
  }, [loadOrgs])

  useEffect(() => {
    if (readStoredCenter()) return
    let alive = true
    void loadOrgs()
      .then((rows) => {
        if (!alive || readStoredCenter()) return
        const joined = rows.filter((org) => org.membership !== 'pending')
        if (joined.length === 1) writeCenterId(joined[0].id)
      })
      .catch(() => undefined)
    return () => {
      alive = false
    }
  }, [loadOrgs])

  useLayoutEffect(() => {
    if (!open) return
    setScope(readCenterId() === PERSONAL_CENTER ? 'personal' : 'orgs')
    setSignOutError(undefined)
    void load()
    const id = ++accountRequest.current
    void loadAccount().then(
      (next) => {
        if (id !== accountRequest.current) return
        setAccount(next)
        publishMemberProfile(next)
      },
      (error: unknown) => {
        if (id !== accountRequest.current) return
        setAccount({ displayName: '', phone: '' })
        if (isSessionDenied(error)) setStatus('denied')
      }
    )
  }, [load, loadAccount, open])

  useLayoutEffect(() => {
    if (!open || !anchor) {
      setCoords(null)
      return
    }
    const place = () => {
      const menu = menuRef.current
      const rect = anchor.getBoundingClientRect()
      const height = menu?.offsetHeight || 280
      let left = rect.right + 8
      if (left + MENU_WIDTH > window.innerWidth - 8) left = Math.max(8, rect.left - MENU_WIDTH - 8)
      let top = rect.top
      if (top + height > window.innerHeight - 8) top = Math.max(8, window.innerHeight - height - 8)
      setCoords((previous) =>
        previous && previous.top === top && previous.left === left && previous.width === MENU_WIDTH
          ? previous
          : { top, left, width: MENU_WIDTH }
      )
    }
    place()
    window.addEventListener('resize', place)
    return () => window.removeEventListener('resize', place)
  }, [account.displayName, account.phone, anchor, open, orgs, scope, signOutError, status])

  useEffect(() => {
    if (!open) return
    const onPointerDown = (event: PointerEvent) => {
      const target = event.target
      if (!(target instanceof Node)) return
      if (menuRef.current?.contains(target) || anchor?.contains(target)) return
      onOpenChange(false)
    }
    const onKey = (event: KeyboardEvent) => {
      if (event.key === 'Escape') onOpenChange(false)
    }
    document.addEventListener('pointerdown', onPointerDown)
    document.addEventListener('keydown', onKey)
    return () => {
      document.removeEventListener('pointerdown', onPointerDown)
      document.removeEventListener('keydown', onKey)
    }
  }, [anchor, onOpenChange, open])

  function choose(next: string) {
    const leavingTeam = centerRef.current !== PERSONAL_CENTER && next === PERSONAL_CENTER
    request.current += 1
    setCenterId(next)
    writeCenterId(next)
    onOpenChange(false)
    if (leavingTeam) openWork()
  }

  async function signOut() {
    setSigningOut(true)
    setSignOutError(undefined)
    try {
      await logoutAccount()
      onOpenChange(false)
      onSignedOut()
    } catch {
      setSignOutError(t('sidebar.center.logout_error'))
      setSigningOut(false)
    }
  }

  function startCreate() {
    onOpenChange(false)
    setName('')
    setLogo('')
    setNameError(undefined)
    setFormError(undefined)
    setCreateOpen(true)
  }

  useEffect(() => {
    if (!joinOpen || joinMode !== 'search') return
    const trimmed = teamQuery.trim()
    if (!trimmed) {
      setTeamResults([])
      setSearchStatus('idle')
      setSearchError(undefined)
      return
    }
    const id = ++searchRequest.current
    setSearchError(undefined)
    const timer = window.setTimeout(() => {
      setSearchStatus('loading')
      void searchOrgs(trimmed)
        .then((rows) => {
          if (id !== searchRequest.current) return
          setTeamResults(rows)
          setSearchStatus('ready')
        })
        .catch((error: unknown) => {
          if (id !== searchRequest.current) return
          setTeamResults([])
          setSearchStatus('error')
          const translate = tRef.current
          setSearchError(
            isSessionDenied(error)
              ? translate('sidebar.center.session_expired')
              : translate('sidebar.center.join_search_error')
          )
        })
    }, 300)
    return () => window.clearTimeout(timer)
  }, [joinOpen, joinMode, teamQuery, searchOrgs])

  function startJoin() {
    onOpenChange(false)
    resetJoin()
    setJoinOpen(true)
  }

  function resetJoin() {
    searchRequest.current += 1
    setJoinMode('code')
    setJoinCode('')
    setJoinError(undefined)
    setJoinFormError(undefined)
    setJoinBusy(false)
    setTeamQuery('')
    setTeamResults([])
    setSearchStatus('idle')
    setSearchError(undefined)
    setApplyError(undefined)
    setApplyNotice(undefined)
    setApplyTitle(undefined)
    setApplyingId(undefined)
    applyingGuard.current = false
  }

  async function onJoin(event: FormEvent<HTMLFormElement>) {
    event.preventDefault()
    const trimmed = joinCode.trim()
    if (!trimmed) {
      setJoinError(t('sidebar.center.join_required'))
      return
    }
    const code = joinCodeFromInput(trimmed)
    if (!code) {
      setJoinError(t('sidebar.center.join_invalid'))
      return
    }
    setJoinBusy(true)
    setJoinFormError(undefined)
    try {
      const joinedOrg = await joinOrg(code)
      setJoinOpen(false)
      resetJoin()
      choose(joinedOrg.id)
    } catch (err) {
      const message = err instanceof Error ? err.message : ''
      if (isSessionDenied(err)) setJoinFormError(t('sidebar.center.session_expired'))
      else if (/not found/i.test(message)) setJoinError(t('sidebar.center.join_not_found'))
      else if (/disabled or expired/i.test(message)) setJoinError(t('sidebar.center.join_disabled'))
      else if (/already a member/i.test(message)) setJoinFormError(t('sidebar.center.already_member'))
      else if (message === 'every seat is taken; add seats or disable a member first')
        setJoinFormError(t('sidebar.center.join_seats_full'))
      else setJoinFormError(t('sidebar.center.join_error'))
      setJoinBusy(false)
    }
  }

  async function onApply(org: DiscoverableOrg) {
    if (org.joined || org.requested || applyingGuard.current) return
    applyingGuard.current = true
    setApplyingId(org.id)
    setApplyError(undefined)
    setApplyNotice(undefined)
    setApplyTitle(undefined)
    try {
      await requestJoin(org.id)
      setTeamResults((rows) => rows.map((row) => (row.id === org.id ? { ...row, requested: true } : row)))
      setApplyTitle(t('sidebar.center.join_apply_done_title'))
      setApplyNotice(t('sidebar.center.join_apply_done'))
      void load()
    } catch (err) {
      const message = err instanceof Error ? err.message : ''
      if (isSessionDenied(err)) setApplyError(t('sidebar.center.session_expired'))
      else if (/already a member/i.test(message)) setApplyError(t('sidebar.center.already_member'))
      else if (/already pending/i.test(message)) {
        setTeamResults((rows) => rows.map((row) => (row.id === org.id ? { ...row, requested: true } : row)))
        setApplyTitle(t('sidebar.center.join_apply_pending_title'))
        setApplyNotice(t('sidebar.center.join_apply_pending'))
      } else if (/not found/i.test(message)) setApplyError(t('sidebar.center.join_not_found'))
      else setApplyError(t('sidebar.center.join_apply_error'))
    } finally {
      applyingGuard.current = false
      setApplyingId(undefined)
    }
  }

  async function onCreate(event: FormEvent<HTMLFormElement>) {
    event.preventDefault()
    const field = event.currentTarget.querySelector('#account-center-team-name')
    const shown = field instanceof HTMLInputElement ? field.value : name
    const trimmed = shown.trim()
    if (!trimmed) {
      setNameError(t('sidebar.center.name_required'))
      return
    }
    setBusy(true)
    setFormError(undefined)
    try {
      const created = await createOrg(trimmed, logo || textLogo(trimmed))
      setCreateOpen(false)
      setName('')
      setLogo('')
      setBusy(false)
      choose(created.id)
    } catch (err) {
      const message = err instanceof Error ? err.message : ''
      if (isSessionDenied(err)) setFormError(t('sidebar.center.session_expired'))
      else if (/already exists/i.test(message)) setNameError(t('sidebar.center.name_taken'))
      else setFormError(t('sidebar.center.create_error'))
      setBusy(false)
    }
  }

  if (coords) coordsHold.current = coords
  const placed = coords ?? coordsHold.current
  const menuShown = Boolean(open && anchor) || (menuPresence.present && placed !== null)
  const menu = menuShown
    ? createPortal(
        <div
          ref={menuRef}
          id={menuId}
          role="dialog"
          aria-label={t('sidebar.center.switch')}
          data-state={menuPresence.state}
          className={`fixed z-[80] flex flex-col overflow-hidden rounded-[14px] border border-[#ececec] bg-white text-[#171717] shadow-[0_8px_22px_rgba(20,40,80,0.08)] dark:border-[#2e2e2e] dark:bg-[#242424] dark:text-[#f5f5f5] ${coords || menuPresence.state === 'closed' ? 'a3s-popup-rise' : ''} ${menuPresence.state === 'closed' ? 'pointer-events-none' : ''}`}
          style={{
            top: (coords ?? placed)?.top ?? 0,
            left: (coords ?? placed)?.left ?? 0,
            width: MENU_WIDTH,
            maxHeight: 'min(22rem, calc(100vh - 1rem))',
            padding: '8px',
            visibility: coords || menuPresence.state === 'closed' ? 'visible' : 'hidden'
          }}>
          <button
            type="button"
            data-ui="account.edit-profile"
            className="group mb-1.5 flex w-full items-center gap-2.5 rounded-[10px] px-2.5 py-1.5 text-start outline-none hover:bg-[#f7f7f7] focus-visible:ring-2 focus-visible:ring-black/10 focus-visible:ring-inset disabled:pointer-events-none dark:hover:bg-[#2c2c2c]"
            aria-label={t('sidebar.center.edit_profile')}
            title={t('sidebar.center.edit_profile')}
            disabled={status === 'denied'}
            onClick={() => {
              onOpenChange(false)
              setProfileOpen(true)
            }}>
            <AccountPortrait name={shownName} avatar={shownAvatar} />
            <div className="min-w-0 flex-1">
              <p className="m-0 truncate text-[14px] leading-5 font-medium text-[#1a1a1a] dark:text-[#f5f5f5]">
                {shownName}
              </p>
              {account.phone ? (
                <p className="m-0 truncate text-[12px] leading-5 text-[#8b8b8b] dark:text-[#9a9a9a]">
                  {t('sidebar.center.phone')} {account.phone}
                </p>
              ) : null}
            </div>
            <Pencil
              className="size-3.5 shrink-0 text-[#a3a3a3] opacity-0 transition-opacity group-hover:opacity-100 group-focus-visible:opacity-100"
              strokeWidth={1.7}
              aria-hidden
            />
          </button>
          <p className="m-0 px-2.5 pb-1.5 text-[13px] leading-5 text-[#8a8a8a] dark:text-[#9a9a9a]">
            {t('sidebar.center.switch')}
          </p>
          <div
            className="mb-1.5 flex items-center rounded-[8px] bg-[#f5f5f5] p-1 dark:bg-[#2c2c2c]"
            role="tablist"
            aria-label={t('sidebar.center.switch')}>
            <ScopeTab
              selected={scope === 'orgs'}
              controls={listId}
              label={t('sidebar.center.teams')}
              onSelect={() => setScope('orgs')}
            />
            <ScopeTab
              selected={scope === 'personal'}
              controls={listId}
              label={t('sidebar.center.personal')}
              onSelect={() => setScope('personal')}
            />
          </div>
          <div id={listId} className="grid content-start gap-0.5 overflow-y-auto pb-1" role="menu">
            {scope === 'personal' ? (
              <CenterRow
                label={t('sidebar.center.personal_label')}
                personal
                selected={onPersonal}
                onSelect={() => choose(PERSONAL_CENTER)}
              />
            ) : status === 'loading' ? (
              <p className="m-0 px-2.5 py-2 text-[13px] text-[#8a8a8a]">{t('common.loading')}</p>
            ) : status === 'denied' ? (
              <div className="grid gap-1.5 px-2.5 py-2" role="alert">
                <p className="m-0 text-[13px] text-[#8a8a8a]">{t('sidebar.center.session_expired')}</p>
                <button
                  type="button"
                  className="inline-flex w-fit items-center gap-1.5 text-[13px] font-medium text-[#171717] outline-none hover:underline focus-visible:underline dark:text-[#f5f5f5]"
                  disabled={signingOut}
                  onClick={() => void signOut()}>
                  <LogIn className="size-3.5 text-current" strokeWidth={1.7} aria-hidden />
                  {t('sidebar.center.sign_in_again')}
                </button>
              </div>
            ) : status === 'error' ? (
              <div className="flex items-center justify-between gap-2 px-2.5 py-2">
                <p className="m-0 text-[13px] text-[#8a8a8a]">{t('sidebar.center.error')}</p>
                <button
                  type="button"
                  className="inline-flex items-center gap-1 text-[13px] text-[#3f3f3f] outline-none hover:text-[#171717] focus-visible:underline dark:text-[#d0d0d0]"
                  onClick={() => void load()}>
                  <RotateCw className="size-3.5 text-current" strokeWidth={1.7} aria-hidden />
                  {t('common.retry')}
                </button>
              </div>
            ) : joined.length === 0 ? (
              <p className="m-0 px-2.5 py-2 text-[13px] text-[#8a8a8a]">{t('sidebar.center.empty')}</p>
            ) : (
              joined.map((org) => (
                <CenterRow
                  key={org.id}
                  label={org.name}
                  logo={org.logo}
                  role={roleLabel(org, t)}
                  selected={org.id === centerId}
                  onSelect={() => choose(org.id)}
                />
              ))
            )}
          </div>
          <button
            type="button"
            className="mt-1 flex h-9 w-full items-center gap-2.5 rounded-[8px] px-2.5 text-start text-[13px] text-[#3f3f3f] outline-none hover:bg-[#f7f7f7] hover:text-[#171717] focus-visible:ring-2 focus-visible:ring-black/10 focus-visible:ring-inset disabled:pointer-events-none disabled:opacity-50 dark:text-[#d0d0d0] dark:hover:bg-[#2c2c2c] dark:hover:text-[#f5f5f5]"
            disabled={status === 'denied'}
            onClick={scope === 'personal' ? startJoin : startCreate}>
            {scope === 'personal' ? (
              <UserPlus className="size-4 shrink-0 text-current" strokeWidth={1.7} aria-hidden />
            ) : (
              <Plus className="size-4 shrink-0 text-current" strokeWidth={1.7} aria-hidden />
            )}
            {t(scope === 'personal' ? 'sidebar.center.join' : 'sidebar.center.create')}
          </button>
          {signOutError ? (
            <p className="m-0 px-2.5 pt-1 text-[12px] text-destructive" role="alert">
              {signOutError}
            </p>
          ) : null}
          <div className="mx-2.5 my-1 h-px bg-[#ebebeb] dark:bg-[#2e2e2e]" />
          <button
            type="button"
            className="flex h-9 w-full items-center gap-2.5 rounded-[8px] px-2.5 text-start text-[13px] text-[#3f3f3f] outline-none hover:bg-[#f7f7f7] hover:text-[#171717] focus-visible:ring-2 focus-visible:ring-black/10 focus-visible:ring-inset disabled:opacity-60 dark:text-[#d0d0d0] dark:hover:bg-[#2c2c2c] dark:hover:text-[#f5f5f5]"
            disabled={signingOut}
            onClick={() => void signOut()}>
            <LogOut className="size-4 shrink-0 text-current" strokeWidth={1.7} aria-hidden />
            {t('sidebar.center.logout')}
          </button>
        </div>,
        document.body
      )
    : null

  return (
    <>
      {menu}
      <Dialog
        open={createOpen}
        onOpenChange={(next) => {
          setCreateOpen(next)
          if (!next) {
            setName('')
            setLogo('')
            setNameError(undefined)
            setFormError(undefined)
            setBusy(false)
          }
        }}>
        <DialogContent className="sm:max-w-sm" aria-describedby={undefined}>
          <DialogTitle>{t('sidebar.center.create_title')}</DialogTitle>
          <form className="grid gap-3" onSubmit={(event) => void onCreate(event)}>
            {formError ? (
              <p className="m-0 text-sm text-destructive" role="alert">
                {formError}
              </p>
            ) : null}
            <label className="grid gap-1.5 text-sm" htmlFor="account-center-team-name">
              {t('sidebar.center.name')}
              <Input
                id="account-center-team-name"
                name="team-name"
                value={name}
                maxLength={64}
                disabled={busy}
                autoFocus
                placeholder={t('sidebar.center.name_placeholder')}
                aria-invalid={nameError ? true : undefined}
                onChange={(event) => {
                  setName(event.target.value)
                  setNameError(undefined)
                }}
              />
            </label>
            {nameError ? (
              <p className="m-0 text-sm text-destructive" role="alert">
                {nameError}
              </p>
            ) : null}
            <TeamLogoField
              name={name}
              logo={logo}
              disabled={busy}
              onChange={(next) => {
                setLogo(next)
                setFormError(undefined)
              }}
              onError={setFormError}
            />
            <div className="flex justify-end gap-2">
              <Button type="button" size="sm" variant="ghost" disabled={busy} onClick={() => setCreateOpen(false)}>
                {t('common.cancel')}
              </Button>
              <Button type="submit" size="sm" disabled={busy}>
                {t('sidebar.center.create_submit')}
              </Button>
            </div>
          </form>
        </DialogContent>
      </Dialog>
      <Dialog
        open={joinOpen}
        onOpenChange={(next) => {
          setJoinOpen(next)
          if (!next) resetJoin()
        }}>
        <DialogContent
          className="!animate-none !opacity-100 sm:max-w-md"
          overlayClassName="!opacity-100"
          aria-describedby={undefined}>
          <DialogTitle>{t('sidebar.center.join_title')}</DialogTitle>
          <div
            className="flex items-center rounded-[8px] bg-[#f5f5f5] p-1 dark:bg-[#2c2c2c]"
            role="tablist"
            aria-label={t('sidebar.center.join_title')}>
            <ScopeTab
              selected={joinMode === 'code'}
              controls="account-center-join-code-panel"
              label={t('sidebar.center.join_code')}
              onSelect={() => setJoinMode('code')}
            />
            <ScopeTab
              selected={joinMode === 'search'}
              controls="account-center-join-search-panel"
              label={t('sidebar.center.join_search_tab')}
              onSelect={() => setJoinMode('search')}
            />
          </div>
          {joinMode === 'code' ? (
            <form
              id="account-center-join-code-panel"
              role="tabpanel"
              className="grid gap-3"
              onSubmit={(event) => void onJoin(event)}>
              {joinFormError ? (
                <p className="m-0 text-sm text-destructive" role="alert">
                  {joinFormError}
                </p>
              ) : null}
              <label className="grid gap-1.5 text-sm" htmlFor="account-center-join-code">
                {t('sidebar.center.join_code')}
                <Input
                  id="account-center-join-code"
                  name="join-code"
                  value={joinCode}
                  maxLength={512}
                  disabled={joinBusy}
                  autoFocus
                  placeholder={t('sidebar.center.join_placeholder')}
                  aria-invalid={joinError ? true : undefined}
                  onChange={(event) => {
                    setJoinCode(event.target.value)
                    setJoinError(undefined)
                  }}
                />
              </label>
              {joinError ? (
                <p className="m-0 text-sm text-destructive" role="alert">
                  {joinError}
                </p>
              ) : null}
              <div className="flex justify-end gap-2">
                <Button type="button" size="sm" variant="ghost" disabled={joinBusy} onClick={() => setJoinOpen(false)}>
                  {t('common.cancel')}
                </Button>
                <Button type="submit" size="sm" disabled={joinBusy}>
                  {t('sidebar.center.join_submit')}
                </Button>
              </div>
            </form>
          ) : (
            <div id="account-center-join-search-panel" role="tabpanel" className="grid gap-3">
              {applyNotice ? (
                <div
                  role="status"
                  className="flex items-start gap-2 rounded-[8px] bg-[#f5f5f5] px-2.5 py-2 dark:bg-[#2c2c2c]">
                  <Clock className="mt-0.5 size-4 shrink-0 text-[#6f6f6f]" strokeWidth={1.7} aria-hidden />
                  <div className="min-w-0">
                    <p className="m-0 text-[13px] leading-5 font-medium text-[#171717] dark:text-[#f5f5f5]">
                      {applyTitle}
                    </p>
                    <p className="m-0 text-[12px] leading-5 text-[#8a8a8a]">{applyNotice}</p>
                  </div>
                </div>
              ) : null}
              {applyError ? (
                <div
                  role="alert"
                  className="flex items-start gap-2 rounded-[8px] bg-[#f5f5f5] px-2.5 py-2 dark:bg-[#2c2c2c]">
                  <CircleAlert className="mt-0.5 size-4 shrink-0 text-[#b42318]" strokeWidth={1.7} aria-hidden />
                  <p className="m-0 text-[13px] leading-5 text-[#b42318]">{applyError}</p>
                </div>
              ) : null}
              <label className="grid gap-1.5 text-sm" htmlFor="account-center-join-search">
                {t('sidebar.center.join_search')}
                <Input
                  id="account-center-join-search"
                  name="team-search"
                  value={teamQuery}
                  maxLength={64}
                  autoFocus
                  placeholder={t('sidebar.center.join_search_placeholder')}
                  onChange={(event) => {
                    setTeamQuery(event.target.value)
                    setApplyError(undefined)
                    setApplyNotice(undefined)
                    setApplyTitle(undefined)
                  }}
                />
              </label>
              <div className="grid max-h-60 content-start gap-0.5 overflow-y-auto">
                {searchStatus === 'idle' ? (
                  <p className="m-0 px-1 py-2 text-[13px] text-[#8a8a8a]">{t('sidebar.center.join_search_hint')}</p>
                ) : searchStatus === 'loading' ? (
                  <p className="m-0 px-1 py-2 text-[13px] text-[#8a8a8a]">{t('common.loading')}</p>
                ) : searchError ? (
                  <p className="m-0 px-1 py-2 text-sm text-destructive" role="alert">
                    {searchError}
                  </p>
                ) : teamResults.length === 0 ? (
                  <p className="m-0 px-1 py-2 text-[13px] text-[#8a8a8a]">{t('sidebar.center.join_search_empty')}</p>
                ) : (
                  teamResults.map((org) => (
                    <div key={org.id} className="flex h-10 items-center gap-2.5 rounded-[8px] px-1">
                      <OrgMark name={org.name} logo={org.logo} />
                      <span className="min-w-0 flex-1 truncate text-[13px] text-[#171717] dark:text-[#f5f5f5]">
                        {org.name}
                      </span>
                      <span className="shrink-0 text-[12px] text-[#8a8a8a]">
                        {t('sidebar.center.join_members', { count: org.memberCount })}
                      </span>
                      {org.joined ? (
                        <span className="shrink-0 text-[12px] text-[#8a8a8a]">{t('sidebar.center.join_joined')}</span>
                      ) : org.requested ? (
                        <span className="shrink-0 text-[12px] text-[#8a8a8a]">{t('sidebar.center.join_applied')}</span>
                      ) : (
                        <button
                          type="button"
                          disabled={applyingId === org.id}
                          aria-label={`${t('sidebar.center.join_apply')} ${org.name}`}
                          className="inline-flex h-6 shrink-0 items-center rounded-[8px] border border-[#e5e5e5] bg-white px-2 text-[12px] leading-none text-[#171717] outline-none hover:bg-[#fafafa] focus-visible:ring-2 focus-visible:ring-black/10 disabled:opacity-40 dark:border-[#2f2f33] dark:bg-[#171717] dark:text-[#f5f5f5] dark:hover:bg-[#232326]"
                          onClick={() => void onApply(org)}>
                          {t('sidebar.center.join_apply')}
                        </button>
                      )}
                    </div>
                  ))
                )}
              </div>
              <div className="flex justify-end">
                <Button type="button" size="sm" variant="ghost" onClick={() => setJoinOpen(false)}>
                  {t('common.cancel')}
                </Button>
              </div>
            </div>
          )}
        </DialogContent>
      </Dialog>
      <ProfileDialog
        open={profileOpen}
        account={{ ...account, displayName: shownName, avatar: account.avatar }}
        onOpenChange={setProfileOpen}
        save={updateAccount}
        onSaved={(next) => {
          setAccount(next)
          publishMemberProfile(next)
        }}
        onSessionDenied={() => setStatus('denied')}
      />
    </>
  )
}

function AccountPortrait({ name, avatar }: { name: string; avatar?: string }) {
  const label = name || ' '
  if (!avatar) return <DefaultAvatar label={label} className="size-12 rounded-[8px]" />
  if (!isEmoji(avatar)) {
    return <img src={avatar} alt={label} className="size-12 shrink-0 rounded-[8px] bg-[#d6d6d6] object-cover" />
  }
  return (
    <span
      className="inline-flex size-12 shrink-0 items-center justify-center rounded-[8px] bg-[#f2f4f7] text-xl"
      role="img"
      aria-label={label}>
      {avatar}
    </span>
  )
}

function roleLabel(org: MemberOrg, t: (key: string) => string) {
  if (org.creator) return t('sidebar.center.creator')
  if (org.membership === 'admin') return t('sidebar.center.admin')
  return t('sidebar.center.member')
}

function ScopeTab({
  selected,
  controls,
  label,
  onSelect
}: {
  selected: boolean
  controls: string
  label: string
  onSelect: () => void
}) {
  return (
    <button
      type="button"
      role="tab"
      aria-selected={selected}
      aria-controls={controls}
      className={cn(
        'inline-flex h-[22px] flex-1 items-center justify-center rounded-[6px] px-3 text-[13px] font-medium whitespace-nowrap outline-none',
        'focus-visible:ring-2 focus-visible:ring-black/10 focus-visible:ring-inset',
        selected
          ? 'bg-white text-[#171717] shadow-[0_1px_2px_rgba(0,0,0,0.06)] dark:bg-[#3a3a3a] dark:text-[#f5f5f5]'
          : 'text-[#6f6f6f] hover:text-[#171717] dark:text-[#9a9a9a] dark:hover:text-[#f5f5f5]'
      )}
      onClick={onSelect}>
      {label}
    </button>
  )
}

function CenterRow({
  label,
  logo = '',
  personal = false,
  role,
  selected,
  onSelect
}: {
  label: string
  logo?: string
  personal?: boolean
  role?: string
  selected: boolean
  onSelect: () => void
}) {
  return (
    <button
      type="button"
      role="menuitemradio"
      aria-checked={selected}
      className={cn(
        'flex h-9 w-full min-w-0 items-center gap-2.5 rounded-[8px] px-2.5 text-start text-[13px] transition-colors outline-none',
        'focus-visible:ring-2 focus-visible:ring-black/10 focus-visible:ring-inset',
        selected
          ? 'bg-[#f2f3f5] font-medium text-[#171717] dark:bg-[#3a3a3a] dark:text-[#f5f5f5]'
          : 'text-[#3f3f3f] hover:bg-[#f7f7f7] hover:text-[#171717] dark:text-[#d0d0d0] dark:hover:bg-[#2c2c2c] dark:hover:text-[#f5f5f5]'
      )}
      onClick={onSelect}>
      {personal ? (
        <span className="inline-flex size-4 shrink-0 items-center justify-center text-current">
          <PersonalGlyph />
        </span>
      ) : (
        <OrgMark name={label} logo={logo} />
      )}
      <span className="min-w-0 flex-1 truncate">{label}</span>
      {role ? <span className="shrink-0 text-[12px] leading-5 text-[#8a8a8a] dark:text-[#9a9a9a]">{role}</span> : null}
    </button>
  )
}

function OrgMark({ name, logo }: { name: string; logo: string }) {
  if (logo) return <img className="size-4 shrink-0 rounded-[4px] object-cover" src={logo} alt="" />
  return (
    <span
      className="inline-flex size-4 shrink-0 items-center justify-center rounded-[4px] bg-[#f2f3f5] text-[8px] font-medium text-[#171717] dark:bg-[#3a3a3a] dark:text-[#f5f5f5]"
      aria-hidden>
      {leadingChars(name, 1)}
    </span>
  )
}

function leadingChars(name: string, count: number) {
  const trimmed = name.trim()
  if (!trimmed) return ''
  if (typeof Intl !== 'undefined' && 'Segmenter' in Intl) {
    const segmenter = new Intl.Segmenter('zh', { granularity: 'grapheme' })
    return [...segmenter.segment(trimmed)]
      .slice(0, count)
      .map((part) => part.segment)
      .join('')
  }
  return [...trimmed].slice(0, count).join('')
}

function PersonalGlyph() {
  return (
    <svg viewBox="0 0 24 24" className="size-4" fill="none" aria-hidden>
      <circle cx="12" cy="8" r="3.25" stroke="currentColor" strokeWidth="1.7" />
      <path d="M5.5 19a6.5 6.5 0 0 1 13 0" stroke="currentColor" strokeWidth="1.7" strokeLinecap="round" />
    </svg>
  )
}
