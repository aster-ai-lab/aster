import dayjs from 'dayjs'
import type { TFunction } from 'i18next'
import {
  AtSign,
  BookOpen,
  Brain,
  Check,
  CirclePause,
  Clipboard,
  Copy,
  CopyPlus,
  File,
  FilePenLine,
  FileText,
  Gem,
  Image,
  Languages,
  Library,
  ListChecks,
  MoreHorizontal,
  Notebook,
  NotebookPen,
  Save,
  Split,
  StickyNote,
  ThumbsUp,
  Upload,
  X
} from 'lucide-react'
import type { ReactNode, RefObject } from 'react'

import { loggerService } from '@logger'
import {
  DEFAULT_MESSAGE_MENUBAR_BUTTON_IDS,
  type MessageMenuBarButtonId,
  STREAMING_DISABLED_BUTTON_IDS
} from '@renderer/components/chat/messages/frame/messageMenuBarConfig'
import { getMessageDeleteUnavailableText } from '@renderer/components/chat/messages/utils/messageDeleteAvailability'
import CopyIcon from '@renderer/components/icons/CopyIcon'
import DeleteIcon from '@renderer/components/icons/DeleteIcon'
import EditIcon from '@renderer/components/icons/EditIcon'
import RefreshIcon from '@renderer/components/icons/RefreshIcon'
import type { MessageExportView } from '@renderer/types/messageExport'
import { formatErrorMessageWithPrefix } from '@renderer/utils/error'
import { removeTrailingDoubleSpaces } from '@renderer/utils/markdownLight'
import { createComposerRichClipboardContentFromParts } from '@renderer/utils/message/composerClipboard'
import { getTranslationFromParts } from '@renderer/utils/message/partsHelpers'
import type { CherryMessagePart } from '@shared/data/types/message'
import type { TranslateLanguage } from '@shared/data/types/translate'

import { createActionRegistry } from '../../actions/actionRegistry'
import type { ActionAvailabilityInput, ActionDescriptor, ResolvedAction } from '../../actions/actionTypes'
import { messageExportTargets } from '../messageExportTargets'
import type { MessageListActions, MessageListItem, MessageListSelectionState } from '../types'
import type { MessageMenuConfig } from '../types'
import { getMessageListItemModelName } from '../utils/messageListItem'
import {
  renderDeleteToolbarAction,
  renderModelPickerToolbarAction,
  renderMoreMenuToolbarAction,
  renderTranslateToolbarAction
} from './MessageMenuBarToolbarRenderers'

export interface MessageMenuBarActionContext {
  actions: MessageListActions
  message: MessageListItem
  messageParts: CherryMessagePart[]
  messageForExport: MessageExportView
  messageContainerRef: RefObject<HTMLDivElement>
  acquireMessageCaptureLease?: (messageId: string) => () => void
  getRenderedMessageElement?: (messageId: string) => HTMLElement | null
  mainTextContent: string
  selection?: MessageListSelectionState
  menuConfig: MessageMenuConfig
  copied: boolean
  setCopied: (value: boolean) => void
  isAssistantMessage: boolean
  isGrouped?: boolean
  isLastMessage: boolean
  isProcessing: boolean
  isTranslating: boolean
  hasTranslationBlocks: boolean
  isUserMessage: boolean
  isSelectedForContext: boolean
  isEditable: boolean
  translateLanguages: TranslateLanguage[]
  translationLanguagesStatus?: 'loading' | 'error' | 'ready'
  getTranslationLanguageLabel?: (language: TranslateLanguage, withEmoji?: boolean) => string | undefined
  startEditingMessage?: (messageId: string) => void
  onSelectContext?: (messageId: string) => void
  t: TFunction
}

export type MessageMenuBarResolvedAction = ResolvedAction<MessageMenuBarActionContext>

