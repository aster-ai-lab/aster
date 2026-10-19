import type { Dispatch, SetStateAction } from 'react'

import { ipcApi } from '@renderer/ipc'
import type { ComposerAttachment } from '@renderer/utils/message/composerAttachment'
import { toComposerAttachment } from '@renderer/utils/message/composerAttachment'

export type ScreenshotMode = 'direct' | 'hide-window'

/** Matches the token returned by the desktop capture command. */
export const SCREEN_CAPTURE_PERMISSION_REQUIRED = 'screen-capture-permission-required'

function errorText(error: unknown): string {
  if (typeof error === 'string') return error
  if (error instanceof Error) return error.message
  if (typeof error === 'object' && error !== null && 'message' in error) {
    const message = (error as { message?: unknown }).message
    if (typeof message === 'string') return message
  }
  return ''
}

export function isScreenCapturePermissionError(error: unknown): boolean {
  return errorText(error).includes(SCREEN_CAPTURE_PERMISSION_REQUIRED)
}

/** Opens Privacy & Security → Screen Recording. The website opener rejects this pane. */
export function openScreenCaptureSettings(): Promise<void> {
  return ipcApi.request('system.mac.open_screen_capture_settings')
}

const PNG_MAGIC = [0x89, 0x50, 0x4e, 0x47, 0x0d, 0x0a, 0x1a, 0x0a]
const MAXIMUM_SCREENSHOT_BYTES = 20 * 1024 * 1024

interface TauriInternals {
  invoke?: (command: string, args?: Record<string, unknown>) => Promise<unknown>
}

function tauriInvoke(): TauriInternals['invoke'] | undefined {
  return (globalThis as typeof globalThis & { __TAURI_INTERNALS__?: TauriInternals }).__TAURI_INTERNALS__?.invoke
}

function screenshotBytes(response: unknown): Uint8Array {
  if (response instanceof ArrayBuffer) return new Uint8Array(response)
  if (ArrayBuffer.isView(response)) {
    return new Uint8Array(response.buffer, response.byteOffset, response.byteLength)
  }
  if (Array.isArray(response) && response.every((value) => Number.isInteger(value) && value >= 0 && value <= 255)) {
    return Uint8Array.from(response as number[])
  }
  throw new Error('The native screenshot response was not binary data.')
}

function twoDigits(value: number): string {
  return value.toString().padStart(2, '0')
}

export function screenshotFileName(capturedAt = new Date()): string {
  return [
    'screenshot-',
    capturedAt.getFullYear(),
    twoDigits(capturedAt.getMonth() + 1),
    twoDigits(capturedAt.getDate()),
    '-',
    twoDigits(capturedAt.getHours()),
    twoDigits(capturedAt.getMinutes()),
    twoDigits(capturedAt.getSeconds()),
    '.png'
  ].join('')
}

/** `null` means the user cancelled. Anything else is a PNG file. */
export function screenshotFileFromResponse(response: unknown, capturedAt = new Date()): File | null {
  const bytes = screenshotBytes(response)
  if (bytes.byteLength === 0) return null
  if (bytes.byteLength > MAXIMUM_SCREENSHOT_BYTES) {
    throw new Error('The screenshot exceeds the 20 MiB attachment limit.')
  }
  const header = bytes.subarray(0, PNG_MAGIC.length)
  if (PNG_MAGIC.some((byte, index) => header[index] !== byte)) {
    throw new Error('The screenshot tool did not return a valid PNG image.')
  }
  return new File([Uint8Array.from(bytes)], screenshotFileName(capturedAt), { type: 'image/png' })
}

export async function captureInteractiveScreenshot(mode: ScreenshotMode): Promise<File | null> {
  const invoke = tauriInvoke()
  if (!invoke) throw new Error('Interactive screenshots are only available in the desktop window.')
  try {
    const response = await invoke('capture_interactive_screenshot', { mode })
    return screenshotFileFromResponse(response)
  } catch (error) {
    if (isScreenCapturePermissionError(error)) throw new Error(SCREEN_CAPTURE_PERMISSION_REQUIRED)
    throw error
  }
}

const queued: File[] = []
const listeners = new Set<(file: File) => void>()

/** Hold a capture until the homepage composer that can attach it is mounted. */
export function queueWorkbenchScreenshot(file: File): void {
  if (listeners.size === 0) {
    queued.push(file)
    return
  }
  for (const listener of listeners) listener(file)
}

/**
 * Give a shortcut capture to a mounted composer. With no listener, copy the PNG
 * instead of queueing it, so a later homepage mount does not attach it by surprise.
 */
export async function deliverCapturedScreenshot(file: File): Promise<void> {
  if (listeners.size > 0) {
    for (const listener of listeners) listener(file)
    return
  }
  await navigator.clipboard.write([new ClipboardItem({ 'image/png': file })])
}

export function subscribeWorkbenchScreenshots(listener: (file: File) => void): () => void {
  listeners.add(listener)
  const pending = queued.splice(0, queued.length)
  for (const file of pending) listener(file)
  return () => {
    listeners.delete(listener)
  }
}

export async function attachScreenshotFile(
  file: File,
  setFiles: Dispatch<SetStateAction<ComposerAttachment[]>>
): Promise<void> {
  const key = await window.api.file.createTempFile(file.name || 'screenshot.png')
  await window.api.file.write(key, new Uint8Array(await file.arrayBuffer()))
  const saved = await window.api.file.get(key)
  if (!saved) throw new Error('The screenshot could not be saved into the workspace.')
  const origin = file.name.replace(/\.png$/i, '') || saved.origin_name
  setFiles((current) => [...current, toComposerAttachment({ ...saved, origin_name: origin })])
}
