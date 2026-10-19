import { fileExtension, previewKind } from './documentPreview'
import { monacoLanguage } from './monacoLanguage'

/** Word wrap and rich preview stay responsive under this size. */
export const COMFORTABLE_TEXT_CHARS = 256_000

/** Monaco stops tokenizing past this many characters. A larger model stalls the window. */
export const VIRTUAL_TEXT_CHARS = 20 * 1024 * 1024

/** TipTap, the HTML frame, and the spreadsheet grid stall past this size. */
export const VISUAL_TEXT_UNITS = 256 * 1024

export const TEXT_READ_BYTES = VIRTUAL_TEXT_CHARS
export const PDF_READ_BYTES = 32 * 1024 * 1024
/**
 * Matches the workspace raw read (`RAW_LIMIT`, 64 MiB). A picture-heavy
 * document is often tens of megabytes; the office editor windows pages, so
 * this size still opens and edits. Larger files stay on the system app.
 */
export const DOCUMENT_READ_BYTES = 64 * 1024 * 1024
export const SPREADSHEET_READ_BYTES = 8 * 1024 * 1024
export const PRESENTATION_READ_BYTES = 12 * 1024 * 1024

const VISUAL_EXTENSIONS = new Set(['md', 'markdown', 'csv', 'tsv', 'html', 'htm', 'xhtml', 'shtml', 'svg'])

export type CodeSurface = 'monaco' | 'monaco-large' | 'textarea' | 'too-large'

export function prefersLargeCodeEditor(value: string): boolean {
  if (value.length > COMFORTABLE_TEXT_CHARS) return true
  const head = value.slice(0, 10_001)
  return head.length > 10_000 && !head.includes('\n')
}

/** Test mode keeps the plain field so existing editor tests can type. */
export function codeSurface(value: string, testMode: boolean): CodeSurface {
  if (value.length > VIRTUAL_TEXT_CHARS) return 'too-large'
  if (testMode) return 'textarea'
  if (prefersLargeCodeEditor(value)) return 'monaco-large'
  return 'monaco'
}

export function readBudgetBytes(fileName: string): number {
  const extension = fileExtension(fileName)
  const kind = previewKind(fileName)
  if (kind === 'pdf' || kind === 'image' || kind === 'audio' || kind === 'video') return PDF_READ_BYTES
  if (kind === 'presentation') return PRESENTATION_READ_BYTES
  if (kind === 'document') return DOCUMENT_READ_BYTES
  if (kind === 'spreadsheet' && extension !== 'csv' && extension !== 'tsv') return SPREADSHEET_READ_BYTES
  return TEXT_READ_BYTES
}

/** Right-hand overview. Large files skip it so the window stays responsive. */
export function codeMinimap(large: boolean): {
  enabled: boolean
  side: 'right'
  showSlider: 'mouseover'
  renderCharacters: true
  maxColumn: 120
} {
  return {
    enabled: !large,
    side: 'right',
    showSlider: 'mouseover',
    renderCharacters: true,
    maxColumn: 120
  }
}

/** Markdown, CSV, HTML, and SVG above the visual budget open as source text. */
export function visualSourceAllowed(extension: string, byteLength: number): boolean {
  if (!VISUAL_EXTENSIONS.has(extension)) return true
  return byteLength <= VISUAL_TEXT_UNITS
}

export function largeMonacoOverrides(
  fileName: string,
  charLength: number
): {
  wordWrap: 'off'
  language: string
  folding: false
  links: false
  occurrencesHighlight: 'off'
  selectionHighlight: false
  matchBrackets: 'never'
  wordBasedSuggestions: 'off'
  quickSuggestions: false
  stopRenderingLineAfter: number
  maxTokenizationLineLength: number
  stickyScroll: { enabled: false }
  unicodeHighlight: { ambiguousCharacters: false; invisibleCharacters: false; nonBasicASCII: false }
  hover: { enabled: false }
} {
  return {
    wordWrap: 'off',
    language: charLength > 1_000_000 ? 'plaintext' : monacoLanguage(fileName),
    folding: false,
    links: false,
    occurrencesHighlight: 'off',
    selectionHighlight: false,
    matchBrackets: 'never',
    wordBasedSuggestions: 'off',
    quickSuggestions: false,
    stopRenderingLineAfter: 10_000,
    maxTokenizationLineLength: 1_000,
    stickyScroll: { enabled: false },
    unicodeHighlight: { ambiguousCharacters: false, invisibleCharacters: false, nonBasicASCII: false },
    hover: { enabled: false }
  }
}