export interface MessageMenuBarToolbarRenderContext {
  action: MessageMenuBarResolvedAction
  actionContext: MessageMenuBarActionContext
  executeAction: (action: MessageMenuBarResolvedAction) => void | Promise<void>
  menuActions: MessageMenuBarResolvedAction[]
  onMenuOpenChange?: (open: boolean) => void
  softHoverBg: boolean
  translationItems: MessageMenuBarTranslationItem[]
}

export type MessageMenuBarToolbarRenderer = (context: MessageMenuBarToolbarRenderContext) => ReactNode

export type MessageMenuBarResolvedToolbarAction = MessageMenuBarResolvedAction & {
  renderToolbar?: MessageMenuBarToolbarRenderer
}

export type MessageMenuBarTranslationItem =
  | {
      key: string
      label: string
      icon?: ReactNode
      enabled?: boolean
      onSelect: () => void | Promise<void>
    }
  | {
      key: string
      type: 'divider'
    }

const TOOLBAR_ICON = { size: 14, strokeWidth: 1.7 } as const
const MENU_ICON = { size: 16, strokeWidth: 1.7 } as const

const EXPORT_TARGET_ICONS: Record<string, ReactNode> = {
  markdown: <FileText {...MENU_ICON} />,
  'markdown-reason': <Brain {...MENU_ICON} />,
  word: <File {...MENU_ICON} />,
  notion: <Notebook {...MENU_ICON} />,
  yuque: <BookOpen {...MENU_ICON} />,
  obsidian: <Gem {...MENU_ICON} />,
  joplin: <StickyNote {...MENU_ICON} />,
  siyuan: <Library {...MENU_ICON} />
}

const toolbarOrder = new Map(DEFAULT_MESSAGE_MENUBAR_BUTTON_IDS.map((id, index) => [id, index * 10]))
const toolbarRenderers = new Map<string, MessageMenuBarToolbarRenderer>()

const messageMenuBarActionRegistry = createActionRegistry<MessageMenuBarActionContext>()
const logger = loggerService.withContext('MessageMenuBarActions')

function toolbarAvailability(
  id: MessageMenuBarButtonId,
  isVisible: (context: MessageMenuBarActionContext) => boolean = () => true
) {
  return (context: MessageMenuBarActionContext): ActionAvailabilityInput => {
    const visible = isVisible(context)
    return {
      visible,
      enabled: visible && !(context.isProcessing && STREAMING_DISABLED_BUTTON_IDS.has(id))
    }
  }
}

function canStartEditing({
  actions,
  message,
  isTranslating,
  startEditingMessage
}: MessageMenuBarActionContext): boolean {
  return !isTranslating && !!startEditingMessage && (actions.canEditMessage?.(message) ?? !!actions.editMessage)
}

function notifyCommandError(id: string, context: MessageMenuBarActionContext, error: unknown) {
  logger.error(`Message menu action failed: ${id}`, error as Error)
  context.actions.notifyError?.(formatErrorMessageWithPrefix(error, context.t('message.error.unknown')))
}

function registerCommand(id: string, run: (context: MessageMenuBarActionContext) => void | Promise<void>) {
  messageMenuBarActionRegistry.registerCommand({ id, run })
}

function registerAction(descriptor: ActionDescriptor<MessageMenuBarActionContext>) {
  messageMenuBarActionRegistry.registerAction(descriptor)
}

function registerToolbarAction(
  descriptor: Omit<ActionDescriptor<MessageMenuBarActionContext>, 'order' | 'surface'> & {
    id: MessageMenuBarButtonId
    renderToolbar?: MessageMenuBarToolbarRenderer
  }
) {
  const { renderToolbar, ...actionDescriptor } = descriptor
  if (renderToolbar) {
    toolbarRenderers.set(actionDescriptor.id, renderToolbar)
  }
  registerAction({
    ...actionDescriptor,
    order: toolbarOrder.get(actionDescriptor.id) ?? 0,
    surface: 'toolbar'
  })
}

