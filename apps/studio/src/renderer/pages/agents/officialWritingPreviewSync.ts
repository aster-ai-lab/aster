import type { WritingFields } from './officialWriting'
import {
  OFFICIAL_WRITING_SLOT_BORDER,
  officialWritingFindSlots,
  officialWritingSlotPlaceholder,
  officialWritingSlotText
} from './officialWritingTemplate'

/**
 * One Chinese fill-in range in the live document, such as 发文机关 inside a dashed character border.
 * Positions are ProseMirror positions and move only when an earlier slot changes length.
 */
interface PreviewSlot {
  key: string
  from: number
  to: number
  text: string
  marks: readonly unknown[]
  border: string | null
}

interface PreviewTextNode {
  isText: boolean
  text?: string
  marks?: readonly unknown[]
}

interface PreviewDoc {
  descendants: (visitor: (node: PreviewTextNode, pos: number) => void) => void
  nodesBetween: (from: number, to: number, visitor: (node: PreviewTextNode) => void) => void
  textBetween: (from: number, to: number) => string
}

interface PreviewTransaction {
  doc: PreviewDoc
  docChanged: boolean
  replaceWith: (from: number, to: number, content: unknown) => PreviewTransaction
  setMeta: (key: string, value: unknown) => PreviewTransaction
}

interface PreviewEditorState {
  doc: PreviewDoc
  schema: {
    text: (text: string, marks?: readonly unknown[] | null) => unknown
  }
  tr: PreviewTransaction
}

/** Duck-typed so the home-page mock can update HTML without booting TipTap. */
export interface OfficialWritingPreviewEditor {
  commands?: {
    setContent: (html: string, options?: { emitUpdate?: boolean }) => boolean
  }
  state?: PreviewEditorState
  view?: {
    dispatch: (transaction: PreviewTransaction) => void
  }
}

export interface OfficialWritingPreviewCache {
  slots: PreviewSlot[] | null
  structureHtml: string
}

export function createOfficialWritingPreviewCache(): OfficialWritingPreviewCache {
  return { slots: null, structureHtml: '' }
}

function canPatch(editor: OfficialWritingPreviewEditor): editor is OfficialWritingPreviewEditor & {
  state: PreviewEditorState
  view: { dispatch: (transaction: PreviewTransaction) => void }
} {
  return typeof editor.state?.doc.descendants === 'function' && typeof editor.view?.dispatch === 'function'
}

function scanSlots(doc: PreviewDoc): PreviewSlot[] {
  const slots: PreviewSlot[] = []
  doc.descendants((node, pos) => {
    if (!node.isText || !node.text) return
    for (const found of officialWritingFindSlots(node.text)) {
      if (!found.key) continue
      const from = pos + found.index
      const marks = node.marks ?? []
      slots.push({
        key: found.key,
        from,
        to: from + found.text.length,
        text: found.text,
        marks,
        border: textStyleBorder(marks)
      })
    }
  })
  return slots
}

function slotsMatch(doc: PreviewDoc, slots: readonly PreviewSlot[]): boolean {
  return slots.every((slot) => doc.textBetween(slot.from, slot.to) === slot.text)
}

function shiftSlotsAfter(slots: readonly PreviewSlot[], originTo: number, delta: number) {
  if (delta === 0) return
  for (const slot of slots) {
    if (slot.from >= originTo) {
      slot.from += delta
      slot.to += delta
    }
  }
}

function writeSlots(
  editor: OfficialWritingPreviewEditor & {
    state: PreviewEditorState
    view: { dispatch: (transaction: PreviewTransaction) => void }
  },
  slots: PreviewSlot[],
  fields: WritingFields,
  brief: string
) {
  const claim = { used: false }
  const changes = slots
    .map((slot) => ({ slot, desired: officialWritingSlotText(slot.key, fields, brief, claim) }))
    .filter((change) => change.desired !== change.slot.text && change.desired.length > 0)
    .sort((left, right) => right.slot.from - left.slot.from)
  if (changes.length === 0) return

  let transaction = editor.state.tr
  for (const change of changes) {
    const keep = change.desired === officialWritingSlotPlaceholder(change.slot.key)
    const marks = marksWithRunBorder(
      marksAt(transaction.doc, change.slot.from) ?? change.slot.marks,
      keep ? (change.slot.border ?? OFFICIAL_WRITING_SLOT_BORDER) : null
    )
    transaction = transaction.replaceWith(
      change.slot.from,
      change.slot.to,
      editor.state.schema.text(change.desired, marks)
    )
  }
  if (transaction.docChanged) editor.view.dispatch(transaction.setMeta('addToHistory', false))

  for (const change of changes) {
    const originTo = change.slot.to
    const delta = change.desired.length - change.slot.text.length
    shiftSlotsAfter(slots, originTo, delta)
    change.slot.text = change.desired
    change.slot.to = change.slot.from + change.desired.length
  }
}

