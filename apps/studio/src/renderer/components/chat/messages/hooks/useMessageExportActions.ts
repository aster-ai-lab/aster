import { useCallback, useMemo } from 'react'
import { useTranslation } from 'react-i18next'

import type { MessageListActions } from '@renderer/components/chat/messages/types'
import { ipcApi } from '@renderer/ipc'
import { createPersonalNote } from '@renderer/pages/knowledge/personalNote'
import { useDisplayedWorkspacePath } from '@renderer/pages/settings/GeneralSettings/useDisplayedWorkspacePath'
import { chooseImageExportMode } from '@renderer/services/imageExportModeChooser'
import { openRoute } from '@renderer/services/mainWindowNavigation'
import { toast } from '@renderer/services/toast'
import type {
  ExportMessages,
  ExportMessagesToObsidian,
  MessageExportTarget,
  MessageExportView
} from '@renderer/types/messageExport'

type MessageExportActions = Pick<
  MessageListActions,
  | 'saveTextFile'
  | 'saveImage'
  | 'saveToKnowledge'
  | 'exportMessageAsMarkdown'
  | 'exportToNotes'
  | 'exportToWord'
  | 'exportToNotion'
  | 'exportToYuque'
  | 'exportToObsidian'
  | 'exportToJoplin'
  | 'exportToSiyuan'
>

interface MessageExportActionParams {
  topicName?: string
  exportToObsidian: ExportMessagesToObsidian
}

export function useMessageExportActions({
  topicName,
  exportToObsidian: showObsidianExport
}: MessageExportActionParams): MessageExportActions & { exportMessages: ExportMessages } {
  const { t } = useTranslation()
  const workspaceRoot = useDisplayedWorkspacePath()

  const exportContent = useCallback(
    async (messages: MessageExportView[], target: MessageExportTarget, title?: string) => {
      const { exportMessagesToTarget } = await import('@renderer/services/ExportService')
      return exportMessagesToTarget(messages, target, {
        title,
        exportToObsidian: showObsidianExport,
        chooseImageMode: chooseImageExportMode
      })
    },
    [showObsidianExport]
  )

  const exportMessages = useCallback<ExportMessages>(
    (messages, target) => exportContent(messages, target, topicName?.trim() || undefined),
    [exportContent, topicName]
  )

  const saveTextFile = useCallback((fileName: string, content: string) => {
    return window.api.file.save(fileName, content)
  }, [])

  const saveImage = useCallback((fileName: string, dataUrl: string) => {
    return window.api.file.saveImage(fileName, dataUrl)
  }, [])

  const exportToWord = useCallback(async (markdown: string, title: string) => {
    await ipcApi.request('export.word.from_markdown', { markdown, fileName: title })
  }, [])

  const saveToKnowledge = useCallback(async (message: MessageExportView) => {
    const { default: SaveToKnowledgePopup } = await import('@renderer/components/SaveToKnowledgePopup')
    void SaveToKnowledgePopup.showForMessage(message)
  }, [])

  const exportMessageAsMarkdown = useCallback(
    async (message: MessageExportView, includeReasoning?: boolean) => {
      await exportContent([message], includeReasoning ? 'markdown-reason' : 'markdown')
    },
    [exportContent]
  )

  const exportToNotes = useCallback(
    async (message: MessageExportView) => {
      const { getMessageTitle, messageToMarkdown } = await import('@renderer/services/ExportService')
      try {
        if (!workspaceRoot) throw new Error('workspace unavailable')
        const title = await getMessageTitle(message)
        const markdown = await messageToMarkdown(message)
        await createPersonalNote(workspaceRoot, title, markdown, t('knowledge.notes.untitled'))
        openRoute('/app/knowledge')
        toast.success(t('message.success.notes.export'))
      } catch (error) {
        toast.error(t('message.error.notes.export'))
        throw error
      }
    },
    [t, workspaceRoot]
  )

  const exportToNotion = useCallback(async (message: MessageExportView) => {
    const { exportMessageToNotion, getMessageTitle, messageToMarkdown } =
      await import('@renderer/services/ExportService')
    const title = await getMessageTitle(message)
    const markdown = await messageToMarkdown(message)
    await exportMessageToNotion(title, markdown, message)
  }, [])

  const exportToYuque = useCallback(
    async (message: MessageExportView) => {
      await exportContent([message], 'yuque')
    },
    [exportContent]
  )

  const exportToObsidian = useCallback(
    async (message: MessageExportView) => {
      await exportContent([message], 'obsidian', topicName || 'Untitled')
    },
    [exportContent, topicName]
  )

  const exportToJoplin = useCallback(
    async (message: MessageExportView) => {
      await exportContent([message], 'joplin')
    },
    [exportContent]
  )

  const exportToSiyuan = useCallback(
    async (message: MessageExportView) => {
      await exportContent([message], 'siyuan')
    },
    [exportContent]
  )

  return useMemo(
    () => ({
      saveTextFile,
      saveImage,
      saveToKnowledge,
      exportMessageAsMarkdown,
      exportMessages,
      exportToNotes,
      exportToWord,
      exportToNotion,
      exportToYuque,
      exportToObsidian,
      exportToJoplin,
      exportToSiyuan
    }),
    [
      exportMessageAsMarkdown,
      exportMessages,
      exportToJoplin,
      exportToNotes,
      exportToNotion,
      exportToObsidian,
      exportToSiyuan,
      exportToWord,
      exportToYuque,
      saveImage,
      saveTextFile,
      saveToKnowledge
    ]
  )
}
