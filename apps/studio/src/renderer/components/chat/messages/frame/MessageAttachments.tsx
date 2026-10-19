import type { FC } from 'react'
import { useTranslation } from 'react-i18next'

import { useQuery } from '@data/hooks/useDataApi'
import { popup } from '@renderer/services/popup'
import type { FileHandle } from '@shared/data/types/file'
import { isFileEntryHandle } from '@shared/utils/file'

import { useOptionalMessageListActions, useOptionalMessageListUi } from '../MessageListProvider'
import { FileMessageCard } from './FileMessageCard'

interface Props {
  /** Addresses the file for open / preview. Main resolves it; never a path this component built. */
  handle: FileHandle
  name: string
  ext: string
  createdAt: string
}

function nameWithExtension(name: string, ext: string): string {
  const suffix = ext.trim().replace(/^\./, '')
  if (!suffix) return name
  if (name.toLowerCase().endsWith(`.${suffix.toLowerCase()}`)) return name
  return `${name}.${suffix}`
}

const MessageAttachments: FC<Props> = ({ handle, name, ext, createdAt }) => {
  const { t } = useTranslation()
  const actions = useOptionalMessageListActions()
  const messageUi = useOptionalMessageListUi()
  const entryId = isFileEntryHandle(handle) ? handle.entryId : undefined
  // The part carries no size; the entry row is the authoritative one for managed files.
  const { data: entry } = useQuery('/files/entries/:id', {
    params: { id: entryId ?? '' },
    enabled: !!entryId
  })

  const displayExt = entry?.ext || ext
  const fileView = messageUi?.getFileView?.({
    origin_name: entry?.name || name,
    ext: displayExt,
    created_at: createdAt
  })
  const fileName = fileView?.displayName || entry?.name || name
  const size = entry?.origin === 'internal' ? entry.size : undefined
  const openFile = actions?.openFile
  const previewFile = actions?.previewFile
  const target = { handle, name: fileName, ext: displayExt }

  const handleOpen = () => {
    if (previewFile) {
      void Promise.resolve(previewFile(target)).catch(() => {
        void popup.error({ content: t('files.preview.error'), centered: true })
      })
      return
    }
    if (!openFile) return
    void Promise.resolve(openFile(target)).catch(() => {
      void popup.error({ content: t('files.preview.error'), centered: true })
    })
  }

  return (
    <div className="message-attachments mt-0.5 mb-1.5 max-w-full min-w-0">
      <FileMessageCard
        iconName={nameWithExtension(fileName, displayExt)}
        name={fileName}
        onOpen={previewFile || openFile ? handleOpen : undefined}
        sizeBytes={size}
      />
    </div>
  )
}

export default MessageAttachments
