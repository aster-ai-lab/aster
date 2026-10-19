import { describe, expect, it } from 'vitest'

import {
  SCREEN_CAPTURE_PERMISSION_REQUIRED,
  isScreenCapturePermissionError,
  screenshotFileFromResponse,
  screenshotFileName
} from '../screenshotCapture'

const PNG = Uint8Array.from([0x89, 0x50, 0x4e, 0x47, 0x0d, 0x0a, 0x1a, 0x0a, 1, 2, 3])

describe('screenshotFileFromResponse', () => {
  it('treats an empty payload as a cancelled capture', () => {
    expect(screenshotFileFromResponse(new Uint8Array())).toBeNull()
    expect(screenshotFileFromResponse([])).toBeNull()
  })

  it('builds a png file from bytes', async () => {
    const file = screenshotFileFromResponse(PNG, new Date('2026-10-06T01:02:03'))
    expect(file?.type).toBe('image/png')
    expect(file?.name).toBe(screenshotFileName(new Date('2026-10-06T01:02:03')))
    expect(file?.size).toBe(PNG.byteLength)
  })

  it('rejects a payload that is not a png', () => {
    expect(() => screenshotFileFromResponse(Uint8Array.from([1, 2, 3, 4]))).toThrow(/PNG/)
  })
})

describe('isScreenCapturePermissionError', () => {
  it('matches only the screen recording permission token', () => {
    expect(isScreenCapturePermissionError(SCREEN_CAPTURE_PERMISSION_REQUIRED)).toBe(true)
    expect(isScreenCapturePermissionError(new Error(SCREEN_CAPTURE_PERMISSION_REQUIRED))).toBe(true)
    expect(isScreenCapturePermissionError({ message: SCREEN_CAPTURE_PERMISSION_REQUIRED })).toBe(true)
    expect(isScreenCapturePermissionError(new Error('The screenshot tool did not return a valid PNG image.'))).toBe(
      false
    )
    expect(isScreenCapturePermissionError(null)).toBe(false)
  })
})
