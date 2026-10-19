import { ArrowLeft, Copy, ExternalLink, FolderSearch } from 'lucide-react'
import type { ReactNode } from 'react'
import { useTranslation } from 'react-i18next'

import { Button, Tooltip } from '@a3s-lab/ui'
import { DocumentFileIcon, workspaceFileIconType } from '@renderer/components/icons/DocumentFileIcon'
import { useCurrentTab } from '@renderer/hooks/tab'
import { useOptionalTabsContext } from '@renderer/hooks/tab/useTabsContext'
import { ipcApi } from '@renderer/ipc'
import { toast } from '@renderer/services/toast'
import { safeOpen } from '@renderer/utils/file/safeOpen'
import { getFilePreviewFileName } from '@renderer/utils/filePreview'
import { readStudioHomePath } from '@shared/studioRuntime'
import type { AbsoluteFilePath } from '@shared/types/file'
import { createFilePathHandle } from '@shared/utils/file'

export function displayedFileDirectory(filePath: string, homePath: string): string {
  const directory = filePath.replace(/[\\/][^\\/]*$/, '') || filePath
  const home = homePath.replace(/[\\/]+$/, '')
  if (home && (directory === home || directory.startsWith(`${home}/`) || directory.startsWith(`${home}\\`))) {
    return `~${directory.slice(home.length)}`
  }
  return directory
}

function HeaderAction({ label, onClick, children }: { label: string; onClick: () => void; children: ReactNode }) {
  return (
    <Tooltip title={label}>
      <Button
        size="icon-sm"
        variant="ghost"
        aria-label={label}
        className="text-muted-foreground hover:text-foreground"
        onClick={onClick}>
        {children}
      </Button>
    </Tooltip>
  )
}

export function FilePreviewPageHeader({ filePath }: { filePath: AbsoluteFilePath }) {
  const { t } = useTranslation()
  const currentTab = useCurrentTab()
  const tabs = useOptionalTabsContext()
  const fileName = getFilePreviewFileName(filePath)
  const directory = displayedFileDirectory(filePath, readStudioHomePath())
  const handle = createFilePathHandle(filePath)

  return (
    <>
      {tabs && currentTab ? (
        <HeaderAction label={t('file_preview.header.back')} onClick={() => tabs.closeTab(currentTab.id)}>
          <ArrowLeft className="size-4" />
        </HeaderAction>
      ) : null}
      <span className="flex w-5 shrink-0 justify-center">
        <DocumentFileIcon type={workspaceFileIconType(fileName)} className="h-5 w-4" />
      </span>
      <div className="flex min-w-0 flex-col leading-tight">
        <span className="truncate font-medium text-[13px] text-foreground" title={fileName}>
          {fileName}
        </span>
        <span className="truncate text-[11px] text-muted-foreground" title={filePath}>
          {directory}
        </span>
      </div>
      <div className="ml-auto flex shrink-0 items-center gap-0.5">
        <HeaderAction
          label={t('file_preview.unsupported.action')}
          onClick={() => void safeOpen(handle).catch(() => toast.error(t('file_preview.unsupported.open_error')))}>
          <ExternalLink className="size-4" />
        </HeaderAction>
        <HeaderAction
          label={t('file_preview.card.reveal')}
          onClick={() =>
            void ipcApi
              .request('file.show_in_folder', handle)
              .catch(() => toast.error(t('file_preview.card.reveal_error')))
          }>
          <FolderSearch className="size-4" />
        </HeaderAction>
        <HeaderAction
          label={t('file_preview.header.copy_path')}
          onClick={() =>
            void navigator.clipboard
              .writeText(filePath)
              .then(() => toast.success(t('file_preview.header.copied')))
              .catch(() => toast.error(t('file_preview.header.copy_error')))
          }>
          <Copy className="size-4" />
        </HeaderAction>
      </div>
    </>
  )
}
