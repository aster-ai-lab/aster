import { mergeAttributes, Node, type Editor } from '@tiptap/core'
import { EditorContent, useEditor } from '@tiptap/react'
import { StarterKit } from '@tiptap/starter-kit'
import {
  Bold,
  Heading2,
  Heading3,
  Italic,
  Link2,
  List,
  ListOrdered,
  Redo2,
  RemoveFormatting,
  Strikethrough,
  TextQuote,
  Underline,
  Undo2,
  type LucideIcon
} from 'lucide-react'
import { useEffect, useImperativeHandle, useMemo, useRef, useState, type ReactNode, type RefObject } from 'react'
import { useTranslation } from 'react-i18next'

import { cn } from '@renderer/utils/style'

import { orgFieldClassName, quietButtonClassName } from '../org/orgUi'
import { chordKeys, chordLabel } from './mailCompose'
import { editorSeed, isMailHtml, normalizeMailHref, sanitizeMailHtml } from './mailDocument'
import { SIGNATURE_TAG } from './mailSignatures'

import './mailRich.css'

const MailSignature = Node.create({
  name: 'mailSignature',
  group: 'block',
  content: 'block+',
  defining: true,
  isolating: true,
  parseHTML: () => [{ tag: SIGNATURE_TAG }],
  renderHTML: ({ HTMLAttributes }) => [SIGNATURE_TAG, mergeAttributes(HTMLAttributes), 0]
})

function inside(editor: Editor, pos: number, typeName: string): boolean {
  const resolved = editor.state.doc.resolve(pos)
  for (let depth = resolved.depth; depth > 0; depth -= 1) {
    if (resolved.node(depth).type.name === typeName) return true
  }
  return false
}

function ownSignatureRange(editor: Editor): { from: number; to: number } | null {
  let found: { from: number; to: number } | null = null
  editor.state.doc.descendants((node, pos) => {
    if (found) return false
    if (node.type.name !== 'mailSignature') return
    if (inside(editor, pos, 'blockquote')) return
    found = { from: pos, to: pos + node.nodeSize }
    return false
  })
  return found
}

function signatureInsertAt(editor: Editor): number | { from: number; to: number } {
  const existing = ownSignatureRange(editor)
  if (existing) return existing
  let at = editor.state.doc.content.size
  editor.state.doc.descendants((node, pos) => {
    if (node.type.name !== 'blockquote' || inside(editor, pos, 'blockquote')) return
    at = pos
    return false
  })
  return at
}

export const mailRichClassName =
  'text-[15px] leading-6 text-[#171717] [&_a]:text-[#171717] [&_a]:underline [&_blockquote]:my-3 [&_blockquote]:border-l-2 [&_blockquote]:border-[#e5e5e5] [&_blockquote]:pl-3 [&_blockquote]:text-[#3f3f3f] [&_h2]:my-2 [&_h2]:text-[18px] [&_h2]:leading-7 [&_h2]:font-medium [&_h3]:my-2 [&_h3]:text-[16px] [&_h3]:leading-6 [&_h3]:font-medium [&_li_p]:my-0 [&_mail-signature]:mt-4 [&_mail-signature]:block [&_mail-signature]:text-[#3f3f3f] [&_ol]:my-2 [&_ol]:list-decimal [&_ol]:pl-5 [&_p]:my-0 [&_p+p]:mt-3 [&_ul]:my-2 [&_ul]:list-disc [&_ul]:pl-5'

const linkFieldClassName = cn(orgFieldClassName, 'outline-none focus-visible:ring-2 focus-visible:ring-black/10')

export type MailEditorHandle = {
  focus: (place: 'start' | 'end') => void
  appendSignature: (html: string) => void
}

type FormatControl = {
  label: string
  icon: LucideIcon
  shortcut?: string
  hint?: string
  pressed: boolean
  toggle?: boolean
  disabled?: boolean
  run: () => void
}

