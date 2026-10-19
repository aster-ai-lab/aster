import {
  officialWritingType,
  writingChoiceOn,
  type OfficialWritingTypeId,
  type WritingChoices,
  type WritingFields
} from './officialWriting'

/**
 * GB/T 9704-shaped Word HTML for one 文种.
 * An empty slot shows its Chinese name inside a dashed character border.
 * A filled field replaces that text and drops the border. The writing note replaces the first body slot.
 */

const FONTS = {
  displaySong: "STSong, 'Songti SC', SimSun, serif",
  fangsong: 'FangSong, STFangsong, serif',
  hei: "SimHei, 'Heiti SC', sans-serif",
  kai: "KaiTi, 'Kaiti SC', serif"
} as const

type FontName = keyof typeof FONTS

const RED = '#FF0000'
const SIZE_BANNER = 48
const SIZE_TITLE = 22
const SIZE_BODY = 16
const SIZE_NOTE = 14
const LINE = 28.99

const CLOSERS: Partial<Record<OfficialWritingTypeId, string>> = {
  decision: '本决定自发布之日起施行。',
  notice: '特此通知。',
  bulletin: '特此通报。',
  reply: '此复。',
  request: '妥否，请批示。',
  report: '特此报告。',
  letter: '特此函达，请予支持为荷。',
  opinion: '以上意见，请结合实际认真贯彻执行。',
  proposal: '以上议案，请予审议。'
}

const FIELD_KEYS: Partial<Record<keyof WritingFields, string>> = {
  org: 'issuer',
  subject: 'subject',
  mainTo: 'recipient',
  docNumber: 'doc_number',
  signerName: 'signer',
  attachmentText: 'attachment',
  signature: 'signature',
  issueDate: 'date',
  copyToText: 'cc',
  printOrg: 'publisher',
  printDate: 'print_date',
  present: 'attendees',
  absent: 'absentees',
  observer: 'observers',
  copyNoText: 'copy_no',
  secretText: 'secret',
  urgency: 'urgency',
  amountText: 'amount',
  noteText: 'note'
}

type Kind = 'file' | 'upward' | 'letter' | 'minutes' | 'proposal'

function escapeHtml(value: string): string {
  return value.replaceAll('&', '&amp;').replaceAll('<', '&lt;').replaceAll('>', '&gt;')
}

/** Chinese names shown in an empty slot. The map key stays the field id the preview patches. */
const SLOT_LABELS: Record<string, string> = {
  issuer: '发文机关',
  subject: '事由',
  recipient: '主送机关',
  doc_number: '发文字号',
  signer: '签发人',
  attachment: '附件',
  signature: '发文机关署名',
  date: '成文日期',
  cc: '抄送机关',
  publisher: '印发机关',
  print_date: '印发日期',
  attendees: '出席',
  absentees: '请假',
  observers: '列席',
  copy_no: '份号',
  secret: '密级和保密期限',
  urgency: '紧急程度',
  amount: '金额',
  note: '附注',
  doc_kind: '文种',
  letter_lead: '起首语',
  incoming_subject: '来文事由',
  incoming_doc_number: '来文字号',
  purpose: '请示目的',
  meeting_time: '会议时间',
  meeting_heard: '听取内容'
}

const LABEL_TO_KEY = new Map(Object.entries(SLOT_LABELS).map(([key, label]) => [label, key]))
const SECTION_MARK = '待填写：'
const PLACEHOLDER_LABELS = Object.values(SLOT_LABELS).sort((left, right) => right.length - left.length)

/** Fixed captions that share a slot name and keep the following colon. */
const STATIC_CAPTION = new Set(['签发人', '出席', '请假', '列席', '附件', '金额'])

/** Serialized character border the office editor paints as a dashed box. */
export const OFFICIAL_WRITING_SLOT_BORDER = '{"style":"dashed","color":{"value":"#595959"},"size":6,"space":1}'

