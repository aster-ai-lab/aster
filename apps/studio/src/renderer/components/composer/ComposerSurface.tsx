import { CirclePause } from 'lucide-react'
import {
  type ComponentType,
  type DragEvent as ReactDragEvent,
  type KeyboardEvent as ReactKeyboardEvent,
  type ReactNode,
  useCallback,
  useEffect,
  useId,
  useLayoutEffect,
  useMemo,
  useRef,
  useState
} from 'react'
import { useTranslation } from 'react-i18next'

import { usePreference } from '@data/hooks/usePreference'
import { loggerService } from '@logger'
import NarrowLayout from '@renderer/components/chat/layout/NarrowLayout'
import { PluginComposerActions } from '@renderer/components/plugins/PluginComposerActions'
import { usePluginContributions } from '@renderer/components/plugins/PluginScopeProvider'
import SendMessageButton from '@renderer/components/SendMessageButton'
import { toast } from '@renderer/services/toast'
import type { PluginComposerDraft } from '@renderer/types/workbenchPlugin'
import { getAppEdition } from '@renderer/utils/appEdition'
import { matchesComposerShortcut, resolveNewlineShortcut, resolveSendShortcut } from '@renderer/utils/input'
import { cn } from '@renderer/utils/style'

import { ComposerFileDropOverlay, dragEventCarriesFiles } from './ComposerFileDropOverlay'
import { ComposerFocusShortcut } from './ComposerFocusShortcut'
import { resolveComposerEditorMinHeight } from './composerSizing'
import type { ComposerDeferredIntent, ComposerSurfaceActions, ComposerSurfaceProps } from './ComposerSurfaceRuntime'
import type { ComposerSerializedDraft, ComposerSerializedToken } from './tokens'

const COMPOSER_SIDE_PADDING_PX = 24

const logger = loggerService.withContext('ComposerSurface')

export type {
  ComposerDeferredIntent,
  ComposerSurfaceActions,
  ComposerSurfaceEditingState,
  ComposerSurfaceProps
} from './ComposerSurfaceRuntime'

let runtimePromise: Promise<{ default: ComponentType<ComposerSurfaceProps> }> | undefined

function loadRuntime() {
  runtimePromise ??= import('./ComposerSurfaceRuntime').catch((error) => {
    // Drop the rejected promise so the next interaction retries instead of replaying the failure.
    runtimePromise = undefined
    logger.error('Failed to load composer runtime', error as Error)
    throw error
  })
  return runtimePromise
}

type FallbackCaret = 'start' | 'end' | 'all' | number

/** `true`, `false`, `null`, and omitted all mean the end, matching the runtime focus command. */
function requestedFallbackCaret(
  position: 'start' | 'end' | 'all' | number | boolean | null | undefined
): FallbackCaret {
  if (typeof position === 'number' || position === 'start' || position === 'all') return position
  return 'end'
}

function fallbackCaretRange(position: FallbackCaret, length: number): { start: number; end: number } {
  if (position === 'start') return { start: 0, end: 0 }
  if (position === 'all') return { start: 0, end: length }
  if (typeof position === 'number') {
    const next = Math.min(Math.max(position, 0), length)
    return { start: next, end: next }
  }
  return { start: length, end: length }
}

/** Clipboard/drag payloads are only readable during their own event, so keep an owned copy. */
function cloneTransfer(source: DataTransfer | null): DataTransfer | undefined {
  if (!source) return undefined
  const clone = new DataTransfer()
  for (const type of source.types) {
    if (type !== 'Files') clone.setData(type, source.getData(type))
  }
  for (const file of source.files) clone.items.add(file)
  return clone
}

