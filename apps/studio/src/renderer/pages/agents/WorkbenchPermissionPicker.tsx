import { DropdownMenu, DropdownMenuContent, DropdownMenuItem, DropdownMenuTrigger } from '@a3s-lab/ui'
import { Check, ChevronDown, Hand, Route, ShieldAlert, ShieldCheck } from 'lucide-react'
import { useLayoutEffect, useRef, useState } from 'react'
import { useTranslation } from 'react-i18next'

import { cn } from '@renderer/utils/style'

import {
  A3S_CODE_POSTURES,
  type A3sCodePosture,
  isRiskyPosture,
  permissionModeForPosture,
  postureForPermissionMode
} from './workbenchPermissionPostures'

const POSTURE_ICONS: Record<A3sCodePosture, typeof Hand> = {
  default: Hand,
  plan: Route,
  auto: ShieldCheck,
  'always-approve': ShieldAlert
}

const POSTURE_KEYS: Record<A3sCodePosture, string> = {
  default: 'default',
  plan: 'plan',
  auto: 'auto',
  'always-approve': 'always_approve'
}

// The choice press is delivered again to a recreated chip, whose pointer timer is still zero.
let suppressTriggerOpenUntil = 0

export function resetPermissionMenuOpenBlock() {
  suppressTriggerOpenUntil = 0
}

function triggerOpenSuppressed() {
  return performance.now() < suppressTriggerOpenUntil
}

type Props = {
  permissionMode?: string
  onPermissionModeChange: (mode: string) => void
}

export function WorkbenchPermissionPicker({ permissionMode, onPermissionModeChange }: Props) {
  const { t } = useTranslation()
  const [open, setOpen] = useState(false)
  const closedByPointer = useRef(false)
  const pointerDownAt = useRef(0)
  const current = postureForPermissionMode(permissionMode)
  const CurrentIcon = POSTURE_ICONS[current]
  const title = (posture: A3sCodePosture) => t(`workbench.permission.${POSTURE_KEYS[posture]}.title`)
  const heading = t('workbench.permission.title')
  const hint = t('workbench.permission.next_task_hint')

  const choose = (posture: A3sCodePosture) => {
    suppressTriggerOpenUntil = performance.now() + 500
    setOpen(false)
    onPermissionModeChange(permissionModeForPosture(posture))
  }
  const setMenuOpen = (next: boolean) => {
    if (next && triggerOpenSuppressed()) {
      setOpen(false)
      return
    }
    setOpen(next)
  }
  const menuOpen = open && !triggerOpenSuppressed()
  useLayoutEffect(() => {
    if (!triggerOpenSuppressed()) return
    if (open) setOpen(false)
  }, [open])

  return (
    <DropdownMenu open={menuOpen} onOpenChange={setMenuOpen}>
      <DropdownMenuTrigger asChild>
        <button
          type="button"
          data-workbench-chip="authorize"
          data-posture={current}
          aria-label={`${heading}: ${title(current)}`}
          onPointerDown={() => {
            pointerDownAt.current = performance.now()
          }}
          onClick={() => {
            // AXPress clicks without the pointerdown Radix uses to open.
            if (triggerOpenSuppressed()) return
            if (pointerDownAt.current === 0 || performance.now() - pointerDownAt.current > 500) setMenuOpen(true)
          }}
          className={cn(
            'flex h-8 items-center gap-1.5 rounded-[8px] px-2 text-[13px] text-[#3f3f3f] outline-none hover:bg-[#f7f7f7] hover:text-[#171717] focus-visible:ring-2 focus-visible:ring-black/10 data-[state=open]:bg-[#f5f5f5] data-[state=open]:text-[#171717]',
            isRiskyPosture(current) && 'text-[#b42318] hover:text-[#b42318] data-[state=open]:text-[#b42318]'
          )}>
          <CurrentIcon size={16} strokeWidth={1.7} aria-hidden="true" />
          <span className="max-w-28 truncate">{title(current)}</span>
          <ChevronDown size={14} strokeWidth={1.7} aria-hidden="true" className="text-[#8a8a8a]" />
        </button>
      </DropdownMenuTrigger>
      <DropdownMenuContent
        side="top"
        align="start"
        sideOffset={8}
        aria-label={`${heading} ${hint}`}
        data-workbench-menu="authorize"
        onPointerDown={() => {
          closedByPointer.current = true
        }}
        onKeyDown={() => {
          closedByPointer.current = false
        }}
        onPointerDownOutside={() => {
          closedByPointer.current = true
        }}
        onCloseAutoFocus={(event) => {
          // Returning focus after a click paints the keyboard focus ring on the trigger.
          if (closedByPointer.current) event.preventDefault()
          closedByPointer.current = false
        }}
        className="z-[80] w-[320px] rounded-[12px] border border-[#ececec] bg-white p-1.5 text-[#171717] shadow-[0_12px_32px_rgba(0,0,0,0.12)]">
        <div className="px-2.5 pt-1.5 pb-1 text-[12px] text-[#8a8a8a]">{heading}</div>
        {A3S_CODE_POSTURES.map((posture) => {
          const Icon = POSTURE_ICONS[posture]
          const selected = posture === current
          const risky = isRiskyPosture(posture)
          return (
            <DropdownMenuItem
              key={posture}
              role="menuitemradio"
              aria-checked={selected}
              data-posture={posture}
              onSelect={() => choose(posture)}
              onClick={() => choose(posture)}
              className={cn(
                'flex cursor-default items-start gap-2.5 rounded-[8px] px-2.5 py-2 text-[#3f3f3f] outline-none focus:bg-[#f7f7f7] data-[highlighted]:bg-[#f7f7f7]',
                selected && 'bg-[#f2f3f5] text-[#171717]'
              )}>
              <Icon
                size={16}
                strokeWidth={1.7}
                aria-hidden="true"
                className={cn('mt-0.5 shrink-0 text-current', risky && 'text-[#b42318]')}
              />
              <span className="flex min-w-0 flex-1 flex-col gap-0.5">
                <span
                  className={cn(
                    'flex items-center gap-1.5 text-[13px] leading-[1.4]',
                    selected && 'font-medium',
                    risky && 'text-[#b42318]'
                  )}>
                  {title(posture)}
                  {risky && (
                    <span className="text-[12px] font-normal text-[#b42318]">{t('workbench.permission.risk')}</span>
                  )}
                </span>
                <span
                  data-posture-description={posture}
                  className={cn('text-[12px] leading-[1.45] text-[#8a8a8a]', risky && 'text-[#b42318]')}>
                  {t(`workbench.permission.${POSTURE_KEYS[posture]}.description`)}
                </span>
              </span>
              <Check
                size={16}
                strokeWidth={1.7}
                aria-hidden="true"
                className={cn('mt-0.5 shrink-0 text-[#171717]', selected ? 'opacity-100' : 'opacity-0')}
              />
            </DropdownMenuItem>
          )
        })}
        <div
          role="group"
          aria-label={hint}
          className="mt-1 border-t border-[#efefef] px-2.5 pt-2 pb-1 text-[12px] leading-[1.4] text-[#8a8a8a]">
          {hint}
        </div>
      </DropdownMenuContent>
    </DropdownMenu>
  )
}
