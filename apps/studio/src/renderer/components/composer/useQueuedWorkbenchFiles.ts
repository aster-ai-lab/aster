import type { TFunction } from 'i18next'
import { useEffect, useRef } from 'react'
import { useTranslation } from 'react-i18next'

import { toast } from '@renderer/services/toast'
import { filterSupportedFiles } from '@renderer/utils/file'
import { filesToDroppedMetadata } from '@renderer/utils/input'
import { type ComposerAttachment, toComposerAttachments } from '@renderer/utils/message/composerAttachment'

import { subscribeWorkbenchDroppedFiles } from './workbenchDroppedFiles'

type SetComposerFiles = (updater: (prevFiles: ComposerAttachment[]) => ComposerAttachment[]) => void

async function attachQueuedFiles(
  files: File[],
  supportedExts: string[],
  setFiles: SetComposerFiles,
  translate: TFunction
) {
  const metadata = await filesToDroppedMetadata(files)
  if (metadata.length === 0) {
    toast.error(translate('chat.input.file_error'))
    return
  }
  const supported = await filterSupportedFiles(metadata, supportedExts)
  if (supported.length > 0) {
    setFiles((current) => [...current, ...toComposerAttachments(supported)])
  }
  if (supported.length !== metadata.length) {
    toast.info(translate('chat.input.file_not_supported_count', { count: metadata.length - supported.length }))
  }
}

/** Replay files dropped on the homepage placeholder once this composer can hold them. */
export function useQueuedWorkbenchFiles(options: {
  enabled: boolean
  supportedExts: string[]
  setFiles: SetComposerFiles
}) {
  const { t } = useTranslation()
  const latest = useRef(options)
  latest.current = options

  useEffect(() => {
    if (!options.enabled) return
    return subscribeWorkbenchDroppedFiles((files) => {
      const current = latest.current
      void attachQueuedFiles(files, current.supportedExts, current.setFiles, t)
    })
  }, [options.enabled, t])
}
