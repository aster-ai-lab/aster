import { OFFICE_CREATE_EXTENSION, type OfficeCreateKind } from './blankOfficeFile'

/** Why a typed office file name cannot be created yet. */
export type OfficeNameIssue = 'empty' | 'invalid' | 'reserved' | 'too_long' | 'taken' | 'failed'

export type OfficeCreateOutcome = 'ok' | 'taken' | 'failed'

const RESERVED_STEM = /^(con|prn|aux|nul|com[1-9]|lpt[1-9])$/iu
const INVALID_CHAR = /[\\/:*?"<>|\u0000-\u001f]/u
const MAX_FILE_NAME_LENGTH = 255

function normalizeKey(value: string): string {
  return value.normalize('NFC').toLocaleLowerCase()
}

/** Drop one trailing extension the field already shows beside the input. */
export function officeCreateStem(raw: string, extension: string): string {
  const trimmed = raw.trim().normalize('NFC')
  const suffix = `.${extension.toLowerCase()}`
  if (trimmed.length > suffix.length && trimmed.toLocaleLowerCase().endsWith(suffix)) {
    return trimmed.slice(0, trimmed.length - suffix.length).trim()
  }
  return trimmed
}

export function officeCreateFileName(raw: string, extension: string): string {
  return `${officeCreateStem(raw, extension)}.${extension}`
}

export function officeCreateIssue(raw: string, extension: string, taken: readonly string[]): OfficeNameIssue | null {
  const stem = officeCreateStem(raw, extension)
  if (!stem || stem === '.' || stem === '..') return 'empty'
  if (stem.startsWith('.') || INVALID_CHAR.test(stem) || /[. ]$/u.test(stem)) return 'invalid'
  if (RESERVED_STEM.test(stem)) return 'reserved'
  const full = `${stem}.${extension}`
  if ([...full].length > MAX_FILE_NAME_LENGTH) return 'too_long'
  const key = normalizeKey(full)
  if (taken.some((name) => normalizeKey(name) === key)) return 'taken'
  return null
}

/** Default stem for a kind. Adds " 2", " 3", … when that file is already in the folder. */
export function uniqueOfficeStem(taken: readonly string[], stem: string, extension: string): string {
  const base = officeCreateStem(stem, extension) || stem.trim()
  if (!officeCreateIssue(base, extension, taken)) return base
  for (let index = 2; index < 100; index += 1) {
    const candidate = `${base} ${index}`
    if (!officeCreateIssue(candidate, extension, taken)) return candidate
  }
  return `${base} ${taken.length + 1}`
}

export function officeExtension(kind: OfficeCreateKind): string {
  return OFFICE_CREATE_EXTENSION[kind]
}
