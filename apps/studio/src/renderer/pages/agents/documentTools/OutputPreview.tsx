import { useTranslation } from 'react-i18next'

import { isOfficePreviewKind, previewKind } from '@renderer/pages/documents/documentPreview'
import { OfficeDocumentSurface } from '@renderer/pages/documents/OfficeDocumentSurface'

import type { ToolOutput } from '../pdfToolsRun'

export function OutputPreview({ file, url }: { file: ToolOutput; url: string }) {
  const { t } = useTranslation()
  const kind = previewKind(file.name)
  const label = t('workbench.feature.pdf.tools.workspace.preview_of', { name: file.name })
  if (isOfficePreviewKind(kind)) {
    return (
      <div
        role="region"
        aria-label={label}
        data-workspace-file-editor
        data-ui="document-tools.office-preview"
        className="h-[65vh] min-h-[360px] w-full min-w-0 overflow-hidden">
        <OfficeDocumentSurface surface={kind} bytes={file.bytes} fileName={file.name} frame="workspace" preview />
      </div>
    )
  }
  if (file.mediaType.startsWith('image/'))
    return <img src={url} alt={label} className="max-h-[65vh] max-w-full object-contain" />
  if (file.mediaType.startsWith('text/'))
    return (
      <pre className="max-h-[65vh] overflow-auto text-[13px] break-words whitespace-pre-wrap">
        {new globalThis.TextDecoder().decode(file.bytes)}
      </pre>
    )
  return <p className="text-[13px] break-words">{file.name}</p>
}
