import type { DocumentContent } from '@a3s-lab/office/core'
import { DocumentEditor } from '@a3s-lab/office/react'
import { useCallback, useEffect, useLayoutEffect, useMemo, useRef } from 'react'

import '@a3s-lab/office/styles.css'

import type { OfficialWritingTypeId, WritingChoices, WritingFields } from './officialWriting'
import {
  createOfficialWritingPreviewCache,
  syncOfficialWritingPreview,
  type OfficialWritingPreviewEditor
} from './officialWritingPreviewSync'
import { officialWritingTemplateHtml } from './officialWritingTemplate'

const PAGE_WIDTH_PX = 794

function keepDocument(content: DocumentContent) {
  return content
}

function fitZoom(width: number): string | null {
  if (width < 80) return null
  const zoom = Math.min(1, Math.max(0.5, (width - 8) / PAGE_WIDTH_PX))
  return zoom.toFixed(3)
}

export function OfficialWritingWordPreview({
  typeId,
  brief,
  choices,
  fields
}: {
  typeId: OfficialWritingTypeId
  brief: string
  choices: WritingChoices
  fields: WritingFields
}) {
  const structureHtml = useMemo(() => officialWritingTemplateHtml(typeId, '', choices, {}), [typeId, choices])
  const filledHtml = useMemo(
    () => officialWritingTemplateHtml(typeId, brief, choices, fields),
    [typeId, brief, choices, fields]
  )
  // The office editor replaces the whole document when `content` changes.
  // Field edits patch placeholder ranges, so this value stays on the skeleton.
  const content = useMemo<DocumentContent | null>(() => {
    if (!structureHtml) return null
    return {
      type: 'document',
      html: structureHtml,
      pageSize: 'a4',
      orientation: 'portrait',
      showPageNumbers: false
    }
  }, [structureHtml])

  const hostRef = useRef<HTMLDivElement>(null)
  const editorRef = useRef<OfficialWritingPreviewEditor | null>(null)
  const cacheRef = useRef(createOfficialWritingPreviewCache())
  const structureRef = useRef(structureHtml ?? '')
  const filledRef = useRef(filledHtml ?? '')
  const fieldsRef = useRef(fields)
  const briefRef = useRef(brief)
  structureRef.current = structureHtml ?? ''
  filledRef.current = filledHtml ?? ''
  fieldsRef.current = fields
  briefRef.current = brief

  const pushPreview = useCallback((editor: OfficialWritingPreviewEditor) => {
    const structure = structureRef.current
    const filled = filledRef.current
    if (!structure || !filled) return
    const scroller = hostRef.current?.querySelector('.work-document-scroll')
    const top = scroller instanceof HTMLElement ? scroller.scrollTop : 0
    syncOfficialWritingPreview(editor, {
      brief: briefRef.current,
      cache: cacheRef.current,
      fields: fieldsRef.current,
      filledHtml: filled,
      structureHtml: structure
    })
    if (scroller instanceof HTMLElement) scroller.scrollTop = top
  }, [])

  const onEditorReady = useCallback(
    (editor: OfficialWritingPreviewEditor) => {
      editorRef.current = editor
      cacheRef.current = createOfficialWritingPreviewCache()
      pushPreview(editor)
    },
    [pushPreview]
  )

  useEffect(() => {
    const editor = editorRef.current
    if (!editor || !structureHtml || !filledHtml) return
    pushPreview(editor)
  }, [brief, choices, fields, filledHtml, pushPreview, structureHtml])

  useLayoutEffect(() => {
    const host = hostRef.current
    if (!host) return
    const apply = () => {
      const zoom = fitZoom(host.clientWidth)
      if (zoom) host.style.setProperty('--writing-fit-zoom', zoom)
    }
    apply()
    if (typeof ResizeObserver === 'undefined') return
    const observer = new ResizeObserver(apply)
    observer.observe(host)
    return () => observer.disconnect()
  }, [])

  if (!content) return null
  return (
    <div
      ref={hostRef}
      data-ui="workbench.writing.live"
      data-writing-live="fit"
      className="writing-word-preview flex min-h-0 flex-1 flex-col overflow-hidden">
      <DocumentEditor
        key={typeId}
        content={content}
        preview
        hideCommentsPanel
        className="min-h-0 flex-1"
        onChange={keepDocument}
        onEditorReady={onEditorReady}
      />
    </div>
  )
}