function getMessageCaptureRef(context: MessageMenuBarActionContext): RefObject<HTMLElement | null> {
  const getRenderedMessageElement = context.getRenderedMessageElement
  if (!getRenderedMessageElement) return context.messageContainerRef

  return {
    get current() {
      const element = getRenderedMessageElement(context.message.id)
      if (!element) {
        throw new Error('Message is no longer available for image capture')
      }
      return element
    }
  }
}

async function withMessageCaptureLease<T>(context: MessageMenuBarActionContext, capture: () => Promise<T>): Promise<T> {
  const release = context.acquireMessageCaptureLease?.(context.message.id)
  try {
    return await capture()
  } finally {
    release?.()
  }
}

registerCommand('message.copy', async ({ actions, mainTextContent, messageParts, setCopied, t }) => {
  const richContent = actions.copyRichContent ? createComposerRichClipboardContentFromParts(messageParts) : null
  if (richContent) {
    // Match the plain copy path's text/plain normalization; the private fragment
    // keeps the original text so paste restoration stays lossless.
    const plainText = removeTrailingDoubleSpaces(richContent.plainText.trimStart())
    await actions.copyRichContent?.(
      { ...richContent, plainText },
      {
        successMessage: t('message.copied')
      }
    )
  } else {
    await actions.copyText?.(removeTrailingDoubleSpaces(mainTextContent.trimStart()), {
      successMessage: t('message.copied')
    })
  }
  setCopied(true)
})

registerCommand('message.edit', ({ message, startEditingMessage }) => {
  startEditingMessage?.(message.id)
})

registerCommand('message.regenerate', async ({ actions, message }) => {
  await actions.regenerateMessage?.(message.id)
})

registerCommand('message.delete', async ({ actions, message }) => {
  await actions.abortMessageTranslation?.(message.id)
  await actions.deleteMessage?.(message.id, {
    modelName: getMessageListItemModelName(message) || undefined
  })
})

registerCommand('message.abortTranslation', async ({ actions, message }) => {
  await actions.abortMessageTranslation?.(message.id)
})

registerCommand('message.newBranch', async ({ actions, message, t }) => {
  await actions.startMessageBranch?.(message.id)
  actions.notifySuccess?.(t('chat.message.new.branch.created'))
})
registerCommand('message.forkSession', async ({ actions, message }) => {
  await actions.forkSession?.run(message.id)
})

registerCommand('message.copyToNewTopic', async ({ actions, message, t }) => {
  await actions.copyBranchToNewTopic?.(message.id)
  actions.notifySuccess?.(t('chat.message.flow.copy_topic.created'))
})

registerCommand('message.multiSelect', ({ actions }) => {
  actions.toggleMultiSelectMode?.(true)
})

registerCommand('message.saveFile', async ({ actions, mainTextContent, message }) => {
  const fileName = dayjs(message.createdAt).format('YYYYMMDDHHmm') + '.md'
  await actions.saveTextFile?.(fileName, mainTextContent)
})

registerCommand('message.exportNotes', async ({ actions, messageForExport }) => {
  await actions.exportToNotes?.(messageForExport)
})

registerCommand('message.copyPlainText', async ({ actions, messageForExport, t }) => {
  const { messageToPlainText } = await import('@renderer/utils/export')
  await actions.copyText?.(await messageToPlainText(messageForExport), {
    successMessage: t('message.copy.success')
  })
})

function readBlobDataUrl(blob: Blob): Promise<string> {
  return new Promise((resolve, reject) => {
    const reader = new FileReader()
    reader.onload = () => resolve(String(reader.result || ''))
    reader.onerror = () => reject(reader.error ?? new Error('image read failed'))
    reader.readAsDataURL(blob)
  })
}

registerCommand('message.copyImage', async (context) => {
  await withMessageCaptureLease(context, async () => {
    const { exportService } = await import('@renderer/services/ExportService')
    const messageContainerRef = getMessageCaptureRef(context)
    await exportService.captureScrollableAsBlob(messageContainerRef, async (blob) => {
      if (!blob) return
      try {
        await context.actions.copyImage?.(blob)
      } catch (error) {
        // WKWebView rejects image clipboard writes, so the same PNG is saved instead.
        if (!context.actions.saveImage) throw error
        const saved = await context.actions.saveImage('message', await readBlobDataUrl(blob))
        if (saved) context.actions.notifySuccess?.(context.t('chat.topics.export.image_saved'))
      }
    })
  })
})