function DeferredComposerSurface(props: ComposerSurfaceProps) {
  const { t } = useTranslation()
  const showAiDisclaimer = props.showAiDisclaimer && getAppEdition() === 'cn'
  const textareaRef = useRef<HTMLTextAreaElement>(null)
  const selectionRef = useRef({ start: props.text.length, end: props.text.length })
  const intentRef = useRef<ComposerDeferredIntent>({})
  // A fill and focus('end') share one turn, before React commits the textarea value.
  // The request stays until that committed value is in the DOM, then it is dropped so
  // a later keystroke keeps the caret where the user put it.
  const pendingCaretRef = useRef<FallbackCaret | null>(null)
  const caretRequestRef = useRef(0)
  const caretFrameRef = useRef(0)
  const textValueRef = useRef(props.text)
  textValueRef.current = props.text
  const [preferredSendMessageShortcut] = usePreference('chat.input.send_message_shortcut')
  const sendMessageShortcut = props.sendMessageShortcut ?? resolveSendShortcut(preferredSendMessageShortcut)
  const [preferredNewlineShortcut] = usePreference('chat.input.newline_shortcut')
  const newlineShortcut = props.newlineShortcut ?? resolveNewlineShortcut(preferredNewlineShortcut, sendMessageShortcut)
  const [Runtime, setRuntime] = useState<ComponentType<ComposerSurfaceProps>>()
  const [runtimeReady, setRuntimeReady] = useState(false)
  const [isComposing, setIsComposing] = useState(false)
  const [fileDragDepth, setFileDragDepth] = useState(0)
  const sendBlockedReasonRef = useRef(props.sendBlockedReason)

  useEffect(() => {
    sendBlockedReasonRef.current = props.sendBlockedReason
  }, [props.sendBlockedReason])

  const showBlockedSendReason = useCallback(() => {
    if (sendBlockedReasonRef.current) {
      toast.error(sendBlockedReasonRef.current)
    }
  }, [])

  const requestRuntime = useCallback(() => {
    void loadRuntime()
      .then((module) => {
        setRuntime(() => module.default)
        setRuntimeReady(true)
      })
      .catch(() => {
        // Already logged; the fallback stays usable and a later interaction retries.
      })
  }, [])

  const getFallbackDraft = useCallback(
    (): ComposerSerializedDraft => ({
      text: props.text,
      tokens: props.draftTokens?.length
        ? [...props.draftTokens]
        : props.tokens.map((token, index) => ({ ...token, index, textOffset: props.text.length }))
    }),
    [props.draftTokens, props.text, props.tokens]
  )

  // The fallback cannot rebase token offsets, render the editing header, grow beyond its fixed
  // two-line box, or represent structural variants, so hand those states to the runtime instead.
  const needsRuntime =
    Boolean(props.editingState) ||
    Boolean(props.draftTokens?.length) ||
    props.tokens.length > 0 ||
    props.text.trim().length > 0 ||
    Boolean(props.compactWhenSingleLine) ||
    props.isExpanded
  useEffect(() => {
    if (needsRuntime) requestRuntime()
  }, [needsRuntime, requestRuntime])

  // Swap while the app is idle rather than under the user's first keystroke: a swap that lands
  // between a keydown and its character insertion drops that character, and no hand-off inside
  // the runtime can recover it. Idle work stays off the first-paint path this fallback protects.
  useEffect(() => {
    if (Runtime || !window.requestIdleCallback) return
    const idleId = window.requestIdleCallback(() => requestRuntime())
    return () => window.cancelIdleCallback(idleId)
  }, [Runtime, requestRuntime])

  useEffect(() => {
    if (Runtime || !props.onActionsChange) return

    const updateText = (updater: string | ((previous: string) => string)) => {
      props.onTextChange(typeof updater === 'function' ? updater(props.text) : updater)
    }
    const updateTokens = (tokens: readonly ComposerSerializedToken[]) => props.onTokensChange(tokens)

    const placeCaret = (position: FallbackCaret, input: HTMLTextAreaElement) => {
      const range = fallbackCaretRange(position, input.value.length)
      input.focus()
      intentRef.current.hadFocus = true
      input.setSelectionRange(range.start, range.end)
      selectionRef.current = range
    }

    const actions: ComposerSurfaceActions = {
      focus: (position) => {
        const requested = requestedFallbackCaret(position)
        pendingCaretRef.current = requested
        const input = textareaRef.current
        // Equality with props.text here is the pre-commit value. Applying it is only
        // safe when this focus did not follow a fill; the layout effect re-applies
        // once the new value is committed and is what the runtime handoff reads.
        if (input && input.value === textValueRef.current) placeCaret(requested, input)
        else if (input) {
          input.focus()
          intentRef.current.hadFocus = true
        }
        if (caretFrameRef.current) window.cancelAnimationFrame(caretFrameRef.current)
        const token = ++caretRequestRef.current
        caretFrameRef.current = window.requestAnimationFrame(() => {
          if (caretRequestRef.current !== token || pendingCaretRef.current == null) return
          const live = textareaRef.current
          if (!live || live.value !== textValueRef.current) return
          placeCaret(pendingCaretRef.current, live)
          pendingCaretRef.current = null
        })
      },
      onTextChange: updateText,
      replaceDraft: (draft) => {
        selectionRef.current = { start: draft.text.length, end: draft.text.length }
        props.onTextChange(draft.text)
        updateTokens(draft.tokens)
      },
      toggleExpanded: (expanded) => props.onExpandedChange(expanded ?? !props.isExpanded),
      removeToken: (tokenId) => updateTokens((props.draftTokens ?? []).filter((token) => token.id !== tokenId)),
      // A token needs its prompt text woven into the document at the caret, which only the rich
      // editor can do; the whole range travels along so the runtime still replaces a selection.
      insertToken: (token, updateOnly, placement) => {
        const input = textareaRef.current
        const length = props.text.length
        intentRef.current.insertToken = {
          token,
          updateOnly,
          selection:
            placement === 'end'
              ? { start: length, end: length }
              : {
                  start: input?.selectionStart ?? length,
                  end: input?.selectionEnd ?? length
                }
        }
        requestRuntime()
      },
      getDraft: getFallbackDraft
    }

    props.onActionsChange(actions)
  }, [Runtime, getFallbackDraft, props, requestRuntime])

  useLayoutEffect(() => {
    const pending = pendingCaretRef.current
    if (pending == null) return
    const input = textareaRef.current
    if (!input) {
      selectionRef.current = fallbackCaretRange(pending, props.text.length)
      intentRef.current.hadFocus = true
      pendingCaretRef.current = null
      return
    }
    if (input.value !== props.text) return
    const range = fallbackCaretRange(pending, input.value.length)
    input.focus()
    intentRef.current.hadFocus = true
    input.setSelectionRange(range.start, range.end)
    selectionRef.current = range
    pendingCaretRef.current = null
  }, [props.text])

  useEffect(
    () => () => {
      if (caretFrameRef.current) window.cancelAnimationFrame(caretFrameRef.current)
    },
    []
  )

  if (Runtime && runtimeReady && !isComposing) {
    return (
      <Runtime
        {...props}
        showAiDisclaimer={showAiDisclaimer}
        initialTextSelection={selectionRef.current}
        deferredIntent={intentRef.current}
      />
    )
  }

  const updateSelection = () => {
    const input = textareaRef.current
    if (input) selectionRef.current = { start: input.selectionStart, end: input.selectionEnd }
  }

  const insertFallbackNewline = (input: HTMLTextAreaElement) => {
    input.setRangeText('\n', input.selectionStart, input.selectionEnd, 'end')
    selectionRef.current = { start: input.selectionStart, end: input.selectionEnd }
    props.onTextChange(input.value)
  }

  const captureTransfer = (kind: 'paste' | 'drop', data: DataTransfer | null) => {
    if (props.editable === false) return
    const transfer = cloneTransfer(data)
    if (transfer) intentRef.current.transfer = { kind, data: transfer }
    requestRuntime()
  }

  const navigateInputHistory = (event: ReactKeyboardEvent<HTMLTextAreaElement>) => {
    if (!props.onInputHistoryNavigate) return false
    if (event.key !== 'ArrowUp' && event.key !== 'ArrowDown') return false
    if (event.ctrlKey || event.metaKey || event.altKey || event.shiftKey || event.nativeEvent.isComposing) return false

    const input = event.currentTarget
    const isAllSelected =
      input.value.length > 0 && input.selectionStart === 0 && input.selectionEnd === input.value.length
    const isAtBoundary =
      event.key === 'ArrowUp' ? input.selectionStart === input.value.length : input.selectionEnd === input.value.length
    if (!(props.text.trim().length === 0 || isAllSelected || isAtBoundary)) return false

    return props.onInputHistoryNavigate(event.key === 'ArrowUp' ? 'up' : 'down')
  }

  // Keeps panel-backed toolbar controls (Skills, the "+" menu) rendered and remembers the panel the
  // user asked for; the runtime opens it as soon as it mounts.
  const pendingPanelControl = {
    available: props.quickPanelEnabled,
    open: (options?: { launcherId?: string; searchText?: string }) => {
      intentRef.current.openPanel = options ?? {}
      requestRuntime()
    }
  }
  const leftControls = props.renderLeftControls?.(undefined, pendingPanelControl)
  const belowControls = props.renderBelowControls?.(undefined, pendingPanelControl)
  const sendAccessoryElement = typeof props.sendAccessory === 'function' ? props.sendAccessory() : props.sendAccessory
  const editorMinHeight = resolveComposerEditorMinHeight(props.fontSize, props.editorMinHeight)
  let sendAction: ReactNode = null
  if (!props.hideSend && props.isLoading && props.sendDisabled) {
    sendAction = (
      <button
        data-ui="chat.composer.action.pause"
        type="button"
        className="flex size-7.5 items-center justify-center rounded-full text-error hover:bg-accent"
        aria-label={t('chat.input.pause')}
        onClick={() => void props.onPause()}>
        <CirclePause size={20} />
      </button>
    )
  } else if (!props.hideSend) {
    sendAction = (
      <SendMessageButton
        disabled={props.sendDisabled}
        sendMessage={() => void props.onSendDraft(getFallbackDraft())}
        onDisabledClick={showBlockedSendReason}
      />
    )
  }
  const inputbarElement = (
    <div
      id="inputbar"
      data-ui="chat.composer"
      data-composer-inputbar=""
      data-composer-presentation="regular"
      className={cn(
        'inputbar-container relative rounded-[20px] border-[0.5px] border-border bg-card pt-2 shadow-sm',
        belowControls ? 'mb-0.5' : 'mb-3'
      )}
      data-drag-over={fileDragDepth > 0 ? 'true' : undefined}>
      {props.topContent}
      <div className={props.leadingContent ? 'flex items-start' : 'contents'}>
        {props.leadingContent ? <div className="shrink-0 pt-1.5 pl-3.5">{props.leadingContent}</div> : null}
        <div className="group/composer-editor relative flex min-w-0 flex-1">
          <textarea
            ref={textareaRef}
            aria-label={props.placeholder}
            value={props.text}
            placeholder={props.placeholder}
            rows={1}
            readOnly={props.editable === false}
            spellCheck={props.enableSpellCheck}
            data-ui="part:composer-input"
            className="box-border block w-full min-w-0 flex-1 resize-none overflow-auto bg-transparent text-foreground outline-none"
            style={{
              height: editorMinHeight,
              minHeight: editorMinHeight,
              padding: '6px 15px 0',
              fontSize: props.fontSize,
              lineHeight: 1.4
            }}
            onChange={(event) => {
              updateSelection()
              props.onTextChange(event.currentTarget.value)
              requestRuntime()
            }}
            onFocus={() => {
              intentRef.current.hadFocus = true
              props.onFocus?.()
              // Start the rich runtime on focus, not on the first key: with a warm chunk the swap
              // would otherwise commit before the keystroke's input event, dropping the character.
              requestRuntime()
            }}
            onSelect={updateSelection}
            onPaste={(event) => {
              // Native insertion would keep only the plain text and drop files, HTML and token
              // fragments, so hand the whole payload to the runtime instead.
              event.preventDefault()
              captureTransfer('paste', event.clipboardData)
            }}
            onCompositionStart={() => setIsComposing(true)}
            onCompositionEnd={(event) => {
              // Some IMEs only write the committed characters on compositionend, and the textarea is
              // about to unmount, so read the final value here rather than waiting for `change`.
              const input = event.currentTarget
              selectionRef.current = { start: input.selectionStart, end: input.selectionEnd }
              if (props.editable !== false && input.value !== props.text) props.onTextChange(input.value)
              setIsComposing(false)
            }}
            onKeyDown={(event) => {
              if (props.editable === false) return
              if (isComposing || event.nativeEvent.isComposing || event.nativeEvent.keyCode === 229) return
              requestRuntime()
              if (navigateInputHistory(event)) {
                event.preventDefault()
                return
              }
              // Same priority order as the runtime surface, so the two never drift: steer wins over
              // send, and every other Enter combination is swallowed rather than inserting a break.
              const isEnterPressed = event.key === 'Enter' || event.key === 'NumpadEnter'
              if (!isEnterPressed) return

              event.preventDefault()

              const isSteerPressed = !!props.steerShortcut && matchesComposerShortcut(event, props.steerShortcut)
              if (isSteerPressed || matchesComposerShortcut(event, sendMessageShortcut)) {
                // Holding the key must not send twice; holding the newline key still repeats.
                if (event.repeat) return
                if (props.sendDisabled) {
                  showBlockedSendReason()
                } else if (isSteerPressed) {
                  void props.onSendDraft(getFallbackDraft(), { steer: true })
                } else {
                  void props.onSendDraft(getFallbackDraft())
                }
                return
              }

              if (matchesComposerShortcut(event, newlineShortcut)) insertFallbackNewline(event.currentTarget)
            }}
          />
          <ComposerFocusShortcut focus={() => textareaRef.current?.focus()} editable={props.editable} />
        </div>
      </div>
      <div
        data-ui="part:composer-actions"
        data-composer-toolbar=""
        className="relative z-2 flex min-h-10 shrink-0 flex-row items-center justify-between gap-4 px-2 py-1.25">
        <div className="flex min-w-0 flex-1 flex-wrap items-center">
          {leftControls}
          {props.dockContent}
        </div>
        <div className="flex flex-row items-center gap-1.5">
          {sendAccessoryElement}
          {sendAction}
        </div>
      </div>
      <ComposerFileDropOverlay active={fileDragDepth > 0} />
    </div>
  )

  const handleDragEnter = props.enableDragDrop
    ? (event: ReactDragEvent<HTMLDivElement>) => {
        if (!dragEventCarriesFiles(event)) return
        event.preventDefault()
        setFileDragDepth((depth) => depth + 1)
      }
    : undefined
  const handleDragOver = props.enableDragDrop
    ? (event: ReactDragEvent<HTMLDivElement>) => {
        event.preventDefault()
        if (event.dataTransfer) event.dataTransfer.dropEffect = 'copy'
        requestRuntime()
        if (!dragEventCarriesFiles(event)) return
        setFileDragDepth((depth) => (depth > 0 ? depth : 1))
      }
    : undefined
  const handleDragLeave = props.enableDragDrop
    ? (event: ReactDragEvent<HTMLDivElement>) => {
        if (!dragEventCarriesFiles(event)) return
        setFileDragDepth((depth) => Math.max(0, depth - 1))
      }
    : undefined
  const handleDrop = props.enableDragDrop
    ? (event: ReactDragEvent<HTMLDivElement>) => {
        event.preventDefault()
        setFileDragDepth(0)
        captureTransfer('drop', event.dataTransfer)
      }
    : undefined

  return (
    <NarrowLayout
      narrowMode={props.narrowMode}
      withSidePadding
      className="pointer-events-auto"
      style={{
        width: '100%',
        ...(props.railGutterPx != null
          ? {
              paddingLeft: COMPOSER_SIDE_PADDING_PX + props.railGutterPx,
              paddingRight: COMPOSER_SIDE_PADDING_PX + props.railGutterPx
            }
          : {})
      }}>
      <div className="w-full">
        <div
          className="inputbar relative z-2 flex flex-col pt-0"
          onDragEnter={handleDragEnter}
          onDragLeave={handleDragLeave}
          onDragOver={handleDragOver}
          onDrop={handleDrop}>
          {belowControls ? (
            <div className="mb-6 rounded-[20px] bg-muted/45 pb-1.5 dark:bg-muted/25">
              {props.queueContent}
              <div className="relative">{inputbarElement}</div>
              <div className="min-w-0 overflow-hidden px-2 pt-0.5">{belowControls}</div>
            </div>
          ) : (
            <>
              {props.queueContent}
              <div className="relative">{inputbarElement}</div>
            </>
          )}
          {showAiDisclaimer ? (
            <div className="-mt-3 pt-1.5 pb-2.5 text-center text-[11px] text-muted-foreground">
              {t('chat.input.ai_disclaimer')}
            </div>
          ) : null}
        </div>
      </div>
    </NarrowLayout>
  )
}

