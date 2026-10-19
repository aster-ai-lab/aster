import type { editor as EditorApi } from 'monaco-editor'
import * as monaco from 'monaco-editor/editor/editor.api.js'
import { useEffect, useRef } from 'react'

import './loadMonaco'
import { codeMinimap, largeMonacoOverrides } from './fileOpenBudget'
import { monacoLanguage } from './monacoLanguage'

export type CodeEditorCommands = {
  rememberSelection: () => void
  cut: () => void
  copy: () => void
  paste: () => void
  selectAll: () => void
}

async function writeClipboard(text: string): Promise<void> {
  try {
    await navigator.clipboard?.writeText(text)
  } catch {
    // A denied clipboard still lets cut remove the selection.
  }
}

async function readClipboard(): Promise<string> {
  try {
    return (await navigator.clipboard?.readText()) ?? ''
  } catch {
    return ''
  }
}

let themeReady = false

function ensureTheme() {
  if (themeReady) return
  themeReady = true
  monaco.editor.defineTheme('a3s-file', {
    base: 'vs',
    inherit: true,
    rules: [],
    colors: {
      'editor.background': '#ffffff',
      'editor.foreground': '#171717',
      'editor.lineHighlightBackground': '#f5f5f5',
      'editorLineNumber.foreground': '#8a8a8a',
      'editorLineNumber.activeForeground': '#3f3f3f',
      'editorCursor.foreground': '#171717',
      'editor.selectionBackground': '#17171714',
      'editorWidget.background': '#ffffff',
      'editorWidget.border': '#ececec'
    }
  })
}

export function MonacoCodeEditor({
  fileName,
  value,
  large = false,
  readOnly = false,
  onChange,
  onDirty,
  onSave,
  onCursor,
  onMounted,
  onFormat,
  valueReader,
  commands
}: {
  fileName: string
  value: string
  large?: boolean
  readOnly?: boolean
  onChange: (value: string) => void
  onDirty?: () => void
  onSave: (value: string) => void
  onCursor?: (cursor: { line: number; column: number }) => void
  onMounted?: () => void
  onFormat?: () => void
  valueReader?: { current: (() => string) | null }
  commands?: { current: CodeEditorCommands | null }
}) {
  const host = useRef<HTMLDivElement>(null)
  const editorRef = useRef<EditorApi.IStandaloneCodeEditor | null>(null)
  const valueRef = useRef(value)
  const onChangeRef = useRef(onChange)
  const onDirtyRef = useRef(onDirty)
  const onSaveRef = useRef(onSave)
  const onCursorRef = useRef(onCursor)
  const onMountedRef = useRef(onMounted)
  const largeRef = useRef(large)
  const readOnlyRef = useRef(readOnly)
  const valueReaderRef = useRef(valueReader)
  const onFormatRef = useRef(onFormat)
  const commandsRef = useRef(commands)
  valueRef.current = value
  onChangeRef.current = onChange
  onDirtyRef.current = onDirty
  onSaveRef.current = onSave
  onCursorRef.current = onCursor
  onMountedRef.current = onMounted
  largeRef.current = large
  readOnlyRef.current = readOnly
  valueReaderRef.current = valueReader
  onFormatRef.current = onFormat
  commandsRef.current = commands

  useEffect(() => {
    const node = host.current
    if (!node) return undefined
    let code: EditorApi.IStandaloneCodeEditor | null = null
    let disposeModelEvents: (() => void) | null = null
    let cancelled = false
    const frame = requestAnimationFrame(() => {
      if (cancelled) return
      ensureTheme()
      const source = valueRef.current
      const heavy = largeRef.current
      code = monaco.editor.create(node, {
        value: source,
        theme: 'a3s-file',
        ariaLabel: fileName,
        automaticLayout: true,
        readOnly: readOnlyRef.current,
        fontFamily: 'ui-monospace, SFMono-Regular, Menlo, monospace',
        fontSize: 13,
        lineHeight: 22,
        minimap: codeMinimap(heavy),
        contextmenu: false,
        scrollBeyondLastLine: false,
        tabSize: 2,
        padding: { top: 12, bottom: 12 },
        renderLineHighlight: 'line',
        overviewRulerBorder: false,
        scrollbar: { verticalScrollbarSize: 8, horizontalScrollbarSize: 8 },
        largeFileOptimizations: true,
        ...(heavy
          ? largeMonacoOverrides(fileName, source.length)
          : { language: monacoLanguage(fileName), wordWrap: 'on' as const })
      })
      editorRef.current = code
      const editor = code
      const reader = valueReaderRef.current
      if (reader) reader.current = () => editor.getValue()
      editor.addCommand(monaco.KeyMod.CtrlCmd | monaco.KeyCode.KeyS, () => {
        onSaveRef.current(editor.getValue())
      })
      editor.addCommand(monaco.KeyMod.Shift | monaco.KeyMod.Alt | monaco.KeyCode.KeyF, () => {
        onFormatRef.current?.()
      })
      const slot = commandsRef.current
      let remembered: monaco.Selection | null = null
      if (slot) {
        slot.current = bindEditorCommands(
          editor,
          () => remembered,
          (next) => {
            remembered = next
          }
        )
      }
      let skip = true
      const subscription = editor.onDidChangeModelContent(() => {
        if (skip) return
        if (largeRef.current) onDirtyRef.current?.()
        else onChangeRef.current(editor.getValue())
      })
      const cursor = editor.onDidChangeCursorPosition((event) => {
        onCursorRef.current?.({ line: event.position.lineNumber, column: event.position.column })
      })
      disposeModelEvents = () => {
        subscription.dispose()
        cursor.dispose()
      }
      onCursorRef.current?.({ line: 1, column: 1 })
      queueMicrotask(() => {
        skip = false
      })
      onMountedRef.current?.()
    })
    return () => {
      cancelled = true
      cancelAnimationFrame(frame)
      disposeModelEvents?.()
      const reader = valueReaderRef.current
      if (reader) reader.current = null
      const slot = commandsRef.current
      if (slot) slot.current = null
      code?.dispose()
      editorRef.current = null
    }
  }, [fileName, large])

  useEffect(() => {
    if (large) return
    const model = editorRef.current?.getModel()
    if (!model || model.getValue() === value) return
    model.setValue(value)
  }, [large, value])

  return (
    <div
      ref={host}
      className="h-full min-h-0 w-full flex-1"
      data-workspace-monaco={fileName}
      data-code-minimap={large ? 'off' : 'on'}
    />
  )
}

