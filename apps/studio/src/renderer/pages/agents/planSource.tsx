import { HighlightStyle, syntaxHighlighting } from '@codemirror/language'
import {
  Decoration,
  EditorView,
  keymap,
  ViewPlugin,
  type DecorationSet,
  type Extension,
  type ViewUpdate
} from '@codemirror/view'
import { tags } from '@lezer/highlight'
import CodeMirror from '@uiw/react-codemirror'
import { useEffect, useImperativeHandle, useMemo, useRef, useState, type Ref } from 'react'

import type { PlanMarkup } from './planModel'

export type PlanSourceHandle = {
  selection: () => { start: number; end: number }
  focusAt: (cursor: number) => void
  scrollToRatio: (ratio: number) => boolean
}

const markdownHighlight = HighlightStyle.define([
  { tag: tags.heading1, color: '#171717', fontWeight: '650' },
  { tag: tags.heading2, color: '#1f3d32', fontWeight: '650' },
  { tag: tags.heading3, color: '#3f3f3f', fontWeight: '600' },
  { tag: tags.heading4, color: '#5c5c5c', fontWeight: '600' },
  { tag: tags.strong, color: '#171717', fontWeight: '650' },
  { tag: tags.emphasis, fontStyle: 'italic' },
  { tag: tags.monospace, color: '#3f3f3f' },
  { tag: tags.link, textDecoration: 'underline' },
  { tag: tags.comment, color: '#8a8a8a' },
  { tag: tags.meta, color: '#8a8a8a' },
  { tag: tags.processingInstruction, color: '#8a8a8a' }
])

const headingLine = Decoration.line({ class: 'cm-plan-heading' })

function headingLines(view: EditorView): DecorationSet {
  const marks = []
  for (const range of view.visibleRanges) {
    let pos = range.from
    while (pos <= range.to) {
      const line = view.state.doc.lineAt(pos)
      if (/^#{1,6}\s+\S/.test(line.text)) marks.push(headingLine.range(line.from))
      if (line.to >= view.state.doc.length) break
      pos = line.to + 1
    }
  }
  return Decoration.set(marks)
}

const headingTint = ViewPlugin.fromClass(
  class {
    decorations: DecorationSet
    constructor(view: EditorView) {
      this.decorations = headingLines(view)
    }
    update(update: ViewUpdate) {
      if (update.docChanged || update.viewportChanged) this.decorations = headingLines(update.view)
    }
  },
  { decorations: (value) => value.decorations }
)

export function PlanSource({
  handle,
  value,
  label,
  fontSize,
  onChange,
  onMarkup,
  onScrollRatio,
  onSelectionCount
}: {
  handle: Ref<PlanSourceHandle>
  value: string
  label: string
  fontSize: number
  onChange: (value: string) => void
  onMarkup: (kind: PlanMarkup) => void
  onScrollRatio: (ratio: number) => void
  onSelectionCount: (count: number) => void
}) {
  const viewRef = useRef<EditorView | null>(null)
  const markupRef = useRef(onMarkup)
  const scrollRef = useRef(onScrollRatio)
  const selectionRef = useRef(onSelectionCount)
  const [language, setLanguage] = useState<Extension[]>([])
  markupRef.current = onMarkup
  scrollRef.current = onScrollRatio
  selectionRef.current = onSelectionCount

  useEffect(() => {
    let cancel = false
    void import('@uiw/codemirror-extensions-langs').then(({ loadLanguage }) => {
      if (cancel) return
      const extension = loadLanguage('markdown')
      if (extension) setLanguage([extension])
    })
    return () => {
      cancel = true
    }
  }, [])

  useImperativeHandle(
    handle,
    () => ({
      selection: () => {
        const view = viewRef.current
        if (!view) return { start: value.length, end: value.length }
        const range = view.state.selection.main
        return { start: range.from, end: range.to }
      },
      focusAt: (cursor: number) => {
        const view = viewRef.current
        if (!view) return
        const pos = Math.max(0, Math.min(cursor, view.state.doc.length))
        view.dispatch({ selection: { anchor: pos }, scrollIntoView: true })
        view.focus()
      },
      scrollToRatio: (ratio: number) => {
        const scroller = viewRef.current?.scrollDOM
        if (!scroller) return false
        const max = scroller.scrollHeight - scroller.clientHeight
        const next = Math.max(0, Math.min(1, ratio)) * Math.max(0, max)
        if (Math.abs(scroller.scrollTop - next) < 1) return false
        scroller.scrollTop = next
        return true
      }
    }),
    [value.length]
  )

  const extensions = useMemo(
    () => [
      ...language,
      syntaxHighlighting(markdownHighlight),
      headingTint,
      EditorView.lineWrapping,
      EditorView.theme({
        '&': { fontSize: `${fontSize}px`, backgroundColor: '#ffffff', height: '100%' },
        '.cm-scroller': {
          fontFamily: 'ui-monospace, SFMono-Regular, Menlo, Consolas, monospace',
          lineHeight: '1.65',
          overflow: 'auto'
        },
        '.cm-content': { padding: '8px 0', caretColor: '#171717' },
        '.cm-gutters': { backgroundColor: '#ffffff', color: '#8a8a8a', borderRight: '1px solid #efefef' },
        '.cm-activeLine': { backgroundColor: '#f7f7f7' },
        '.cm-activeLineGutter': { backgroundColor: '#f7f7f7' },
        '.cm-plan-heading': { backgroundColor: '#f3f6f4' }
      }),
      keymap.of([
        {
          key: 'Mod-b',
          preventDefault: true,
          run: () => {
            markupRef.current('bold')
            return true
          }
        },
        {
          key: 'Mod-i',
          preventDefault: true,
          run: () => {
            markupRef.current('italic')
            return true
          }
        }
      ])
    ],
    [fontSize, language]
  )

  return (
    <div data-ui="workbench.plan.source" aria-label={label} className="relative min-h-0 flex-1">
      <CodeMirror
        value={value}
        className="absolute inset-0 [&_.cm-editor]:h-full"
        height="100%"
        theme="none"
        basicSetup={{ lineNumbers: true, foldGutter: true, highlightActiveLine: true }}
        extensions={extensions}
        onChange={onChange}
        onCreateEditor={(view) => {
          viewRef.current = view
          const scroller = view.scrollDOM
          const onScroll = () => {
            const max = scroller.scrollHeight - scroller.clientHeight
            scrollRef.current(max <= 0 ? 0 : scroller.scrollTop / max)
          }
          scroller.addEventListener('scroll', onScroll, { passive: true })
        }}
        onUpdate={(update) => {
          if (!update.selectionSet && !update.docChanged) return
          const range = update.state.selection.main
          selectionRef.current(Math.abs(range.to - range.from))
        }}
      />
    </div>
  )
}
