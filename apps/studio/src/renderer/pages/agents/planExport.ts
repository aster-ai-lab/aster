/** Default download name. Unsafe path characters are removed. */
export function planExportName(title: string, extension: string): string {
  const base =
    title
      .replace(/[\\/:*?"<>|]/g, '')
      .trim()
      .slice(0, 80) || 'plan'
  return `${base}.${extension}`
}

type PlanFileSave = (
  name: string,
  content: string | Uint8Array,
  options?: { filters?: { name: string; extensions: string[] }[] }
) => Promise<unknown>

function readSave(): PlanFileSave | null {
  const host = window as Window & { api?: { file?: { save?: unknown } } }
  const save = host.api?.file?.save
  return typeof save === 'function' ? (save as PlanFileSave) : null
}

/** Saves through the desktop file dialog. Falls back to a browser download in tests. */
export async function savePlanFile(name: string, content: string | Uint8Array): Promise<'saved' | 'cancelled'> {
  const extension = name.split('.').pop() || 'txt'
  const save = readSave()
  if (save) {
    const result = await save(name, content, { filters: [{ name: extension, extensions: [extension] }] })
    return result == null || result === false ? 'cancelled' : 'saved'
  }
  const blob =
    typeof content === 'string' ? new Blob([content], { type: 'text/plain;charset=utf-8' }) : new Blob([content])
  const url = URL.createObjectURL(blob)
  const link = document.createElement('a')
  link.href = url
  link.download = name
  link.rel = 'noopener'
  document.body.appendChild(link)
  link.click()
  link.remove()
  URL.revokeObjectURL(url)
  return 'saved'
}

/** Prints the on-screen pages when they are mounted, otherwise the escaped document. */
export function printPlanPages(preview: HTMLElement | null, fallbackHtml: string): void {
  const frame = document.createElement('iframe')
  frame.setAttribute('aria-hidden', 'true')
  frame.style.cssText = 'position:fixed;right:0;bottom:0;width:0;height:0;border:0'
  document.body.appendChild(frame)
  const doc = frame.contentDocument
  const win = frame.contentWindow
  if (!doc || !win) {
    frame.remove()
    throw new Error('print frame is unavailable')
  }
  if (preview) {
    const styles = Array.from(document.querySelectorAll('link[rel="stylesheet"], style'))
      .map((node) => node.outerHTML)
      .join('')
    doc.open()
    const reset =
      '<style>@page{size:A4;margin:12mm}[data-plan-slot]{width:210mm!important;height:297mm!important;margin:0 auto 8mm!important}[data-plan-scale]{transform:none!important;width:210mm!important;height:297mm!important}</style>'
    doc.write(
      `<!DOCTYPE html><html><head><meta charset="utf-8">${styles}${reset}</head><body>${preview.innerHTML}</body></html>`
    )
    doc.close()
  } else {
    doc.open()
    doc.write(fallbackHtml)
    doc.close()
  }
  window.setTimeout(() => {
    try {
      win.focus()
      win.print()
    } finally {
      window.setTimeout(() => frame.remove(), 1000)
    }
  }, 200)
}
