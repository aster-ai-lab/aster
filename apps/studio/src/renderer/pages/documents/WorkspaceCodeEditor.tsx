import {
  ContextMenu,
  ContextMenuContent,
  ContextMenuItem,
  ContextMenuItemContent,
  ContextMenuSeparator,
  ContextMenuTrigger
} from '@a3s-lab/ui'
import { Braces, ClipboardPaste, Copy, Scissors, TextSelect } from 'lucide-react'
import {
  useEffect,
  useRef,
  useState,
  useSyncExternalStore,
  type ComponentType,
  type ReactNode,
  type RefObject
} from 'react'
import { useTranslation } from 'react-i18next'

import { commandMod, desktopCapabilities } from '@renderer/host/desktopHost'
import { readAixCatalogue, subscribeAixCatalogue } from '@renderer/services/plugins'
import { toast } from '@renderer/services/toast'

import { formatDocumentSize } from './documentLibrary'
import { FileEditorLoading } from './FileEditorLoading'
import { codeSurface } from './fileOpenBudget'
import type { CodeEditorCommands } from './monacoCodeEditor'
import { monacoLanguage } from './monacoLanguage'
import { WorkspaceNotice } from './WorkspaceFileChrome'

type CodeCursor = { line: number; column: number }

type MonacoEditor = ComponentType<{
  fileName: string
  value: string
  large?: boolean
  readOnly?: boolean
  onChange: (value: string) => void
  onDirty?: () => void
  onSave: (value: string) => void
  onCursor?: (cursor: CodeCursor) => void
  onMounted?: () => void
  onFormat?: () => void
  valueReader?: { current: (() => string) | null }
  commands?: { current: CodeEditorCommands | null }
}>

type TextSpan = { start: number; end: number }

function commandPrefix(): string {
  return commandMod(desktopCapabilities().platform) === 'cmd' ? '⌘' : 'Ctrl+'
}

function formatShortcut(): string {
  return commandMod(desktopCapabilities().platform) === 'cmd' ? '⇧⌥F' : 'Shift+Alt+F'
}

function lineBounds(value: string, offset: number): TextSpan {
  const from = value.lastIndexOf('\n', Math.max(0, offset - 1)) + 1
  const next = value.indexOf('\n', offset)
  return { start: from, end: next === -1 ? value.length : next + 1 }
}

async function writeClipboard(text: string): Promise<void> {
  try {
    await navigator.clipboard?.writeText(text)
  } catch {
    // Cut still removes the text when the clipboard is blocked.
  }
}

async function readClipboard(): Promise<string> {
  try {
    return (await navigator.clipboard?.readText()) ?? ''
  } catch {
    return ''
  }
}

function caretAt(value: string, offset: number): CodeCursor {
  const head = value.slice(0, offset)
  const line = head.split('\n').length
  const column = offset - head.lastIndexOf('\n')
  return { line, column }
}

