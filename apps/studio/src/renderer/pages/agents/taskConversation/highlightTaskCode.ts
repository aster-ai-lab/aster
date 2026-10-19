import { loadStudioShiki } from '@renderer/utils/shikiBundle'

const LANGUAGE_ALIAS: Record<string, string> = {
  ts: 'typescript',
  tsx: 'tsx',
  js: 'javascript',
  jsx: 'jsx',
  py: 'python',
  sh: 'bash',
  shell: 'bash',
  yml: 'yaml',
  md: 'markdown',
  text: 'plaintext',
  txt: 'plaintext',
  rs: 'rust'
}

function loadHighlighter() {
  highlighterPromise ??= (async () => {
    const { createHighlighter } = await loadStudioShiki()
    return createHighlighter({
      themes: ['github-light', 'github-dark'],
      langs: [
        'typescript',
        'tsx',
        'javascript',
        'jsx',
        'python',
        'bash',
        'json',
        'css',
        'html',
        'sql',
        'yaml',
        'markdown',
        'plaintext',
        'rust',
        'go'
      ]
    })
  })()
  return highlighterPromise
}

let highlighterPromise: ReturnType<typeof loadHighlighter> | undefined

/** Highlight a closed fence. Returns undefined when the language cannot be highlighted. */
export async function highlightTaskCode(code: string, language: string): Promise<string | undefined> {
  const highlighter = await loadHighlighter()
  const normalized = language.trim().toLowerCase()
  const candidates = [normalized, LANGUAGE_ALIAS[normalized] ?? '', 'plaintext'].filter(
    (candidate, index, list) => candidate !== '' && list.indexOf(candidate) === index
  )
  for (const lang of candidates) {
    try {
      return highlighter.codeToHtml(code, {
        lang,
        themes: { light: 'github-light', dark: 'github-dark' }
      })
    } catch {
      try {
        return highlighter.codeToHtml(code, { lang, theme: 'github-light' })
      } catch {
        continue
      }
    }
  }
  return undefined
}
