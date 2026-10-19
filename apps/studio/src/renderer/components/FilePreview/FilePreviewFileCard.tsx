import { ExternalLink, FolderSearch } from 'lucide-react'
import { useTranslation } from 'react-i18next'

import { Button } from '@a3s-lab/ui'
import {
  DocumentFileIcon,
  type DocumentFileIconType,
  workspaceFileIconType
} from '@renderer/components/icons/DocumentFileIcon'
import { ipcApi } from '@renderer/ipc'
import { toast } from '@renderer/services/toast'
import { formatFileSize } from '@renderer/utils/file'
import { safeOpen } from '@renderer/utils/file/safeOpen'
import { getFilePreviewExtension, getFilePreviewFileName } from '@renderer/utils/filePreview'
import type { AbsoluteFilePath } from '@shared/types/file'
import { createFilePathHandle } from '@shared/utils/file'

import type { FilePreviewFileMetadata } from './types'

interface FilePreviewFileCardProps {
  filePath: AbsoluteFilePath
  metadata?: FilePreviewFileMetadata
  isText?: boolean
  description: string
}

function cardIconType(fileName: string, isText: boolean): DocumentFileIconType {
  const type = workspaceFileIconType(fileName)
  return type === 'code' && !isText ? 'file' : type
}

export function FilePreviewFileCard({ filePath, metadata, isText = false, description }: FilePreviewFileCardProps) {
  const { t, i18n } = useTranslation()
  const fileName = getFilePreviewFileName(filePath)
  const extension = getFilePreviewExtension(filePath)?.toUpperCase()
  const details = [
    extension ? t('file_preview.card.type', { ext: extension }) : t('file_preview.card.type_unknown'),
    metadata ? formatFileSize(metadata.size) : null,
    metadata && metadata.modifiedAt > 0
      ? t('file_preview.card.modified', {
          time: new Date(metadata.modifiedAt).toLocaleString(i18n?.language, {
            year: 'numeric',
            month: 'short',
            day: 'numeric',
            hour: '2-digit',
            minute: '2-digit'
          })
        })
      : null
  ].filter(Boolean)

  const openWithDefaultApp = () => {
    void safeOpen(createFilePathHandle(filePath)).catch(() => toast.error(t('file_preview.unsupported.open_error')))
  }
  const revealInFolder = () => {
    void ipcApi
      .request('file.show_in_folder', createFilePathHandle(filePath))
      .catch(() => toast.error(t('file_preview.card.reveal_error')))
  }

  return (
    <div data-testid="file-preview-file-card" className="flex h-full items-center justify-center p-8">
      <div className="flex w-full max-w-[380px] flex-col items-center text-center">
        <DocumentFileIcon type={cardIconType(fileName, isText)} className="h-[72px] w-16" />
        <h2 className="mt-4 max-w-full truncate font-semibold text-[15px] text-foreground" title={fileName}>
          {fileName}
        </h2>
        <p className="mt-1 text-[12px] text-muted-foreground">{details.join(' · ')}</p>
        <p className="mt-4 text-[13px] text-muted-foreground leading-5">{description}</p>
        <div className="mt-5 flex flex-wrap items-center justify-center gap-2">
          <Button size="sm" onClick={openWithDefaultApp}>
            <ExternalLink className="size-3.5" />
            {t('file_preview.unsupported.action')}
          </Button>
          <Button size="sm" variant="outline" onClick={revealInFolder}>
            <FolderSearch className="size-3.5" />
            {t('file_preview.card.reveal')}
          </Button>
        </div>
      </div>
    </div>
  )
}
