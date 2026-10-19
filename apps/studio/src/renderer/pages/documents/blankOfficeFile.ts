import type { DocumentFileIconType } from '@renderer/components/icons/DocumentFileIcon'

import { BLANK_PRESENTATION_BASE64 } from './blankPresentationBytes'

/** The five kinds `@a3s-lab/office` can open. */
export const OFFICE_CREATE_KINDS = ['document', 'spreadsheet', 'presentation', 'pdf', 'markdown'] as const
export type OfficeCreateKind = (typeof OFFICE_CREATE_KINDS)[number]

export const OFFICE_CREATE_EXTENSION: Record<OfficeCreateKind, string> = {
  document: 'docx',
  spreadsheet: 'xlsx',
  presentation: 'pptx',
  pdf: 'pdf',
  markdown: 'md'
}

export const OFFICE_CREATE_LABEL_KEY: Record<OfficeCreateKind, string> = {
  document: 'documents.kind.doc',
  spreadsheet: 'documents.kind.sheet',
  presentation: 'documents.kind.presentation',
  pdf: 'documents.kind.pdf',
  markdown: 'documents.kind.markdown'
}

export const OFFICE_CREATE_TITLE_KEY: Record<OfficeCreateKind, string> = {
  document: 'documents.untitled.doc',
  spreadsheet: 'documents.untitled.sheet',
  presentation: 'documents.untitled.presentation',
  pdf: 'documents.untitled.pdf',
  markdown: 'documents.untitled.markdown'
}

export const OFFICE_CREATE_ICON: Record<OfficeCreateKind, DocumentFileIconType> = {
  document: 'word',
  spreadsheet: 'excel',
  presentation: 'slides',
  pdf: 'pdf',
  markdown: 'markdown'
}

const BLANK_PDF = `%PDF-1.4
1 0 obj
<< /Type /Catalog /Pages 2 0 R >>
endobj
2 0 obj
<< /Type /Pages /Kids [3 0 R] /Count 1 >>
endobj
3 0 obj
<< /Type /Page /Parent 2 0 R /MediaBox [0 0 595 842] /Contents 4 0 R /Resources << >> >>
endobj
4 0 obj
<< /Length 0 >>
stream

endstream
endobj
xref
0 5
0000000000 65535 f 
0000000009 00000 n 
0000000058 00000 n 
0000000115 00000 n 
0000000219 00000 n 
trailer
<< /Size 5 /Root 1 0 R >>
startxref
268
%%EOF
`

export function blankPdfBytes(): Uint8Array {
  return new TextEncoder().encode(BLANK_PDF)
}

function decodeBase64(value: string): Uint8Array {
  const binary = atob(value)
  const bytes = new Uint8Array(binary.length)
  for (let index = 0; index < binary.length; index += 1) bytes[index] = binary.charCodeAt(index)
  return bytes
}

/** Bytes a new office file must contain before the editor can open it. */
export async function blankOfficeFileBytes(kind: OfficeCreateKind): Promise<Uint8Array> {
  if (kind === 'pdf') return blankPdfBytes()
  if (kind === 'presentation') return decodeBase64(BLANK_PRESENTATION_BASE64)
  const template =
    kind === 'document' ? 'blank-document' : kind === 'spreadsheet' ? 'blank-spreadsheet' : 'blank-markdown'
  const { createArtifact, createArtifactBlob } = await import('@a3s-lab/office/core')
  const blob = await createArtifactBlob(createArtifact(template))
  return new Uint8Array(await blob.arrayBuffer())
}
