/** Fortune Sheet measures its canvas only when the window resizes. */
export function bindSpreadsheetPaneResize(container: HTMLElement): () => void {
  if (typeof ResizeObserver === 'undefined') return () => undefined
  let frame = 0
  let width = container.clientWidth
  let height = container.clientHeight
  const observer = new ResizeObserver(() => {
    if (frame) cancelAnimationFrame(frame)
    frame = requestAnimationFrame(() => {
      frame = 0
      const nextWidth = container.clientWidth
      const nextHeight = container.clientHeight
      if (nextWidth === width && nextHeight === height) return
      width = nextWidth
      height = nextHeight
      window.dispatchEvent(new Event('resize'))
    })
  })
  observer.observe(container)
  return () => {
    observer.disconnect()
    if (frame) cancelAnimationFrame(frame)
  }
}
