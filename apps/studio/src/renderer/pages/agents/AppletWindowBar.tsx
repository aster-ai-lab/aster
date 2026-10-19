import { Button } from '@a3s-lab/ui'
import { ArrowLeft, ArrowRight, Copy, Home, Minus, RotateCw, Square, X } from 'lucide-react'
import { useEffect, useState, type MouseEvent, type ReactNode } from 'react'
import { useTranslation } from 'react-i18next'

import { runAppletWindowAction } from './workbenchAppletWindowBridge'

/**
 * Title bar of a native applet window. The applet mark and title sit at the
 * leading edge. Navigation follows them. Either of those clusters drags the
 * system window. Window controls stay on the trailing edge.
 */
export function AppletWindowBar({
  title,
  icon,
  canBack = false,
  canForward = false,
  onBack,
  onForward,
  onHome,
  onRefresh,
  onClose
}: {
  title: string
  icon: ReactNode
  canBack?: boolean
  canForward?: boolean
  onBack?: () => void
  onForward?: () => void
  onHome: () => void
  onRefresh: () => void
  onClose?: () => void
}) {
  const { t } = useTranslation()
  const [zoomed, setZoomed] = useState(false)
  useEffect(() => {
    document.getElementById('applet-boot-bar')?.remove()
  }, [])
  const zoom = () => {
    setZoomed((value) => !value)
    void runAppletWindowAction('toggle-maximize')
  }
  const drag = (event: MouseEvent) => {
    if (event.button !== 0) return
    void runAppletWindowAction('start-dragging')
  }
  return (
    <header className="relative z-30 flex h-8 shrink-0 items-center border-b border-[#efefef] bg-white">
      <div
        data-ui="workbench.applet.window-bar"
        className="flex h-full min-w-0 shrink-0 items-center gap-1.5 pl-2"
        onMouseDown={drag}
        onDoubleClick={(event) => {
          if (event.button !== 0) return
          event.preventDefault()
          zoom()
        }}>
        <span className="size-4 shrink-0">{icon}</span>
        <h1 className="m-0 max-w-[12rem] truncate text-[14px] leading-tight font-medium text-[#171717]">{title}</h1>
      </div>
      <div
        role="toolbar"
        aria-label={t('workbench.applet.nav')}
        data-ui="workbench.applet.nav"
        data-tauri-drag-region="false"
        className="flex shrink-0 items-center pl-1 [-webkit-app-region:no-drag]">
        <NavButton label={t('workbench.applet.nav.home')} ui="workbench.applet.nav.home" onClick={onHome}>
          <Home className="size-3.5" strokeWidth={1.7} aria-hidden />
        </NavButton>
        <NavButton
          label={t('workbench.applet.nav.back')}
          ui="workbench.applet.nav.back"
          disabled={!canBack}
          onClick={onBack}>
          <ArrowLeft className="size-3.5" strokeWidth={1.7} aria-hidden />
        </NavButton>
        <NavButton
          label={t('workbench.applet.nav.forward')}
          ui="workbench.applet.nav.forward"
          disabled={!canForward}
          onClick={onForward}>
          <ArrowRight className="size-3.5" strokeWidth={1.7} aria-hidden />
        </NavButton>
        <NavButton label={t('workbench.applet.nav.refresh')} ui="workbench.applet.nav.refresh" onClick={onRefresh}>
          <RotateCw className="size-3.5" strokeWidth={1.7} aria-hidden />
        </NavButton>
      </div>
      <div
        className="h-full min-w-0 flex-1"
        onMouseDown={drag}
        onDoubleClick={(event) => {
          if (event.button !== 0) return
          event.preventDefault()
          zoom()
        }}
      />
      <div
        role="group"
        aria-label={t('workbench.applet.window_controls')}
        data-tauri-drag-region="false"
        className="flex shrink-0 items-center gap-0.5 pr-1.5 [-webkit-app-region:no-drag]">
        <WindowControl label={t('workbench.applet.minimize')} onClick={() => void runAppletWindowAction('minimize')}>
          <Minus className="size-3.5" strokeWidth={1.7} aria-hidden />
        </WindowControl>
        <WindowControl label={zoomed ? t('workbench.applet.restore') : t('workbench.applet.maximize')} onClick={zoom}>
          {zoomed ? (
            <Copy className="size-3.5" strokeWidth={1.7} aria-hidden />
          ) : (
            <Square className="size-3.5" strokeWidth={1.7} aria-hidden />
          )}
        </WindowControl>
        <WindowControl
          label={t('common.close')}
          onClick={() => {
            if (onClose) {
              onClose()
              return
            }
            void runAppletWindowAction('close')
          }}>
          <X className="size-3.5" strokeWidth={1.7} aria-hidden />
        </WindowControl>
      </div>
    </header>
  )
}

function NavButton({
  label,
  ui,
  disabled,
  onClick,
  children
}: {
  label: string
  ui: string
  disabled?: boolean
  onClick?: () => void
  children: ReactNode
}) {
  return (
    <Button
      type="button"
      variant="ghost"
      size="icon-sm"
      data-ui={ui}
      data-tauri-drag-region="false"
      aria-label={label}
      disabled={disabled}
      onMouseDown={(event) => event.stopPropagation()}
      onClick={onClick}
      className={chromeButtonClass}>
      {children}
    </Button>
  )
}

function WindowControl({ label, onClick, children }: { label: string; onClick: () => void; children: ReactNode }) {
  return (
    <Button
      type="button"
      variant="ghost"
      size="icon-sm"
      data-tauri-drag-region="false"
      aria-label={label}
      onMouseDown={(event) => event.stopPropagation()}
      onClick={onClick}
      className={chromeButtonClass}>
      {children}
    </Button>
  )
}

const chromeButtonClass =
  'size-6 rounded-[6px] text-[#3f3f3f] hover:bg-[#f2f3f5] hover:text-[#171717] focus-visible:ring-2 focus-visible:ring-inset focus-visible:ring-black/10 disabled:opacity-40'