function markRecord(mark: unknown): {
  type?: { name?: string; create?: (attrs: Record<string, unknown>) => unknown }
  attrs?: Record<string, unknown>
} | null {
  if (!mark || typeof mark !== 'object') return null
  return mark as {
    type?: { name?: string; create?: (attrs: Record<string, unknown>) => unknown }
    attrs?: Record<string, unknown>
  }
}

function textStyleBorder(marks: readonly unknown[]): string | null {
  for (const mark of marks) {
    const record = markRecord(mark)
    if (record?.type?.name === 'textStyle' && typeof record.attrs?.runBorder === 'string') return record.attrs.runBorder
  }
  return null
}

/** A filled value drops the dashed box. Clearing the field puts that box back on the Chinese name. */
function marksWithRunBorder(marks: readonly unknown[], border: string | null): readonly unknown[] {
  return marks.map((mark) => {
    const record = markRecord(mark)
    if (record?.type?.name !== 'textStyle' || typeof record.type.create !== 'function') return mark
    const attrs = record.attrs ?? {}
    const current = typeof attrs.runBorder === 'string' ? attrs.runBorder : null
    if (current === border) return mark
    return record.type.create({ ...attrs, runBorder: border })
  })
}

function marksAt(doc: PreviewDoc, from: number): readonly unknown[] | null {
  let marks: readonly unknown[] | null = null
  doc.nodesBetween(from, from + 1, (node) => {
    if (node.isText && node.marks) marks = node.marks
  })
  return marks
}

function loadStructure(
  editor: OfficialWritingPreviewEditor & { state: PreviewEditorState },
  structureHtml: string,
  cache: OfficialWritingPreviewCache
) {
  editor.commands?.setContent(structureHtml, { emitUpdate: false })
  cache.structureHtml = structureHtml
  cache.slots = scanSlots(editor.state.doc)
}

/**
 * Replace placeholder text inside the open document.
 * A field edit dispatches one transaction. It does not call setContent.
 * A structure change reloads the skeleton once, then fills the slots again.
 */
export function syncOfficialWritingPreview(
  editor: OfficialWritingPreviewEditor,
  options: {
    brief: string
    cache: OfficialWritingPreviewCache
    fields: WritingFields
    filledHtml: string
    structureHtml: string
  }
): void {
  const { brief, cache, fields, filledHtml, structureHtml } = options
  if (!canPatch(editor)) {
    editor.commands?.setContent(filledHtml, { emitUpdate: false })
    return
  }

  const structureChanged = cache.structureHtml !== structureHtml || cache.slots === null
  if (structureChanged) {
    let slots = scanSlots(editor.state.doc)
    if (slots.length === 0 && structureHtml.includes('data-writing-slot')) {
      loadStructure(editor, structureHtml, cache)
      slots = cache.slots ?? []
    } else {
      cache.structureHtml = structureHtml
      cache.slots = slots
    }
  }

  const slots = cache.slots
  if (!slots || slots.length === 0) {
    editor.commands?.setContent(filledHtml, { emitUpdate: false })
    return
  }
  if (!slotsMatch(editor.state.doc, slots)) {
    loadStructure(editor, structureHtml, cache)
    if (!cache.slots || cache.slots.length === 0 || !slotsMatch(editor.state.doc, cache.slots)) {
      editor.commands?.setContent(filledHtml, { emitUpdate: false })
      cache.slots = null
      return
    }
  }
  writeSlots(editor, cache.slots, fields, brief)
}
