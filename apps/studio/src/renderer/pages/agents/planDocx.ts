import type { FileChild } from 'docx'

import {
  parsePlanMarkdown,
  planBumpHeading,
  planCaptionLabel,
  planFlowPages,
  planHeadingNumber,
  planListMark,
  planTocChunks,
  planTocEntries,
  resolvePlanImage,
  type PlanDraft,
  type PlanHeadingCounter
} from './planModel'

function picture(dataUrl: string): { type: 'png' | 'jpg' | 'gif'; data: Uint8Array } | null {
  const match = /^data:image\/(png|jpeg|gif);base64,([A-Za-z0-9+/=\r\n]+)$/.exec(dataUrl)
  if (!match) return null
  const binary = atob(match[2].replace(/\s/g, ''))
  const data = new Uint8Array(binary.length)
  for (let index = 0; index < binary.length; index += 1) data[index] = binary.charCodeAt(index)
  return { type: match[1] === 'jpeg' ? 'jpg' : (match[1] as 'png' | 'gif'), data }
}

/** Builds a Word document from the same blocks the preview uses. */
export async function planDocumentDocx(draft: PlanDraft): Promise<Uint8Array> {
  const docx = await import('docx')
  const children: FileChild[] = []
  const runs = (text: string) => {
    return text
      .split(/(\*\*[^*]+\*\*|`[^`]+`|\*[^*]+\*|<sup>[^<]*<\/sup>|<sub>[^<]*<\/sub>)/g)
      .filter((part) => part.length > 0)
      .map((part) => {
        if (part.startsWith('**') && part.endsWith('**') && part.length > 4) {
          return new docx.TextRun({ text: part.slice(2, -2), bold: true })
        }
        if (part.startsWith('`') && part.endsWith('`') && part.length > 2) {
          return new docx.TextRun({ text: part.slice(1, -1), font: 'Consolas' })
        }
        if (part.startsWith('<sup>') && part.endsWith('</sup>')) {
          return new docx.TextRun({ text: part.slice(5, -6), superScript: true })
        }
        if (part.startsWith('<sub>') && part.endsWith('</sub>')) {
          return new docx.TextRun({ text: part.slice(5, -6), subScript: true })
        }
        if (part.startsWith('*') && part.endsWith('*') && part.length > 2) {
          return new docx.TextRun({ text: part.slice(1, -1), italics: true })
        }
        return new docx.TextRun(part)
      })
  }
  const paragraph = (text: string, heading?: (typeof docx.HeadingLevel)[keyof typeof docx.HeadingLevel]) =>
    new docx.Paragraph({ heading, children: runs(text) })

  const logo = picture(draft.meta.logo)
  if (logo) {
    children.push(
      new docx.Paragraph({
        children: [
          new docx.ImageRun({
            type: logo.type,
            data: logo.data,
            transformation: { width: 48, height: 48 },
            altText: { name: draft.meta.title || 'logo', title: draft.meta.title || 'logo', description: '' }
          })
        ]
      })
    )
  }
  children.push(paragraph(draft.meta.title || 'Plan', docx.HeadingLevel.TITLE))
  if (draft.meta.subtitle) children.push(paragraph(draft.meta.subtitle))
  const facts = [draft.meta.number, draft.meta.version, draft.meta.date, draft.meta.author].filter(Boolean).join(' · ')
  if (facts) children.push(paragraph(facts))

  const flow = planFlowPages(draft.markdown, draft.style.pagination, draft.style.tableCaption, draft.style.h1Break)
  const tocEntries = draft.style.tocOn ? planTocEntries(flow, draft.style.tocDepth) : []
  const tocCount = draft.style.tocOn ? planTocChunks(tocEntries).length : 0
  const pageBreak = () => new docx.Paragraph({ children: [new docx.PageBreak()] })
  if (draft.style.tocOn) {
    if (draft.style.tocTitle) children.push(paragraph(draft.style.tocTitle, docx.HeadingLevel.HEADING_1))
    for (const entry of tocEntries) {
      const page = draft.style.tocPages ? '  ' + String(entry.sheet + 2 + tocCount) : ''
      children.push(paragraph(entry.text + page))
    }
  }
  let headingCounters: PlanHeadingCounter = [0, 0, 0, 0]
  let imageSeen = 0
  let tableSeen = 0
  let bodyStarted = false
  const blocks = parsePlanMarkdown(draft.markdown)
  for (let index = 0; index < blocks.length; index += 1) {
    const block = blocks[index]
    const next = blocks[index + 1]
    if (!block) continue
    if (block.kind === 'break') {
      children.push(pageBreak())
      continue
    }
    if (block.kind === 'p' && next?.kind === 'table' && draft.style.tableCaption) continue
    if (block.kind === 'h' && block.level === 1 && draft.style.h1Break && bodyStarted) children.push(pageBreak())
    bodyStarted = true
    if (block.kind === 'h') {
      headingCounters = planBumpHeading(headingCounters, block.level)
      const label = planHeadingNumber(draft.style.numbering, headingCounters, block.level)
      const heading =
        block.level === 1
          ? docx.HeadingLevel.HEADING_1
          : block.level === 2
            ? docx.HeadingLevel.HEADING_2
            : block.level === 3
              ? docx.HeadingLevel.HEADING_3
              : docx.HeadingLevel.HEADING_4
      children.push(paragraph(label ? label + ' ' + block.text : block.text, heading))
      continue
    }
    if (block.kind === 'p' || block.kind === 'quote' || block.kind === 'math') {
      children.push(paragraph(block.text))
      continue
    }
    if (block.kind === 'ul' || block.kind === 'ol') {
      block.items.forEach((item, itemIndex) => {
        children.push(paragraph(planListMark(block.kind, draft.style, itemIndex) + ' ' + item))
      })
      continue
    }
    if (block.kind === 'code') {
      children.push(new docx.Paragraph({ children: [new docx.TextRun({ text: block.text, font: 'Consolas' })] }))
      continue
    }
    if (block.kind === 'hr') {
      children.push(
        new docx.Paragraph({ border: { bottom: { style: docx.BorderStyle.SINGLE, size: 6, color: 'E5E5E5' } } })
      )
      continue
    }
    if (block.kind === 'table') {
      tableSeen += 1
      const previous = blocks[index - 1]
      const caption =
        draft.style.tableCaption && previous?.kind === 'p'
          ? planCaptionLabel(draft.style.tablePrefix, tableSeen, previous.text, draft.style.tableNumber)
          : ''
      if (caption && draft.style.tablePlace === 'above') children.push(paragraph(caption))
      children.push(
        new docx.Table({
          rows: block.rows.map(
            (row) =>
              new docx.TableRow({
                children: row.map(
                  (cell) =>
                    new docx.TableCell({
                      children: [paragraph(cell)]
                    })
                )
              })
          )
        })
      )
      if (caption && draft.style.tablePlace === 'below') children.push(paragraph(caption))
      continue
    }
    if (block.kind === 'image') {
      imageSeen += 1
      const src = resolvePlanImage(block.ref, draft.images)
      const image = src ? picture(src) : null
      const caption = draft.style.imageCaption
        ? planCaptionLabel(draft.style.imagePrefix, imageSeen, block.alt, draft.style.imageNumber)
        : ''
      if (image) {
        children.push(
          new docx.Paragraph({
            children: [
              new docx.ImageRun({
                type: image.type,
                data: image.data,
                transformation: { width: 460, height: 280 },
                altText: { name: block.alt || 'image', title: block.alt || 'image', description: block.alt }
              })
            ]
          })
        )
      }
      if (caption) children.push(paragraph(caption))
      else if (!image && block.alt) children.push(paragraph(block.alt))
    }
  }

  const file = new docx.Document({ title: draft.meta.title || 'Plan', sections: [{ children }] })
  return new Uint8Array(await docx.Packer.toArrayBuffer(file))
}