export function WorkspaceCodeEditor({
  fileName,
  value,
  placeholder,
  readOnly = false,
  onChange,
  onDirty,
  onSave,
  onMounted,
  onFormat,
  valueReader,
  statusExtra
}: {
  fileName: string
  value: string
  placeholder?: string
  readOnly?: boolean
  onChange: (value: string) => void
  onDirty?: () => void
  onSave: (value: string) => void
  onMounted?: () => void
  onFormat?: () => void
  valueReader?: { current: (() => string) | null }
  statusExtra?: ReactNode
}) {
  const { t } = useTranslation()
  const surface = codeSurface(value, import.meta.env.MODE === 'test')
  const plain = surface === 'textarea' || surface === 'too-large'
  const [Monaco, setMonaco] = useState<MonacoEditor | null>(null)
  const [failed, setFailed] = useState(false)
  const [cursor, setCursor] = useState<CodeCursor>({ line: 1, column: 1 })
  const language = monacoLanguage(fileName)
  const valueRef = useRef(value)
  const fieldRef = useRef<HTMLTextAreaElement>(null)
  const commands = useRef<CodeEditorCommands | null>(null)
  const selectionRef = useRef<TextSpan | null>(null)
  const pendingSelection = useRef<(TextSpan & { waitForValue: boolean }) | null>(null)
  const onChangeRef = useRef(onChange)
  const onFormatRef = useRef(onFormat)
  valueRef.current = value
  onChangeRef.current = onChange
  onFormatRef.current = onFormat

  useEffect(() => {
    if (!valueReader || surface === 'monaco' || surface === 'monaco-large') return undefined
    valueReader.current = () => valueRef.current
    return () => {
      valueReader.current = null
    }
  }, [surface, valueReader])

  useEffect(() => {
    const field = fieldRef.current
    const next = pendingSelection.current
    if (!field || !next?.waitForValue) return
    pendingSelection.current = null
    field.focus()
    field.setSelectionRange(next.start, next.end)
  }, [value])

  useEffect(() => {
    if (surface !== 'textarea') return undefined
    onMounted?.()
    commands.current = textareaCommands({
      fieldRef,
      valueRef,
      selectionRef,
      pendingSelection,
      onChangeRef,
      setCursor
    })
    return () => {
      commands.current = null
    }
  }, [onMounted, surface])

  useEffect(() => {
    if (plain) return undefined
    let cancelled = false
    void import('./monacoCodeEditor')
      .then((mod) => {
        if (!cancelled) setMonaco(() => mod.MonacoCodeEditor)
      })
      .catch(() => {
        if (!cancelled) setFailed(true)
      })
    return () => {
      cancelled = true
    }
  }, [plain])

  const status = (
    <div className="flex h-7 shrink-0 items-center justify-between border-t border-[#ececec] px-3 text-[12px] text-[#8a8a8a]">
      <span>{language}</span>
      <span className="ml-auto flex items-center gap-3">
        {statusExtra}
        <span>
          {cursor.line}:{cursor.column}
        </span>
      </span>
    </div>
  )

  const restoreSelection = () => {
    const field = fieldRef.current
    const next = pendingSelection.current
    if (!field || !next || next.waitForValue) return
    pendingSelection.current = null
    field.focus()
    field.setSelectionRange(next.start, next.end)
  }

  if (surface === 'too-large') {
    return (
      <WorkspaceNotice
        title={t('documents.open.too_large.title')}
        description={t('documents.open.too_large.description', { size: formatDocumentSize(value.length) })}
      />
    )
  }

  if (!plain && !failed && Monaco) {
    return (
      <EditorMenu commands={commands} editing={!readOnly} onFormat={onFormat} onClose={restoreSelection}>
        <Monaco
          fileName={fileName}
          value={value}
          large={surface === 'monaco-large'}
          readOnly={readOnly}
          onChange={onChange}
          onDirty={onDirty}
          onSave={onSave}
          onCursor={setCursor}
          onMounted={onMounted}
          onFormat={onFormat}
          valueReader={valueReader}
          commands={commands}
        />
        {status}
      </EditorMenu>
    )
  }

  if (!plain && !failed) {
    return <FileEditorLoading label={t('file_preview.loading')} />
  }

  return (
    <EditorMenu commands={commands} editing={!readOnly} onFormat={onFormat} onClose={restoreSelection}>
      <textarea
        ref={fieldRef}
        aria-label={fileName}
        readOnly={readOnly}
        value={value}
        onChange={(event) => {
          if (!readOnly) onChange(event.target.value)
        }}
        onKeyDown={(event) => {
          if (event.shiftKey && event.altKey && !event.metaKey && !event.ctrlKey && event.key.toLowerCase() === 'f') {
            event.preventDefault()
            onFormatRef.current?.()
            return
          }
          if ((event.metaKey || event.ctrlKey) && !event.altKey && !event.shiftKey && event.key.toLowerCase() === 's') {
            event.preventDefault()
            onSave(event.currentTarget.value)
          }
        }}
        onSelect={(event) => setCursor(caretAt(value, event.currentTarget.selectionStart))}
        placeholder={placeholder}
        spellCheck={language === 'plaintext'}
        data-workspace-code-editor={fileName}
        className="min-h-0 flex-1 resize-none border-0 bg-white px-5 py-4 font-mono text-[13px] leading-6 text-[#171717] outline-none placeholder:text-[#c5c5c5]"
      />
      {status}
    </EditorMenu>
  )
}