registerCommand('message.exportImage', async (context) => {
  await withMessageCaptureLease(context, async () => {
    const { exportService, getMessageTitle } = await import('@renderer/services/ExportService')
    const imageData = await exportService.captureScrollableAsDataUrl(getMessageCaptureRef(context))
    const title = await getMessageTitle(context.messageForExport)
    if (!title || !imageData || !context.actions.saveImage) {
      context.actions.notifyError?.(context.t('message.error.unknown'))
      return
    }

    const success = await context.actions.saveImage(title, imageData)
    // A dismissed folder panel returns false. That is a cancel, not a failure.
    if (success) {
      context.actions.notifySuccess?.(context.t('chat.topics.export.image_saved'))
    }
  })
})

registerCommand('message.exportMarkdown', async ({ actions, messageForExport }) => {
  await actions.exportMessageAsMarkdown?.(messageForExport)
})

registerCommand('message.exportMarkdownReason', async ({ actions, messageForExport }) => {
  await actions.exportMessageAsMarkdown?.(messageForExport, true)
})

registerCommand('message.exportWord', async ({ actions, messageForExport }) => {
  const { getMessageTitle, messageToMarkdown } = await import('@renderer/services/ExportService')
  const markdown = await messageToMarkdown(messageForExport)
  const title = await getMessageTitle(messageForExport)
  await actions.exportToWord?.(markdown, title)
})

registerCommand('message.exportNotion', async ({ actions, messageForExport }) => {
  await actions.exportToNotion?.(messageForExport)
})

registerCommand('message.exportYuque', async ({ actions, messageForExport }) => {
  await actions.exportToYuque?.(messageForExport)
})

registerCommand('message.exportObsidian', async ({ actions, messageForExport }) => {
  await actions.exportToObsidian?.(messageForExport)
})

registerCommand('message.exportJoplin', async ({ actions, messageForExport }) => {
  await actions.exportToJoplin?.(messageForExport)
})

registerCommand('message.exportSiyuan', async ({ actions, messageForExport }) => {
  await actions.exportToSiyuan?.(messageForExport)
})

registerCommand('message.useful', ({ message, onSelectContext }) => {
  onSelectContext?.(message.id)
})

registerToolbarAction({
  id: 'user-edit',
  commandId: 'message.edit',
  label: ({ t, actions }) => actions.editLabel ?? t('common.edit'),
  icon: <EditIcon {...TOOLBAR_ICON} />,
  availability: toolbarAvailability('user-edit', (context) => context.isUserMessage && canStartEditing(context))
})

registerToolbarAction({
  id: 'copy',
  commandId: 'message.copy',
  label: ({ t }) => t('common.copy'),
  icon: ({ copied }) => (copied ? <Check {...TOOLBAR_ICON} /> : <CopyIcon {...TOOLBAR_ICON} />),
  availability: toolbarAvailability('copy', ({ actions }) => !!actions.copyText)
})

registerToolbarAction({
  id: 'assistant-regenerate',
  commandId: 'message.regenerate',
  label: ({ t }) => t('common.regenerate'),
  icon: <RefreshIcon {...TOOLBAR_ICON} />,
  availability: toolbarAvailability(
    'assistant-regenerate',
    ({ actions, isAssistantMessage }) => isAssistantMessage && !!actions.regenerateMessage
  )
})

registerToolbarAction({
  id: 'assistant-mention-model',
  renderToolbar: renderModelPickerToolbarAction,
  label: ({ t }) => t('message.mention.title'),
  icon: <AtSign {...TOOLBAR_ICON} />,
  availability: toolbarAvailability(
    'assistant-mention-model',
    ({ actions, isAssistantMessage }) => isAssistantMessage && !!actions.renderRegenerateModelPicker
  )
})