/** Both deferred and rich editors consume the same scoped actions and draft guard. */
function ComposerSurface(props: ComposerSurfaceProps) {
  const contributions = usePluginContributions('composer')
  const latest = useRef(props)
  latest.current = props
  const instanceId = useId()
  const draftId = JSON.stringify([instanceId, props.draftScope ?? null, props.editingState?.messageId ?? null])
  const blocked = props.editable === false || props.tokens.length > 0 || !!props.draftTokens?.length
  const draft = useRef({ id: draftId, revision: 0, text: props.text, blocked })
  useLayoutEffect(() => {
    const current = draft.current
    if (current.id !== draftId || current.text !== props.text || current.blocked !== blocked)
      draft.current = { id: draftId, revision: current.revision + 1, text: props.text, blocked }
  })
  const onTextChange = useCallback((next: string) => {
    // Invalidate before React commits, including change-and-undo within one batch.
    draft.current = { ...draft.current, revision: draft.current.revision + 1, text: next }
    latest.current.onTextChange(next)
  }, [])
  const onTokensChange = useCallback((next: readonly ComposerSerializedToken[]) => {
    draft.current = {
      ...draft.current,
      revision: draft.current.revision + 1,
      blocked: latest.current.editable === false || next.length > 0 || latest.current.tokens.length > 0
    }
    latest.current.onTokensChange(next)
  }, [])
  const readDraft = useCallback((): PluginComposerDraft => {
    const { id, revision, text } = draft.current
    return Object.freeze({ id, revision, text })
  }, [])
  const applyEdit = useCallback(
    (expected: PluginComposerDraft, replacement: string) => {
      const current = latest.current
      if (
        draft.current.blocked ||
        current.editable === false ||
        current.tokens.length ||
        current.draftTokens?.length ||
        JSON.stringify([instanceId, current.draftScope ?? null, current.editingState?.messageId ?? null]) !==
          expected.id ||
        current.text !== expected.text ||
        draft.current.id !== expected.id ||
        draft.current.revision !== expected.revision ||
        draft.current.text !== expected.text
      )
        return
      onTextChange(replacement)
    },
    [instanceId, onTextChange]
  )
  const harness = usePluginContributions('harness')
  const pluginActions = useMemo(
    () =>
      contributions.length || harness.length ? (
        <>
          {contributions.length ? (
            <PluginComposerActions readDraft={readDraft} applyEdit={applyEdit} disabled={blocked} />
          ) : null}
          {harness.map((item) =>
            item.contribution.slot === 'harness' ? (
              <span key={item.key} className="px-1 text-[12px] text-[#8a8a8a]">
                {item.contribution.components.join(' ')}
              </span>
            ) : null
          )}
        </>
      ) : null,
    [contributions, harness, readDraft, blocked, applyEdit]
  )
  const leftControls = useCallback<NonNullable<ComposerSurfaceProps['renderLeftControls']>>(
    (adapter, panel) => (
      <>
        {props.renderLeftControls?.(adapter, panel)}
        {pluginActions}
      </>
    ),
    [props.renderLeftControls, pluginActions]
  )
  const compactControls = useCallback<NonNullable<ComposerSurfaceProps['renderCompactControls']>>(
    (adapter, panel) => (
      <>
        {props.renderCompactControls?.(adapter, panel)}
        {pluginActions}
      </>
    ),
    [props.renderCompactControls, pluginActions]
  )
  return (
    <DeferredComposerSurface
      {...props}
      onTextChange={onTextChange}
      onTokensChange={onTokensChange}
      renderLeftControls={pluginActions ? leftControls : props.renderLeftControls}
      renderCompactControls={pluginActions ? compactControls : props.renderCompactControls}
    />
  )
}

export default ComposerSurface