/** Visible text for an empty slot. Body sections keep the topic so two blanks stay distinct. */
export function officialWritingSlotPlaceholder(key: string): string {
  if (key.startsWith('section.')) return `${SECTION_MARK}${key.slice('section.'.length)}`
  return SLOT_LABELS[key] ?? '待填写'
}

/** Map a visible placeholder back to the slot id the field table uses. */
export function officialWritingSlotKey(label: string): string {
  if (label.startsWith(SECTION_MARK)) return `section.${label.slice(SECTION_MARK.length)}`
  return LABEL_TO_KEY.get(label) ?? label
}

/**
 * Placeholder ranges in one text node. Longer names win, so 事由 does not match inside 来文事由.
 * A section heading such as 事由与依据, and a caption such as 签发人：, are not slots.
 */
export function officialWritingFindSlots(text: string): Array<{ key: string; index: number; text: string }> {
  const occupied = new Array<boolean>(text.length).fill(false)
  const found: Array<{ key: string; index: number; text: string }> = []
  const claim = (index: number, label: string) => {
    const end = index + label.length
    for (let cursor = index; cursor < end; cursor += 1) {
      if (occupied[cursor]) return
    }
    const next = text[end]
    const prev = index > 0 ? text[index - 1] : ''
    if (label === '事由' && (next === '与' || prev === '、')) return
    if (next === '：' && STATIC_CAPTION.has(label)) return
    occupied.fill(true, index, end)
    found.push({ key: officialWritingSlotKey(label), index, text: label })
  }
  const sections = /待填写：[^\s。，；、\n]+/g
  let section: RegExpExecArray | null
  while ((section = sections.exec(text))) claim(section.index, section[0])
  for (const label of PLACEHOLDER_LABELS) {
    let from = 0
    while (from < text.length) {
      const index = text.indexOf(label, from)
      if (index < 0) break
      claim(index, label)
      from = index + label.length
    }
  }
  found.sort((left, right) => left.index - right.index)
  return found
}

type Inline = { text: string } | { slot: string }

function plain(text: string): Inline {
  return { text }
}

function blank(key: string): Inline {
  return { slot: key }
}

function titleFor(id: OfficialWritingTypeId): Inline[] {
  const issuer = blank('issuer')
  const subject = blank('subject')
  switch (id) {
    case 'decision':
      return [issuer, plain('关于'), subject, plain('的决定')]
    case 'notice':
      return [issuer, plain('关于'), subject, plain('的通知')]
    case 'bulletin':
      return [issuer, plain('关于'), subject, plain('的通报')]
    case 'reply':
      return [issuer, plain('关于'), subject, plain('的批复')]
    case 'request':
      return [issuer, plain('关于'), subject, plain('的请示')]
    case 'report':
      return [issuer, plain('关于'), subject, plain('的报告')]
    case 'letter':
      return [issuer, plain('关于'), subject, plain('的函')]
    case 'opinion':
      return [issuer, plain('关于'), subject, plain('的意见')]
    case 'minutes':
      return [plain('关于'), subject, plain('的会议纪要')]
    case 'proposal':
      return [issuer, plain('关于提请审议'), subject, plain('的议案')]
    case 'general':
      return [issuer, plain('关于'), subject, plain('的'), blank('doc_kind')]
  }
}

function kindOf(id: OfficialWritingTypeId): Kind {
  if (id === 'request' || id === 'report') return 'upward'
  if (id === 'letter') return 'letter'
  if (id === 'minutes') return 'minutes'
  if (id === 'proposal') return 'proposal'
  return 'file'
}

function run(text: string, font: FontName, sizePt: number, color?: string): string {
  return runHtml(text.length > 0 ? escapeHtml(text) : '&#160;', font, sizePt, color)
}

function runHtml(html: string, font: FontName, sizePt: number, color?: string): string {
  const colorCss = color ? `;color: ${color}` : ''
  return `<span style="font-family: ${FONTS[font]};font-size: ${sizePt}pt${colorCss}">${html}</span>`
}

