import {
  type ImagePreviewAction,
  ImagePreviewDialog,
  type ImagePreviewItem,
  type ImagePreviewLabels
} from '@a3s-lab/ui'
import CopyIcon from 'lucide-react/dist/esm/icons/copy'
import { useCallback, useMemo } from 'react'
import { useTranslation } from 'react-i18next'

import { loggerService } from '@logger'
import { ImagePreviewDialogFrame } from '@renderer/components/layout/shellPreviewPortal'
import { createPopup, type PopupInjectedProps } from '@renderer/services/popup'
import { toast } from '@renderer/services/toast'
import {
  copyImageToClipboard,
  type ImageInput,
  imageInputToPreviewUrl,
  type ImagePreviewOptions
} from '@renderer/utils/image'

const logger = loggerService.withContext('ImagePreviewService')

/** Above the workspace rail (z-1000) that shares the tab stacking context. */
const imagePreviewLayer = {
  className: 'z-[1100]',
  overlayClassName: 'z-[1100]'
} as const

type PreviewProps = { src: string } & PopupInjectedProps<void>

export function useImagePreviewLabels(): Partial<ImagePreviewLabels> {
  const { t } = useTranslation()
  return useMemo(
    () => ({
      close: t('preview.close'),
      dialogTitle: t('preview.label'),
      flipHorizontal: t('preview.flip_horizontal'),
      flipVertical: t('preview.flip_vertical'),
      next: t('preview.next'),
      previous: t('preview.previous'),
      reset: t('preview.reset'),
      rotateLeft: t('preview.rotate_left'),
      rotateRight: t('preview.rotate_right'),
      zoomIn: t('preview.zoom_in'),
      zoomOut: t('preview.zoom_out')
    }),
    [t]
  )
}

/** One full-screen preview. Callers keep their own object URL; this view does not revoke it. */
export function ImagePreviewOverlay({
  alt,
  onOpenChange,
  open,
  src,
  toolbarActions
}: {
  alt?: string
  onOpenChange: (open: boolean) => void
  open: boolean
  src: string
  toolbarActions?: ImagePreviewAction[]
}) {
  const labels = useImagePreviewLabels()
  if (!open) return null
  return (
    <ImagePreviewDialogFrame>
      <ImagePreviewDialog
        className={imagePreviewLayer.className}
        items={[{ alt, id: src, src }]}
        labels={labels}
        onOpenChange={onOpenChange}
        open
        overlayClassName={imagePreviewLayer.overlayClassName}
        toolbarActions={toolbarActions}
      />
    </ImagePreviewDialogFrame>
  )
}

const ImagePreviewContainer: React.FC<PreviewProps> = ({ src, open, resolve }) => {
  const { t } = useTranslation()
  const labels = useImagePreviewLabels()

  const handleVisibleChange = (visible: boolean) => {
    if (!visible) {
      // Revoke the object URL on the close path (createObjectURL happens in
      // imageInputToPreviewUrl for SVG elements and blobs).
      if (src.startsWith('blob:')) {
        URL.revokeObjectURL(src)
      }
      resolve()
    }
  }

  const handleCopyImage = useCallback(
    async (item: ImagePreviewItem) => {
      try {
        await copyImageToClipboard(item.src)
        toast.success(t('message.copy.success'))
      } catch (error) {
        const err = error as Error
        logger.error(`Failed to copy image: ${err.message}`, { stack: err.stack })
        toast.error(t('message.copy.failed'))
      }
    },
    [t]
  )

  const contextActions = useMemo<ImagePreviewAction[]>(
    () => [
      {
        icon: <CopyIcon className="size-4" />,
        id: 'copy-image',
        label: t('preview.copy.image'),
        onSelect: handleCopyImage
      }
    ],
    [handleCopyImage, t]
  )

  return (
    <ImagePreviewDialog
      actions={contextActions}
      className={imagePreviewLayer.className}
      items={[{ id: src, src }]}
      labels={labels}
      onOpenChange={handleVisibleChange}
      open={open}
      overlayClassName={imagePreviewLayer.overlayClassName}
    />
  )
}

const imagePreviewPopup = createPopup<{ src: string }, void>(ImagePreviewContainer, { dismissResult: undefined })

export type { ImageInput, ImagePreviewOptions }

/**
 * Image preview service — resolves any supported input to a URL and shows it in the
 * shared image preview dialog (a createPopup popup). "Opens a popup" is a services
 * concern; the popup rendering lives in ImagePreviewContainer via PopupHost.
 */
export class ImagePreviewService {
  static async show(input: ImageInput, options: ImagePreviewOptions = {}): Promise<void> {
    try {
      const src = await imageInputToPreviewUrl(input, options)
      await imagePreviewPopup.show({ src })
    } catch (error) {
      logger.error('Failed to show image preview:', error as Error)
      throw error
    }
  }
}