function control(label: string, icon: LucideIcon, pressed: boolean, run: () => void, hint?: string): FormatControl {
  return { label, icon, hint, shortcut: hint ? chordKeys(hint) : undefined, pressed, toggle: true, run }
}

export function MailEditor({
  seed,
  autoFocus,
  place,
  editorRef,
  tools,
  onChange
}: {
  seed: string
  autoFocus: boolean
  place: 'start' | 'end'
  editorRef: RefObject<MailEditorHandle | null>
  tools?: ReactNode
  onChange: (body: string) => void
}) {
  const { t } = useTranslation()
  const onChangeRef = useRef(onChange)
  onChangeRef.current = onChange
  const openLinkRef = useRef<() => void>(() => {})
  const linkInputRef = useRef<HTMLInputElement>(null)
  const selectionRef = useRef<{ from: number; to: number } | null>(null)
  const [linkOpen, setLinkOpen] = useState(false)
  const [linkUrl, setLinkUrl] = useState('')
  const [linkError, setLinkError] = useState('')
  const [editingLink, setEditingLink] = useState(false)
  const label = t('mail.compose.body')

  const extensions = useMemo(
    () => [
      MailSignature,
      StarterKit.configure({
        heading: { levels: [2, 3] },
        code: false,
        codeBlock: false,
        horizontalRule: false,
        trailingNode: false,
        link: {
          openOnClick: false,
          autolink: true,
          linkOnPaste: true,
          defaultProtocol: 'https',
          HTMLAttributes: { rel: 'noopener noreferrer', target: '_blank' },
          isAllowedUri: (url) => /^(https?:\/\/|mailto:)/i.test(url.trim())
        }
      })
    ],
    []
  )

  const editorProps = useMemo(
    () => ({
      attributes: {
        role: 'textbox',
        'aria-multiline': 'true',
        'aria-label': label,
        spellcheck: 'true',
        class: 'min-h-full outline-none'
      },
      handleKeyDown: (_view: unknown, event: KeyboardEvent) => {
        if ((event.metaKey || event.ctrlKey) && !event.altKey && event.key.toLowerCase() === 'k') {
          event.preventDefault()
          openLinkRef.current()
          return true
        }
        return false
      }
    }),
    [label]
  )

  const editor = useEditor(
    {
      extensions,
      editorProps,
      content: editorSeed(seed) || '<p></p>',
      shouldRerenderOnTransaction: true,
      onUpdate: ({ editor: current }) => {
        onChangeRef.current(current.isEmpty ? '' : current.getHTML())
      }
    },
    [extensions]
  )

  useImperativeHandle(
    editorRef,
    () => ({
      focus: (next) => editor?.commands.focus(next),
      appendSignature: (html) => {
        if (!editor || !html) return
        editor.chain().insertContentAt(signatureInsertAt(editor), html).run()
      }
    }),
    [editor]
  )

  useEffect(() => {
    if (!editor || !autoFocus) return
    editor.commands.focus(place)
  }, [autoFocus, editor, place])

  useEffect(() => {
    if (linkOpen) linkInputRef.current?.focus()
  }, [linkOpen])

  const openLink = () => {
    if (!editor) return
    editor.chain().focus().extendMarkRange('link').run()
    const { from, to } = editor.state.selection
    selectionRef.current = { from, to }
    setEditingLink(editor.isActive('link'))
    setLinkUrl(String(editor.getAttributes('link').href ?? ''))
    setLinkError('')
    setLinkOpen(true)
  }
  openLinkRef.current = openLink

  const applyLink = () => {
    if (!editor || !selectionRef.current) return
    const href = normalizeMailHref(linkUrl)
    if (!href) {
      setLinkError(t('mail.compose.link_invalid'))
      return
    }
    const { from, to } = selectionRef.current
    const chain = editor.chain().focus().setTextSelection({ from, to })
    if (from === to) {
      chain.insertContent({ type: 'text', text: href, marks: [{ type: 'link', attrs: { href } }] }).run()
    } else {
      chain.setLink({ href }).run()
    }
    setLinkOpen(false)
    setLinkError('')
  }

  const removeLink = () => {
    if (!editor || !selectionRef.current) return
    const { from, to } = selectionRef.current
    editor.chain().focus().setTextSelection({ from, to }).extendMarkRange('link').unsetLink().run()
    setLinkOpen(false)
  }

  const controls: FormatControl[][] = editor
    ? [
        [
          control(
            t('mail.compose.heading2'),
            Heading2,
            editor.isActive('heading', { level: 2 }),
            () => editor.chain().focus().toggleHeading({ level: 2 }).run(),
            'Mod+Alt+2'
          ),
          control(
            t('mail.compose.heading3'),
            Heading3,
            editor.isActive('heading', { level: 3 }),
            () => editor.chain().focus().toggleHeading({ level: 3 }).run(),
            'Mod+Alt+3'
          )
        ],
        [
          control(
            t('mail.compose.bold'),
            Bold,
            editor.isActive('bold'),
            () => editor.chain().focus().toggleBold().run(),
            'Mod+B'
          ),
          control(
            t('mail.compose.italic'),
            Italic,
            editor.isActive('italic'),
            () => editor.chain().focus().toggleItalic().run(),
            'Mod+I'
          ),
          control(
            t('mail.compose.underline'),
            Underline,
            editor.isActive('underline'),
            () => editor.chain().focus().toggleUnderline().run(),
            'Mod+U'
          ),
          control(
            t('mail.compose.strike'),
            Strikethrough,
            editor.isActive('strike'),
            () => editor.chain().focus().toggleStrike().run(),
            'Mod+Shift+S'
          )
        ],
        [
          control(
            t('mail.compose.bullet'),
            List,
            editor.isActive('bulletList'),
            () => editor.chain().focus().toggleBulletList().run(),
            'Mod+Shift+8'
          ),
          control(
            t('mail.compose.numbered'),
            ListOrdered,
            editor.isActive('orderedList'),
            () => editor.chain().focus().toggleOrderedList().run(),
            'Mod+Shift+7'
          ),
          control(
            t('mail.compose.quote'),
            TextQuote,
            editor.isActive('blockquote'),
            () => editor.chain().focus().toggleBlockquote().run(),
            'Mod+Shift+B'
          )
        ],
        [
          control(t('mail.compose.link'), Link2, editor.isActive('link'), openLink, 'Mod+K'),
          control(t('mail.compose.clear'), RemoveFormatting, false, () =>
            editor.chain().focus().unsetAllMarks().clearNodes().run()
          )
        ],
        [
          {
            label: t('mail.compose.undo'),
            icon: Undo2,
            hint: 'Mod+Z',
            shortcut: chordKeys('Mod+Z'),
            pressed: false,
            disabled: !editor.can().undo(),
            run: () => editor.chain().focus().undo().run()
          },
          {
            label: t('mail.compose.redo'),
            icon: Redo2,
            hint: 'Mod+Shift+Z',
            shortcut: chordKeys('Mod+Shift+Z'),
            pressed: false,
            disabled: !editor.can().redo(),
            run: () => editor.chain().focus().redo().run()
          }
        ]
      ]
    : []

  return (
    <div data-ui="mail.editor" className="flex min-h-[180px] flex-1 flex-col border-t border-[#efefef]">
      <div
        data-ui="mail.toolbar"
        className="relative z-20 flex shrink-0 flex-wrap items-center gap-0.5 px-4 py-1.5"
        role="toolbar"
        aria-label={label}>
        {controls.map((group, index) => (
          <div key={group[0]?.label ?? index} className="flex items-center gap-0.5">
            {index > 0 ? <span aria-hidden className="mx-1 h-4 w-px bg-[#efefef]" /> : null}
            {group.map((item) => {
              const Icon = item.icon
              return (
                <button
                  key={item.label}
                  type="button"
                  title={item.hint ? `${item.label} ${chordLabel(item.hint)}` : item.label}
                  aria-label={item.label}
                  aria-pressed={item.toggle ? item.pressed : undefined}
                  aria-keyshortcuts={item.shortcut}
                  disabled={item.disabled}
                  className={cn(
                    'inline-flex size-8 items-center justify-center rounded-lg text-[#3f3f3f] hover:bg-[#f2f3f5] hover:text-[#171717] focus-visible:ring-2 focus-visible:ring-black/10 focus-visible:outline-none disabled:pointer-events-none disabled:text-[#8a8a8a] disabled:hover:bg-transparent',
                    item.pressed && 'bg-[#f2f3f5] text-[#171717]'
                  )}
                  onMouseDown={(event) => event.preventDefault()}
                  onClick={item.run}>
                  <Icon size={16} strokeWidth={1.7} />
                </button>
              )
            })}
          </div>
        ))}
        {tools ? (
          <div className="flex items-center gap-0.5">
            <span aria-hidden className="mx-1 h-4 w-px bg-[#efefef]" />
            {tools}
          </div>
        ) : null}
      </div>
      {linkOpen ? (
        <div
          data-ui="mail.link"
          className="shrink-0 border-t border-[#efefef] px-6 py-2"
          onKeyDown={(event) => {
            if (event.key !== 'Escape') return
            event.preventDefault()
            event.stopPropagation()
            setLinkOpen(false)
            editor?.commands.focus()
          }}>
          <div className="flex items-center gap-2">
            <input
              ref={linkInputRef}
              aria-label={t('mail.compose.link_url')}
              aria-describedby={linkError ? 'mail-link-error' : undefined}
              value={linkUrl}
              placeholder={t('mail.compose.link_hint')}
              maxLength={2000}
              className={cn(linkFieldClassName, 'min-w-0 flex-1')}
              onChange={(event) => {
                setLinkUrl(event.target.value)
                setLinkError('')
              }}
              onKeyDown={(event) => {
                if (event.key !== 'Enter') return
                event.preventDefault()
                event.stopPropagation()
                applyLink()
              }}
            />
            <button type="button" className={quietButtonClassName(36)} onClick={applyLink}>
              {t('mail.compose.link_apply')}
            </button>
            {editingLink ? (
              <button
                type="button"
                className="h-9 shrink-0 rounded-lg px-2 text-[13px] text-[#8a8a8a] hover:text-[#171717]"
                onClick={removeLink}>
                {t('mail.compose.link_remove')}
              </button>
            ) : null}
          </div>
          {linkError ? (
            <p id="mail-link-error" role="alert" className="m-0 pt-2 text-[12px] leading-4 text-[#b42318]">
              {linkError}
            </p>
          ) : null}
        </div>
      ) : null}
      <div className="relative min-h-0 flex-1 overflow-y-auto px-6 py-3">
        {editor?.isEmpty ? (
          <span className="pointer-events-none absolute top-3 left-6 text-[15px] leading-6 text-[#c5c5c5]">
            {t('mail.compose.placeholder')}
          </span>
        ) : null}
        <EditorContent
          editor={editor}
          className={cn('min-h-full', mailRichClassName, '[&_.ProseMirror]:min-h-full [&_.ProseMirror]:outline-none')}
        />
      </div>
    </div>
  )
}

export function MailMessageBody({ body }: { body: string }) {
  if (!isMailHtml(body)) {
    return (
      <div
        data-ui="mail.body"
        className="min-h-0 flex-1 overflow-y-auto px-4 py-3 text-[15px] leading-6 whitespace-pre-wrap select-text">
        {body}
      </div>
    )
  }
  return (
    <div
      data-ui="mail.body"
      className={cn('min-h-0 flex-1 overflow-y-auto px-4 py-3 select-text', mailRichClassName)}
      dangerouslySetInnerHTML={{ __html: sanitizeMailHtml(body) }}
    />
  )
}
