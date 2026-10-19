export function getComposerEditorMinHeight(fontSize: number) {
  return Math.ceil(fontSize * 1.4 * 2 + 6)
}

/** Keeps a caller-supplied floor, and never goes below the shared two-line minimum. */
export function resolveComposerEditorMinHeight(fontSize: number, floor?: number) {
  const minimum = getComposerEditorMinHeight(fontSize)
  return floor === undefined ? minimum : Math.max(minimum, floor)
}

export function getCompactComposerEditorMinHeight(fontSize: number) {
  return Math.ceil(fontSize * 1.4 + 6)
}
