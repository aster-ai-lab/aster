import { ChevronDown, EyeOff, Scissors } from 'lucide-react'
import { type Dispatch, type SetStateAction, useEffect, useId, useRef, useState } from 'react'
import { createPortal } from 'react-dom'
import { useTranslation } from 'react-i18next'

import { desktopCapabilities } from '@renderer/host/desktopHost'
import { toast } from '@renderer/services/toast'
import type { ComposerAttachment } from '@renderer/utils/message/composerAttachment'
import { cn } from '@renderer/utils/style'

import {
  attachScreenshotFile,
  captureInteractiveScreenshot,
  isScreenCapturePermissionError,
  openScreenCaptureSettings,
  type ScreenshotMode,
  subscribeWorkbenchScreenshots
} from './screenshotCapture'

interface ComposerScreenshotControlProps {
  disabled?: boolean
  /** Messages stages the file in its own composer. */
  onCapture?: (file: File) => void | Promise<void>
  /** Homepage and chat composers append a path-backed attachment. */
  setFiles?: Dispatch<SetStateAction<ComposerAttachment[]>>
  /** The homepage composer accepts a capture taken while its fallback was showing. */
  acceptQueued?: boolean
}

const MENU_WIDTH = 196
const MENU_HEIGHT = 44

function menuPosition(rect: DOMRect): { left: number; top: number } {
  const left = Math.max(8, Math.min(rect.left, window.innerWidth - MENU_WIDTH - 8))
  const openUp = window.innerHeight - rect.bottom < MENU_HEIGHT + 12
  const top = openUp ? Math.max(8, rect.top - MENU_HEIGHT - 4) : rect.bottom + 4
  return { left, top }
}

export function ComposerScreenshotControl({
  disabled = false,
  onCapture,
  setFiles,
  acceptQueued = false
}: ComposerScreenshotControlProps) {
  const { t } = useTranslation()
  const menuId = useId()
  const rootRef = useRef<HTMLDivElement>(null)
  const menuRef = useRef<HTMLDivElement>(null)
  const hideItemRef = useRef<HTMLButtonElement>(null)
  const [open, setOpen] = useState(false)
  const [busy, setBusy] = useState(false)
  const [menuPos, setMenuPos] = useState<{ left: number; top: number } | null>(null)

  useEffect(() => {
    if (!acceptQueued || !setFiles) return
    return subscribeWorkbenchScreenshots((file) => {
      void attachScreenshotFile(file, setFiles).catch(() => {
        toast.error(t('workbench.composer.file_unavailable'))
      })
    })
  }, [acceptQueued, setFiles, t])

  useEffect(() => {
    if (!open) return
    hideItemRef.current?.focus()
    const close = (event: PointerEvent) => {
      const target = event.target
      if (!(target instanceof Node)) return
      if (rootRef.current?.contains(target) || menuRef.current?.contains(target)) return
      setOpen(false)
    }
    const onKey = (event: KeyboardEvent) => {
      if (event.key === 'Escape') setOpen(false)
    }
    const dismiss = () => setOpen(false)
    document.addEventListener('pointerdown', close, true)
    document.addEventListener('keydown', onKey)
    window.addEventListener('resize', dismiss)
    window.addEventListener('scroll', dismiss, true)
    return () => {
      document.removeEventListener('pointerdown', close, true)
      document.removeEventListener('keydown', onKey)
      window.removeEventListener('resize', dismiss)
      window.removeEventListener('scroll', dismiss, true)
    }
  }, [open])

  const desktop = desktopCapabilities()
  if (desktop.platform && !desktop.screenshot) return null

  const deliver = async (file: File) => {
    if (onCapture) {
      await onCapture(file)
      return
    }
    if (setFiles) {
      await attachScreenshotFile(file, setFiles)
      return
    }
    throw new Error('The screenshot has nowhere to attach.')
  }

  const capture = async (mode: ScreenshotMode) => {
    if (busy || disabled) return
    setOpen(false)
    setBusy(true)
    toast.info(
      t(mode === 'hide-window' ? 'workbench.composer.capture_hide_hint' : 'workbench.composer.capture_waiting')
    )
    try {
      const file = await captureInteractiveScreenshot(mode)
      if (!file) {
        toast.info(t('workbench.composer.capture_cancelled'))
        return
      }
      await deliver(file)
    } catch (error) {
      if (isScreenCapturePermissionError(error)) {
        // The failure toast still explains the block when the pane cannot open.
        void openScreenCaptureSettings().catch(() => undefined)
      }
      toast.error(t('workbench.composer.capture_failed'))
    } finally {
      setBusy(false)
    }
  }

  const toggleMenu = () => {
    if (busy || disabled) return
    if (open) {
      setOpen(false)
      return
    }
    const rect = rootRef.current?.getBoundingClientRect()
    if (!rect) return
    setMenuPos(menuPosition(rect))
    setOpen(true)
  }

  const menu =
    open && menuPos
      ? createPortal(
          <div
            ref={menuRef}
            id={menuId}
            role="menu"
            aria-label={t('workbench.composer.capture_options')}
            data-ui="composer.screenshot.menu"
            style={{ left: menuPos.left, top: menuPos.top, width: MENU_WIDTH }}
            className="fixed z-[90] rounded-[12px] border border-[#ebebeb] bg-white p-1 shadow-[0_12px_32px_rgba(0,0,0,0.12)]">
            <button
              ref={hideItemRef}
              type="button"
              role="menuitem"
              data-ui="composer.screenshot.hide"
              onClick={() => void capture('hide-window')}
              className="flex h-9 w-full items-center gap-2.5 rounded-[8px] px-2.5 text-left text-[13px] text-[#3f3f3f] outline-none hover:bg-[#f7f7f7] hover:text-[#171717] focus-visible:bg-[#f7f7f7] focus-visible:text-[#171717]">
              <EyeOff className="size-4 shrink-0" strokeWidth={1.7} aria-hidden />
              <span className="min-w-0 truncate">{t('workbench.composer.capture_hide')}</span>
            </button>
          </div>,
          document.body
        )
      : null

  return (
    <div ref={rootRef} data-ui="composer.screenshot" className="relative flex items-center">
      <button
        type="button"
        data-ui="composer.screenshot.capture"
        aria-label={t('workbench.composer.capture')}
        title={t('workbench.composer.capture_direct_hint')}
        aria-busy={busy || undefined}
        disabled={disabled || busy}
        onClick={() => void capture('direct')}
        className="flex size-8 items-center justify-center rounded-[8px] text-[#171717] outline-none hover:bg-[#f5f5f5] focus-visible:ring-2 focus-visible:ring-black/10 disabled:opacity-40">
        <Scissors className="size-4" strokeWidth={1.7} />
      </button>
      <button
        type="button"
        data-ui="composer.screenshot.options"
        aria-label={t('workbench.composer.capture_options')}
        title={t('workbench.composer.capture_options')}
        aria-controls={open ? menuId : undefined}
        aria-expanded={open}
        aria-haspopup="menu"
        disabled={disabled || busy}
        onClick={toggleMenu}
        className="-ml-1 flex h-8 w-6 items-center justify-center rounded-[6px] text-[#171717] outline-none hover:bg-[#f5f5f5] focus-visible:ring-2 focus-visible:ring-black/10 disabled:opacity-40">
        <ChevronDown className={cn('size-3', open && 'rotate-180')} strokeWidth={1.7} />
      </button>
      {menu}
    </div>
  )
}
