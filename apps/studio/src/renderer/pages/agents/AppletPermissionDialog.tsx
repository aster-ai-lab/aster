import { Button } from '@a3s-lab/ui'
import { BookOpen, Cpu, User } from 'lucide-react'
import { useEffect, useRef, useState, type ReactNode } from 'react'
import { useTranslation } from 'react-i18next'

import { useRestingDialog } from '@renderer/components/restingDialog'
import { type AppletPermission, readAppletPermissions } from '@shared/appletRuntime'

import { allowPermissionGrant, entryPermissions, permissionGrantMatches } from './appletPermissionGrant'
import { loadAppletProfile } from './loadAppletProfile'
import { maskDisplayName } from './maskDisplayName'
import { runAppletWindowAction } from './workbenchAppletWindowBridge'

/**
 * The applet body stays unmounted until this install has been allowed.
 * Deny closes the window and does not remember a grant, so the next open asks again.
 */
export function AppletPermissionGate({
  appId,
  epoch = 0,
  name,
  onDeny,
  children
}: {
  appId: string
  epoch?: number
  name: string
  onDeny?: () => void
  children: ReactNode
}) {
  const [required, setRequired] = useState<AppletPermission[]>(() => entryPermissions(readAppletPermissions()))
  const [phase, setPhase] = useState<'ask' | 'allowed'>(() =>
    permissionGrantMatches(appId, entryPermissions(readAppletPermissions())) ? 'allowed' : 'ask'
  )
  useEffect(() => {
    const next = entryPermissions(readAppletPermissions())
    setRequired(next)
    setPhase(permissionGrantMatches(appId, next) ? 'allowed' : 'ask')
  }, [appId, epoch])
  const presence = useRestingDialog(phase === 'ask')
  if (!presence.present) return children
  return (
    <div className="relative z-0 min-h-0 flex-1 overflow-hidden">
      <AppletPermissionDialog
        state={presence.state}
        appId={appId}
        name={name}
        permissions={required}
        onAllow={() => {
          allowPermissionGrant(appId, required)
          setPhase('allowed')
        }}
        onDeny={() => {
          if (onDeny) {
            onDeny()
            return
          }
          void runAppletWindowAction('close')
        }}
      />
    </div>
  )
}

export function AppletPermissionDialog({
  state = 'open',
  appId,
  name,
  permissions,
  onAllow,
  onDeny
}: {
  state?: 'open' | 'closed'
  appId: string
  name: string
  permissions: readonly AppletPermission[]
  onAllow: () => void
  onDeny: () => void
}) {
  const { t } = useTranslation()
  const dialogRef = useRef<HTMLDivElement>(null)
  const wantsUser = permissions.includes('user')
  const [subject, setSubject] = useState<{ name: string; avatar: string } | null>(null)
  useEffect(() => {
    dialogRef.current?.focus()
  }, [])
  useEffect(() => {
    if (!wantsUser) return
    let current = true
    // The member sees which account this grant covers. The name is masked.
    // Profile drops the phone, and this card does not render it.
    void loadAppletProfile(appId).then((profile) => {
      if (!current || !profile) return
      const masked = maskDisplayName(profile.displayName)
      if (!masked && !profile.avatar) return
      setSubject({ name: masked, avatar: profile.avatar })
    })
    return () => {
      current = false
    }
  }, [appId, wantsUser])
  return (
    <div
      data-state={state}
      className={`a3s-dialog-scrim absolute inset-0 z-10 flex flex-col overflow-x-hidden overflow-y-auto overscroll-contain bg-black/20 p-4${state === 'closed' ? ' pointer-events-none' : ''}`}>
      <div
        ref={dialogRef}
        tabIndex={-1}
        role="dialog"
        aria-modal="true"
        aria-labelledby="applet-permission-title"
        aria-describedby="applet-permission-hint"
        data-ui="workbench.applet.permission"
        data-state={state}
        className={`a3s-dialog-surface m-auto w-full max-w-[360px] min-w-0 shrink-0 rounded-[12px] bg-white p-5 shadow-[0_12px_32px_rgba(0,0,0,0.12)] outline-none${state === 'closed' ? ' pointer-events-none' : ''}`}>
        <p className="m-0 text-[13px] leading-5 font-medium text-[#171717]">{name}</p>
        <h2 id="applet-permission-title" className="m-0 mt-1 text-[15px] leading-6 font-medium text-[#171717]">
          {t('workbench.applet.permission.title')}
        </h2>
        <ul
          aria-label={t('workbench.applet.permission.list')}
          className="m-0 mt-3 list-none divide-y divide-[#efefef] overflow-hidden rounded-[10px] border border-[#efefef] p-0">
          {permissions.map((permission) => {
            const preview = permission === 'user' ? subject : null
            return (
              <li
                key={permission}
                data-ui={`workbench.applet.permission.${permission}`}
                className="flex gap-3 bg-[#fafafa] px-3 py-2.5">
                <span
                  className={`grid size-8 shrink-0 place-items-center overflow-hidden bg-white text-[#171717] ${preview?.avatar ? 'rounded-full' : 'rounded-[8px]'}`}>
                  {preview?.avatar ? (
                    <img
                      data-ui="workbench.applet.permission.avatar"
                      src={preview.avatar}
                      alt=""
                      className="size-8 object-cover"
                    />
                  ) : (
                    <PermissionGlyph permission={permission} />
                  )}
                </span>
                <span className="min-w-0">
                  <span className="block text-[13px] leading-5 font-medium break-words text-[#171717]">
                    {t(`workbench.applet.permission.${permission}`)}
                  </span>
                  {preview?.name ? (
                    <span
                      data-ui="workbench.applet.permission.subject"
                      className="mt-0.5 block truncate text-[13px] leading-5 text-[#171717]">
                      {preview.name}
                    </span>
                  ) : null}
                  <span className="mt-0.5 block text-[12px] leading-4 break-words text-[#3f3f3f]">
                    {t(`workbench.applet.permission.${permission}.body`)}
                  </span>
                  <span className="mt-0.5 block text-[12px] leading-4 break-words text-[#8a8a8a]">
                    {t(`workbench.applet.permission.${permission}.limit`)}
                  </span>
                </span>
              </li>
            )
          })}
        </ul>
        <p id="applet-permission-hint" className="m-0 mt-3 text-[12px] leading-4 break-words text-[#8a8a8a]">
          {t('workbench.applet.permission.hint')}
        </p>
        <div className="mt-4 flex flex-wrap justify-end gap-2">
          <Button type="button" variant="outline" size="md" data-ui="workbench.applet.permission.deny" onClick={onDeny}>
            {t('workbench.applet.permission.deny')}
          </Button>
          <Button
            type="button"
            variant="emphasis"
            size="md"
            data-ui="workbench.applet.permission.allow"
            onClick={onAllow}>
            {t('workbench.applet.permission.allow')}
          </Button>
        </div>
      </div>
    </div>
  )
}

function PermissionGlyph({ permission }: { permission: AppletPermission }) {
  const Icon = permission === 'user' ? User : permission === 'model' ? Cpu : BookOpen
  return <Icon className="size-4" strokeWidth={1.7} aria-hidden />
}
