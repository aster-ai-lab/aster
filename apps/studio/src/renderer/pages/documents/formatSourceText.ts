/** Pretty-print JSON. Other text drops trailing spaces and extra blank lines. */
export function formatSourceText(value: string, language: string): string {
  if (language === 'json') {
    return `${JSON.stringify(JSON.parse(value), null, 2)}\n`
  }
  const lines = value.replace(/\r\n?/gu, '\n').split('\n')
  const normalized: string[] = []
  let empty = 0
  for (const line of lines) {
    const clean = line.replace(/[\t ]+$/u, '')
    if (!clean) {
      empty += 1
      if (empty > 2) continue
    } else {
      empty = 0
    }
    normalized.push(clean)
  }
  return `${normalized.join('\n').replace(/\n+$/u, '')}\n`
}
