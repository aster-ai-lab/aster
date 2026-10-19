/** Dirty note kept in memory while the knowledge page is unmounted. Never written to the file. */
export interface HeldNoteDraft {
  path: string
  text: string
  title: string | null
  savedMarkdown: string
}

let heldNoteDraft: HeldNoteDraft | null = null

export function readHeldNoteDraft(): HeldNoteDraft | null {
  return heldNoteDraft
}

export function writeHeldNoteDraft(draft: HeldNoteDraft | null): void {
  heldNoteDraft = draft
}