function bindEditorCommands(
  editor: EditorApi.IStandaloneCodeEditor,
  readSelection: () => monaco.Selection | null,
  writeSelection: (selection: monaco.Selection | null) => void
): CodeEditorCommands {
  const modelRange = () => {
    const model = editor.getModel()
    const selection = readSelection() ?? editor.getSelection()
    if (!model || !selection || selection.isEmpty()) {
      const position = selection?.getStartPosition() ?? editor.getPosition()
      if (!model || !position) return null
      const line = position.lineNumber
      const last = model.getLineCount()
      const endLine = line < last ? line + 1 : line
      const endColumn = line < last ? 1 : model.getLineMaxColumn(line)
      return new monaco.Selection(line, 1, endLine, endColumn)
    }
    return selection
  }

  return {
    rememberSelection() {
      writeSelection(editor.getSelection())
    },
    cut() {
      const model = editor.getModel()
      const range = modelRange()
      if (!model || !range) return
      const text = model.getValueInRange(range)
      void writeClipboard(text)
      editor.executeEdits('menu', [{ range, text: '', forceMoveMarkers: true }])
      editor.setPosition({ lineNumber: range.startLineNumber, column: range.startColumn })
      editor.focus()
    },
    copy() {
      const model = editor.getModel()
      const range = modelRange()
      if (!model || !range) return
      void writeClipboard(model.getValueInRange(range))
      editor.focus()
    },
    paste() {
      const selection = readSelection() ?? editor.getSelection()
      const position = selection?.getStartPosition() ?? editor.getPosition()
      if (!position) return
      const range =
        selection && !selection.isEmpty()
          ? selection
          : new monaco.Selection(position.lineNumber, position.column, position.lineNumber, position.column)
      void readClipboard().then((text) => {
        if (!text) return
        editor.executeEdits('menu', [{ range, text, forceMoveMarkers: true }])
        editor.focus()
      })
    },
    selectAll() {
      const model = editor.getModel()
      if (!model) return
      editor.setSelection(model.getFullModelRange())
      editor.focus()
    }
  }
}
