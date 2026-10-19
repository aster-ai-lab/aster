import { useEffect, useRef } from 'react'
import { useTranslation } from 'react-i18next'

import { usePreference } from '@data/hooks/usePreference'
import { loggerService } from '@logger'
import {
  captureInteractiveScreenshot,
  deliverCapturedScreenshot,
  isScreenCapturePermissionError,
  openScreenCaptureSettings
} from '@renderer/components/composer/screenshot/screenshotCapture'
import { useCommandHandler } from '@renderer/hooks/command'
import { ipcApi } from '@renderer/ipc'
import { toast } from '@renderer/services/toast'

const logger = loggerService.withContext('useWindowCommands')

const ZOOM_STEP = 0.2
const ZOOM_MIN = 0.2
const ZOOM_MAX = 10

/** Webview zoom matches the Tauri polyfill: steps of 0.2, clamped to 0.2–10, one decimal. */
export function clampZoomFactor(value: number): number {
  if (!Number.isFinite(value)) return 1
  const rounded = Math.round(value * 10) / 10
  return Math.min(ZOOM_MAX, Math.max(ZOOM_MIN, rounded))
}

export function nextZoomFactor(current: number, direction: -1 | 1): number {
  return clampZoomFactor(current + direction * ZOOM_STEP)
}

/**
 * Window-local shortcuts. Mount this before page handlers so a later notes
 * print handler wins while a note file is active.
 */
export function useWindowCommands(): void {
  const { t } = useTranslation()
  const [zoom, setZoom] = usePreference('app.zoom_factor')
  const [screenshotEnabled] = usePreference('feature.screenshot.enabled')
  const appliedZoom = useRef<number | null>(null)
  const requestedZoom = useRef<number | null>(null)
  const stepping = useRef(false)

  useEffect(() => {
    const factor = clampZoomFactor(zoom)
    if (!stepping.current || requestedZoom.current === factor) {
      requestedZoom.current = factor
      stepping.current = false
    }
    if (appliedZoom.current === factor) return
    if (appliedZoom.current === null && factor === 1) {
      appliedZoom.current = 1
      return
    }
    appliedZoom.current = factor
    void ipcApi.request('window.set_zoom', { factor }).catch((error: unknown) => {
      logger.error('Failed to apply window zoom', error as Error)
    })
  }, [zoom])

  const stepZoom = (direction: -1 | 1) => {
    const base = requestedZoom.current ?? clampZoomFactor(zoom)
    const next = nextZoomFactor(base, direction)
    requestedZoom.current = next
    stepping.current = true
    void setZoom(next)
  }

  useCommandHandler('app.zoom.in', () => stepZoom(1))
  useCommandHandler('app.zoom.out', () => stepZoom(-1))
  useCommandHandler('app.zoom.reset', () => {
    requestedZoom.current = 1
    stepping.current = true
    void setZoom(1)
  })
  useCommandHandler('app.window.close', () => {
    void ipcApi.request('window.close').catch((error: unknown) => {
      logger.error('Failed to close the window', error as Error)
    })
  })
  useCommandHandler('app.print', () => {
    window.print()
  })
  useCommandHandler(
    'screenshot.capture',
    () => {
      void captureInteractiveScreenshot('direct')
        .then(async (file) => {
          if (!file) return
          await deliverCapturedScreenshot(file)
        })
        .catch((error: unknown) => {
          if (isScreenCapturePermissionError(error)) {
            void openScreenCaptureSettings().catch(() => undefined)
          }
          toast.error(t('workbench.composer.capture_failed'))
        })
    },
    { enabled: screenshotEnabled === true }
  )
}

export function WindowCommandBindings(): null {
  useWindowCommands()
  return null
}