registerToolbarAction({
  id: 'translate',
  renderToolbar: renderTranslateToolbarAction,
  commandId: 'message.abortTranslation',
  label: ({ t }) => t('chat.translate'),
  icon: ({ isTranslating }) => (isTranslating ? <CirclePause {...TOOLBAR_ICON} /> : <Languages {...TOOLBAR_ICON} />),
  availability: (context) => {
    const canTranslate =
      !!context.actions.translateMessage &&
      (context.translateLanguages.length > 0 || !!context.actions.requestTranslationLanguages)
    const canCopyTranslation = context.hasTranslationBlocks && !!context.actions.copyText
    const canRemoveTranslation = context.hasTranslationBlocks && !!context.actions.removeMessageTranslation
    const canAbortTranslation = context.isTranslating && !!context.actions.abortMessageTranslation
    const visible =
      !context.isUserMessage && (canTranslate || canCopyTranslation || canRemoveTranslation || canAbortTranslation)

    return {
      visible,
      enabled:
        visible &&
        (context.isTranslating ? canAbortTranslation : canTranslate || canCopyTranslation || canRemoveTranslation)
    }
  }
})

registerToolbarAction({
  id: 'useful',
  commandId: 'message.useful',
  label: ({ t }) => t('chat.message.useful.label'),
  icon: ({ isSelectedForContext }) =>
    isSelectedForContext ? <ThumbsUp size={14} fill="currentColor" strokeWidth={0} /> : <ThumbsUp {...TOOLBAR_ICON} />,
  availability: toolbarAvailability(
    'useful',
    ({ actions, isAssistantMessage, isGrouped }) => isAssistantMessage && !!isGrouped && !!actions.setActiveBranch
  )
})

registerToolbarAction({
  id: 'notes',
  commandId: 'message.exportNotes',
  label: ({ t }) => t('chat.save.wiki.title'),
  icon: <NotebookPen {...TOOLBAR_ICON} />,
  availability: toolbarAvailability(
    'notes',
    ({ actions, isAssistantMessage }) => isAssistantMessage && !!actions.exportToNotes
  )
})

registerToolbarAction({
  id: 'delete',
  renderToolbar: renderDeleteToolbarAction,
  commandId: 'message.delete',
  label: ({ t }) => t('common.delete'),
  icon: <DeleteIcon {...TOOLBAR_ICON} />,
  confirm: ({ menuConfig, t }) =>
    menuConfig.confirmDeleteMessage
      ? {
          title: t('message.message.delete.content'),
          confirmText: t('common.delete'),
          destructive: true
        }
      : undefined,
  availability: ({ actions, isProcessing, message, t }) => {
    const visible = !!actions.deleteMessage
    const deleteAvailability = actions.getMessageDeleteAvailability?.(message.id) ?? { enabled: true }
    const reason = getMessageDeleteUnavailableText(
      deleteAvailability.enabled ? undefined : deleteAvailability.reason,
      t
    )
    return {
      visible,
      enabled: visible && !isProcessing && deleteAvailability.enabled,
      reason
    }
  }
})

registerToolbarAction({
  id: 'more-menu',
  renderToolbar: renderMoreMenuToolbarAction,
  label: ({ t }) => t('chat.message.more'),
  icon: <MoreHorizontal {...TOOLBAR_ICON} />,
  availability: toolbarAvailability('more-menu', ({ isUserMessage }) => !isUserMessage)
})

registerAction({
  id: 'edit',
  commandId: 'message.edit',
  label: ({ t, actions }) => actions.editLabel ?? t('common.edit'),
  icon: <FilePenLine {...MENU_ICON} />,
  group: 'write',
  order: 10,
  surface: 'menu',
  availability: (context) =>
    context.isEditable && (context.isUserMessage || context.isAssistantMessage) && canStartEditing(context)
})