function placeholderRun(key: string, font: FontName, sizePt: number, color?: string): string {
  const label = escapeHtml(officialWritingSlotPlaceholder(key))
  const border =
    'border: 1px dashed #595959; padding: 1.333px; box-decoration-break: clone; -webkit-box-decoration-break: clone'
  const inner = `<span data-writing-slot="${escapeHtml(key)}" data-office-run-border='${OFFICIAL_WRITING_SLOT_BORDER}' style="${border}">${label}</span>`
  return runHtml(inner, font, sizePt, color)
}

function line(
  parts: readonly Inline[],
  font: FontName,
  sizePt: number,
  options: Parameters<typeof paragraph>[1] & { color?: string } = {}
): string {
  const inner = parts
    .map((part) =>
      'slot' in part
        ? placeholderRun(part.slot, font, sizePt, options.color)
        : run(part.text, font, sizePt, options.color)
    )
    .join('')
  return paragraph(inner, options)
}

function paragraph(
  inner: string,
  options: {
    align?: 'left' | 'center' | 'right'
    before?: number
    after?: number
    line?: number
    indent?: boolean
    border?: string
  } = {}
): string {
  const styles = [
    options.align ? `text-align: ${options.align}` : '',
    options.line ? `line-height: ${options.line}pt` : '',
    options.before !== undefined ? `margin-top: ${options.before}pt` : '',
    options.after !== undefined ? `margin-bottom: ${options.after}pt` : '',
    options.indent ? `text-indent: ${SIZE_BODY * 2}pt` : '',
    options.border ? `border-bottom: ${options.border}` : ''
  ].filter(Boolean)
  const style = styles.length ? ` style="${styles.join('; ')}"` : ''
  return `<p${style}>${inner}</p>`
}

function textParagraph(
  text: string,
  font: FontName,
  sizePt: number,
  options: Parameters<typeof paragraph>[1] & { color?: string } = {}
): string {
  return paragraph(run(text, font, sizePt, options.color), options)
}

function section(body: string): string {
  const box = { top: 14, right: 16, bottom: 12, left: 18 }
  return `<section data-document-section="" data-section-id="document-section-1" data-section-page-size="a4" data-section-orientation="portrait" data-section-margin-top="${box.top}" data-section-margin-right="${box.right}" data-section-margin-bottom="${box.bottom}" data-section-margin-left="${box.left}" data-section-show-page-numbers="false" data-section-document-grid-type="linesAndChars" data-section-document-grid-line-pitch="${LINE}">${body}</section>`
}

const ORDINALS = ['一', '二', '三', '四', '五', '六', '七', '八', '九', '十'] as const

function chineseOrdinal(index: number): string {
  return ORDINALS[index] ?? String(index + 1)
}

const KEY_TO_FIELD = new Map<string, keyof WritingFields>(
  (Object.entries(FIELD_KEYS) as Array<[keyof WritingFields, string]>).map(([field, key]) => [key, field])
)

/** Text that should stand in one slot. An empty field keeps the Chinese mark. */
export function officialWritingSlotText(
  key: string,
  fields: WritingFields,
  brief: string,
  claim: { used: boolean }
): string {
  const token = officialWritingSlotPlaceholder(key)
  if (key.startsWith('section.')) {
    const note = brief.trim()
    if (!claim.used && note) {
      claim.used = true
      return note
    }
    return token
  }
  const field = KEY_TO_FIELD.get(key)
  const value = field ? (fields[field]?.trim() ?? '') : ''
  return value || token
}

function applyFields(html: string, fields: WritingFields): string {
  const values = new Map<string, string>()
  for (const [field, key] of Object.entries(FIELD_KEYS) as Array<[keyof WritingFields, string]>) {
    const value = fields[field]?.trim() ?? ''
    if (key && value) values.set(key, value)
  }
  if (values.size === 0) return html
  return html.replace(SLOT_SPAN, (token, key: string) => {
    const value = values.get(key)
    return value === undefined ? token : escapeHtml(value)
  })
}