function EditorMenu({
  commands,
  editing = true,
  onFormat,
  onClose,
  children
}: {
  commands: RefObject<CodeEditorCommands | null>
  editing?: boolean
  onFormat?: () => void
  onClose: () => void
  children: ReactNode
}) {
  const { t } = useTranslation()
  const mod = commandPrefix()
  const catalogue = useSyncExternalStore(subscribeAixCatalogue, readAixCatalogue, readAixCatalogue)
  const contributed = catalogue.packages.flatMap((item) =>
    (item.menus ?? []).flatMap((menu) => {
      const command = item.commands?.find((entry) => entry.id === menu.command)
      if (!command || (menu.location !== 'editor/context' && menu.location !== 'editor/title')) return []
      return [{ key: `${item.slug}/${menu.location}/${menu.command}`, location: menu.location, title: command.title }]
    })
  )
  return (
    <ContextMenu>
      <ContextMenuTrigger asChild>
        <div className="flex min-h-0 flex-1 flex-col" onContextMenu={() => commands.current?.rememberSelection()}>
          {contributed.some((item) => item.location === 'editor/title') ? (
            <div className="flex justify-end gap-1 border-b border-border px-2 py-1">
              {contributed
                .filter((item) => item.location === 'editor/title')
                .map((item) => (
                  <button
                    key={item.key}
                    type="button"
                    className="text-[12px] text-[#8a8a8a]"
                    onClick={() => toast.info(item.title)}>
                    {item.title}
                  </button>
                ))}
            </div>
          ) : null}
          {children}
        </div>
      </ContextMenuTrigger>
      <ContextMenuContent
        aria-label={t('code.editor.menu')}
        onCloseAutoFocus={(event) => {
          event.preventDefault()
          onClose()
        }}>
        {editing ? (
          <ContextMenuItem onSelect={() => void commands.current?.cut()}>
            <ContextMenuItemContent icon={<Scissors size={16} strokeWidth={1.7} />} shortcut={`${mod}X`}>
              {t('code.editor.cut')}
            </ContextMenuItemContent>
          </ContextMenuItem>
        ) : null}
        <ContextMenuItem onSelect={() => void commands.current?.copy()}>
          <ContextMenuItemContent icon={<Copy size={16} strokeWidth={1.7} />} shortcut={`${mod}C`}>
            {t('code.editor.copy')}
          </ContextMenuItemContent>
        </ContextMenuItem>
        {editing ? (
          <ContextMenuItem onSelect={() => void commands.current?.paste()}>
            <ContextMenuItemContent icon={<ClipboardPaste size={16} strokeWidth={1.7} />} shortcut={`${mod}V`}>
              {t('code.editor.paste')}
            </ContextMenuItemContent>
          </ContextMenuItem>
        ) : null}
        {onFormat ? (
          <>
            <ContextMenuSeparator />
            <ContextMenuItem onSelect={() => onFormat()}>
              <ContextMenuItemContent icon={<Braces size={16} strokeWidth={1.7} />} shortcut={formatShortcut()}>
                {t('code.format_json')}
              </ContextMenuItemContent>
            </ContextMenuItem>
          </>
        ) : null}
        <ContextMenuSeparator />
        {contributed.some((item) => item.location === 'editor/context') ? (
          <>
            {contributed
              .filter((item) => item.location === 'editor/context')
              .map((item) => (
                <ContextMenuItem key={item.key} onSelect={() => toast.info(item.title)}>
                  <ContextMenuItemContent>{item.title}</ContextMenuItemContent>
                </ContextMenuItem>
              ))}
          </>
        ) : null}
        <ContextMenuItem onSelect={() => commands.current?.selectAll()}>
          <ContextMenuItemContent icon={<TextSelect size={16} strokeWidth={1.7} />} shortcut={`${mod}A`}>
            {t('code.editor.select_all')}
          </ContextMenuItemContent>
        </ContextMenuItem>
      </ContextMenuContent>
    </ContextMenu>
  )
}

function textareaCommands({
  fieldRef,
  valueRef,
  selectionRef,
  pendingSelection,
  onChangeRef,
  setCursor
}: {
  fieldRef: RefObject<HTMLTextAreaElement | null>
  valueRef: RefObject<string>
  selectionRef: RefObject<TextSpan | null>
  pendingSelection: RefObject<(TextSpan & { waitForValue: boolean }) | null>
  onChangeRef: RefObject<(value: string) => void>
  setCursor: (cursor: CodeCursor) => void
}): CodeEditorCommands {
  const chosen = () => {
    const value = valueRef.current
    const snap = selectionRef.current ?? { start: value.length, end: value.length }
    const start = Math.min(snap.start, snap.end)
    const end = Math.max(snap.start, snap.end)
    if (start !== end) return { start, end, text: value.slice(start, end), line: false }
    const line = lineBounds(value, start)
    return { start: line.start, end: line.end, text: value.slice(line.start, line.end), line: true }
  }

  const replace = (start: number, end: number, text: string) => {
    const value = valueRef.current
    const next = `${value.slice(0, start)}${text}${value.slice(end)}`
    const caret = start + text.length
    pendingSelection.current = { start: caret, end: caret, waitForValue: true }
    setCursor(caretAt(next, caret))
    onChangeRef.current(next)
  }

  return {
    rememberSelection() {
      const field = fieldRef.current
      if (!field) return
      selectionRef.current = { start: field.selectionStart, end: field.selectionEnd }
    },
    cut() {
      const span = chosen()
      void writeClipboard(span.text)
      replace(span.start, span.end, '')
    },
    copy() {
      const span = chosen()
      void writeClipboard(span.text)
      pendingSelection.current = { start: span.start, end: span.line ? span.start : span.end, waitForValue: false }
    },
    paste() {
      const value = valueRef.current
      const snap = selectionRef.current ?? { start: value.length, end: value.length }
      const start = Math.min(snap.start, snap.end)
      const end = Math.max(snap.start, snap.end)
      void readClipboard().then((text) => {
        if (!text) return
        replace(start, end, text)
      })
    },
    selectAll() {
      const length = valueRef.current.length
      pendingSelection.current = { start: 0, end: length, waitForValue: false }
      setCursor(caretAt(valueRef.current, length))
    }
  }
}