registerAction({
  id: 'new-branch',
  commandId: 'message.newBranch',
  label: ({ t }) => t('chat.message.new.branch.label'),
  icon: <Split {...MENU_ICON} />,
  group: 'write',
  order: 20,
  surface: 'menu',
  availability: ({ actions, isAssistantMessage }) => {
    if (!actions.startMessageBranch || !isAssistantMessage) return false
    return true
  }
})

registerAction({
  id: 'fork-session',
  commandId: 'message.forkSession',
  label: ({ actions }) => actions.forkSession?.label ?? '',
  icon: <Split {...MENU_ICON} />,
  group: 'write',
  order: 22,
  surface: 'menu',
  availability: ({ actions, message }) => actions.forkSession?.availability(message) ?? false
})

registerAction({
  id: 'copy-to-new-topic',
  commandId: 'message.copyToNewTopic',
  label: ({ t }) => t('chat.message.flow.copy_topic.label'),
  icon: <CopyPlus {...MENU_ICON} />,
  group: 'write',
  order: 25,
  surface: 'menu',
  availability: ({ actions, isAssistantMessage }) => !!actions.copyBranchToNewTopic && isAssistantMessage
})

registerAction({
  id: 'multi-select',
  commandId: 'message.multiSelect',
  label: ({ t }) => t('chat.multiple.select.label'),
  icon: <ListChecks {...MENU_ICON} />,
  group: 'write',
  order: 30,
  surface: 'menu',
  availability: ({ actions, isProcessing, selection }) => ({
    visible: !!selection?.enabled && !!actions.toggleMultiSelectMode,
    enabled: !isProcessing
  })
})

registerAction({
  id: 'save',
  label: ({ t }) => t('chat.save.label'),
  icon: <Save {...MENU_ICON} />,
  group: 'save',
  order: 100,
  surface: 'menu',
  children: [
    {
      id: 'save.file',
      commandId: 'message.saveFile',
      label: ({ t }) => t('chat.save.file.title'),
      icon: <FileText {...MENU_ICON} />,
      order: 10,
      availability: ({ actions }) => !!actions.saveTextFile
    },
    {
      id: 'save.notes',
      commandId: 'message.exportNotes',
      label: ({ t }) => t('chat.save.wiki.title'),
      icon: <NotebookPen {...MENU_ICON} />,
      order: 20,
      availability: ({ actions, isAssistantMessage }) => isAssistantMessage && !!actions.exportToNotes
    }
  ]
})

registerAction({
  id: 'export',
  label: ({ t }) => t('chat.topics.export.title'),
  icon: <Upload {...MENU_ICON} />,
  group: 'export',
  order: 200,
  surface: 'menu',
  children: [
    {
      id: 'export.image',
      commandId: 'message.exportImage',
      label: ({ t }) => t('chat.topics.export.image'),
      icon: <Image {...MENU_ICON} />,
      group: 'file',
      order: 10,
      availability: ({ actions, menuConfig }) => menuConfig.exportMenuOptions.image && !!actions.saveImage
    },
    ...messageExportTargets.map(({ target, commandId, labelKey, group, option, action }, index) => ({
      id: `export.${target}`,
      commandId,
      label: ({ t }: MessageMenuBarActionContext) => t(labelKey),
      icon: EXPORT_TARGET_ICONS[target],
      group,
      order: (index + 2) * 10,
      availability: ({ actions, menuConfig }: MessageMenuBarActionContext) =>
        menuConfig.exportMenuOptions[option] && !!actions[action]
    })),
    {
      id: 'export.copy-plain-text',
      commandId: 'message.copyPlainText',
      label: ({ t }) => t('chat.topics.copy.plain_text'),
      icon: <Clipboard {...MENU_ICON} />,
      group: 'copy',
      order: 100,
      availability: ({ actions, menuConfig }) => menuConfig.exportMenuOptions.plain_text && !!actions.copyText
    },
    {
      id: 'export.copy-image',
      commandId: 'message.copyImage',
      label: ({ t }) => t('chat.topics.copy.image'),
      icon: <Image {...MENU_ICON} />,
      group: 'copy',
      order: 110,
      availability: ({ actions, menuConfig }) => menuConfig.exportMenuOptions.image && !!actions.copyImage
    }
  ]
})