const SLOT_SPAN = /<span\b[^>]*\bdata-writing-slot="([^"]+)"[^>]*>[\s\S]*?<\/span>/g

function applyBrief(html: string, brief: string): string {
  const note = brief.trim()
  if (!note) return html
  let used = false
  return html.replace(SLOT_SPAN, (token, key: string) => {
    if (used || !key.startsWith('section.')) return token
    used = true
    return escapeHtml(note)
  })
}

export function officialWritingTemplateHtml(
  id: string,
  brief = '',
  choices: WritingChoices = {},
  fields: WritingFields = {}
): string | null {
  const type = officialWritingType(id)
  if (!type) return null
  const kind = kindOf(type.id)
  const on = (option: Parameters<typeof writingChoiceOn>[1]) => writingChoiceOn(type.id, option, choices)
  const blocks: string[] = []

  if (on('copyNo')) blocks.push(line([blank('copy_no')], 'fangsong', SIZE_BODY, { align: 'left', line: LINE }))
  if (on('secret')) blocks.push(line([blank('secret')], 'hei', SIZE_BODY, { align: 'left', line: LINE }))
  if (on('banner')) {
    const banner =
      kind === 'letter'
        ? [blank('issuer')]
        : kind === 'minutes'
          ? [blank('issuer'), plain('纪要')]
          : [blank('issuer'), plain('文件')]
    blocks.push(
      line(banner, 'displaySong', SIZE_BANNER, {
        align: 'center',
        color: RED,
        before: 0,
        after: 8,
        line: SIZE_BANNER
      })
    )
    blocks.push(
      paragraph(run('', 'fangsong', 4), {
        border: '1.5pt solid #FF0000',
        before: 0,
        after: 0,
        line: 4
      })
    )
  }
  if (on('urgent')) blocks.push(line([blank('urgency')], 'hei', SIZE_BODY, { align: 'right', line: LINE }))
  if (on('docNo') || (kind === 'upward' && on('signer'))) {
    if (kind === 'upward' && on('signer')) {
      const number = on('docNo') ? placeholderRun('doc_number', 'fangsong', SIZE_BODY) : ''
      const signer = `${run('签发人：', 'fangsong', SIZE_BODY)}${placeholderRun('signer', 'kai', SIZE_BODY)}`
      blocks.push(
        paragraph(`${number}${signer}`, { align: on('docNo') ? 'left' : 'right', before: 0, after: 11, line: LINE })
      )
    } else if (on('docNo')) {
      blocks.push(
        line([blank('doc_number')], 'fangsong', SIZE_BODY, {
          align: kind === 'letter' ? 'right' : 'center',
          before: kind === 'letter' ? 14 : 0,
          after: kind === 'letter' ? 0 : 11,
          line: LINE
        })
      )
    }
  }

  blocks.push(
    line(titleFor(type.id), 'displaySong', SIZE_TITLE, {
      align: 'center',
      before: 12,
      after: 8,
      line: LINE
    })
  )

  if (kind !== 'minutes') {
    const recipient =
      kind === 'proposal' ? [blank('recipient'), plain('人民代表大会常务委员会：')] : [blank('recipient'), plain('：')]
    blocks.push(line(recipient, 'fangsong', SIZE_BODY, { align: 'left', line: LINE }))
  }

  if (type.id === 'reply') {
    blocks.push(
      line(
        [
          plain('你单位《关于'),
          blank('incoming_subject'),
          plain('的请示》（'),
          blank('incoming_doc_number'),
          plain('）收悉。经研究，现批复如下：')
        ],
        'fangsong',
        SIZE_BODY,
        { indent: true, line: LINE }
      )
    )
  }
  if (type.id === 'request') {
    blocks.push(
      line([plain('为'), blank('purpose'), plain('，现请示如下：')], 'fangsong', SIZE_BODY, {
        indent: true,
        line: LINE
      })
    )
  }
  if (type.id === 'report') {
    blocks.push(textParagraph('现将有关情况报告如下：', 'fangsong', SIZE_BODY, { indent: true, line: LINE }))
  }
  if (type.id === 'letter') {
    blocks.push(line([blank('letter_lead'), plain('。')], 'fangsong', SIZE_BODY, { indent: true, line: LINE }))
  }
  if (type.id === 'minutes') {
    blocks.push(
      line(
        [
          plain('会议于'),
          blank('meeting_time'),
          plain('召开。会议听取了'),
          blank('meeting_heard'),
          plain('，议定以下事项：')
        ],
        'fangsong',
        SIZE_BODY,
        { indent: true, line: LINE }
      )
    )
  }

  type.outline.forEach((topic, index) => {
    blocks.push(textParagraph(`${chineseOrdinal(index)}、${topic}`, 'hei', SIZE_BODY, { indent: true, line: LINE }))
    blocks.push(line([blank(`section.${topic}`), plain('。')], 'fangsong', SIZE_BODY, { indent: true, line: LINE }))
  })

  const closer = CLOSERS[type.id]
  if (closer) blocks.push(textParagraph(closer, 'fangsong', SIZE_BODY, { indent: true, line: LINE }))
  if (on('amount'))
    blocks.push(
      line([plain('金额：'), blank('amount')], 'fangsong', SIZE_BODY, { indent: true, before: LINE, line: LINE })
    )
  if (on('attachment')) {
    blocks.push(
      line([plain('附件：'), blank('attachment')], 'fangsong', SIZE_BODY, { indent: true, before: LINE, line: LINE })
    )
  }
  if (kind === 'minutes' && on('attendance')) {
    blocks.push(
      paragraph(`${run('出席：', 'hei', SIZE_BODY)}${placeholderRun('attendees', 'fangsong', SIZE_BODY)}`, {
        indent: true,
        before: LINE,
        line: LINE
      })
    )
    blocks.push(
      paragraph(`${run('请假：', 'hei', SIZE_BODY)}${placeholderRun('absentees', 'fangsong', SIZE_BODY)}`, {
        indent: true,
        line: LINE
      })
    )
    blocks.push(
      paragraph(`${run('列席：', 'hei', SIZE_BODY)}${placeholderRun('observers', 'fangsong', SIZE_BODY)}`, {
        indent: true,
        line: LINE
      })
    )
  }
  blocks.push(line([blank('signature')], 'fangsong', SIZE_BODY, { align: 'right', before: LINE, line: LINE }))
  blocks.push(line([blank('date')], 'fangsong', SIZE_BODY, { align: 'right', line: LINE }))
  if (on('note'))
    blocks.push(line([plain('（'), blank('note'), plain('）')], 'fangsong', SIZE_NOTE, { indent: true, line: LINE }))
  if (on('copyTo') || on('imprint')) {
    blocks.push(paragraph(run('', 'fangsong', 4), { border: '0.35pt solid #000000', before: LINE, line: 4 }))
    if (on('copyTo'))
      blocks.push(line([plain('抄送：'), blank('cc'), plain('。')], 'fangsong', SIZE_NOTE, { line: LINE }))
    if (on('imprint')) {
      blocks.push(
        paragraph(
          `${placeholderRun('publisher', 'fangsong', SIZE_NOTE)}${placeholderRun('print_date', 'fangsong', SIZE_NOTE)}${run('印发', 'fangsong', SIZE_NOTE)}`,
          { align: 'left', line: LINE, border: '0.35pt solid #000000' }
        )
      )
    }
  }

  return applyBrief(applyFields(section(blocks.join('')), fields), brief)
}