export function resolveMessageMenuBarTranslationItems(
  context: MessageMenuBarActionContext
): MessageMenuBarTranslationItem[] {
  const { actions, getTranslationLanguageLabel, hasTranslationBlocks, mainTextContent, message, messageParts, t } =
    context

  const items: MessageMenuBarTranslationItem[] = actions.translateMessage
    ? context.translateLanguages.map((language) => ({
        label: getTranslationLanguageLabel?.(language) ?? language.langCode,
        key: language.langCode,
        onSelect: async () => {
          try {
            await actions.translateMessage?.(message.id, language, mainTextContent)
          } catch (error) {
            notifyCommandError('message.translate', context, error)
          }
        }
      }))
    : []

  if (items.length === 0 && actions.translateMessage && actions.requestTranslationLanguages) {
    const retryTranslationLanguages = actions.retryTranslationLanguages
    if (context.translationLanguagesStatus === 'error' && retryTranslationLanguages) {
      items.push({
        key: 'translate-retry',
        label: t('common.retry'),
        onSelect: () => retryTranslationLanguages()
      })
    } else {
      items.push({
        key: 'translate-loading',
        label: t('common.loading'),
        enabled: false,
        onSelect: () => undefined
      })
    }
  }

  if (!hasTranslationBlocks) return items

  const trailingItems: MessageMenuBarTranslationItem[] = []

  if (actions.copyText) {
    trailingItems.push({
      label: t('common.copy'),
      icon: <Copy {...MENU_ICON} />,
      key: 'translate-copy',
      onSelect: async () => {
        const translationContent = getTranslationFromParts(messageParts)
          .map((item) => item.content || '')
          .join('\n\n')
          .trim()

        if (translationContent) {
          try {
            await actions.copyText?.(translationContent, {
              successMessage: t('translate.copied')
            })
          } catch (error) {
            notifyCommandError('message.copyTranslation', context, error)
          }
        } else {
          actions.notifyWarning?.(t('translate.empty'))
        }
      }
    })
  }

  if (actions.removeMessageTranslation) {
    trailingItems.push({
      label: t('translate.close'),
      icon: <X {...MENU_ICON} />,
      key: 'translate-close',
      onSelect: async () => {
        try {
          await actions.removeMessageTranslation?.(message.id)
          actions.notifySuccess?.(t('translate.closed'))
        } catch (error) {
          notifyCommandError('message.removeTranslation', context, error)
        }
      }
    })
  }

  if (trailingItems.length === 0) return items

  return [
    ...items,
    ...(items.length > 0 ? [{ type: 'divider' as const, key: 'translate-divider' }] : []),
    ...trailingItems
  ]
}

export function resolveMessageMenuBarToolbarActions(
  context: MessageMenuBarActionContext
): MessageMenuBarResolvedToolbarAction[] {
  return messageMenuBarActionRegistry.resolve(context, 'toolbar').map((action) => {
    const renderToolbar = toolbarRenderers.get(action.id)
    return renderToolbar ? { ...action, renderToolbar } : action
  })
}

export function resolveMessageMenuBarMenuActions(context: MessageMenuBarActionContext): MessageMenuBarResolvedAction[] {
  return messageMenuBarActionRegistry
    .resolve(context, 'menu')
    .filter((action) => !!action.commandId || action.children.length > 0)
}

export async function executeMessageMenuBarAction(
  actionId: string,
  context: MessageMenuBarActionContext
): Promise<boolean> {
  try {
    return await messageMenuBarActionRegistry.execute(actionId, context)
  } catch (error) {
    notifyCommandError(actionId, context, error)
    return false
  }
}
