export const OFFICIAL_WRITING_EVENT = 'a3s:official-writing'

export const OFFICIAL_WRITING_CATEGORIES = [
  {
    id: 'downward',
    titleKey: 'workbench.writing.hub.category.downward',
    summaryKey: 'workbench.writing.hub.category.downward.summary'
  },
  {
    id: 'upward',
    titleKey: 'workbench.writing.hub.category.upward',
    summaryKey: 'workbench.writing.hub.category.upward.summary'
  },
  {
    id: 'parallel',
    titleKey: 'workbench.writing.hub.category.parallel',
    summaryKey: 'workbench.writing.hub.category.parallel.summary'
  },
  {
    id: 'meeting',
    titleKey: 'workbench.writing.hub.category.meeting',
    summaryKey: 'workbench.writing.hub.category.meeting.summary'
  },
  {
    id: 'general',
    titleKey: 'workbench.writing.hub.category.general',
    summaryKey: 'workbench.writing.hub.category.general.summary'
  }
] as const

export type OfficialWritingCategoryId = (typeof OFFICIAL_WRITING_CATEGORIES)[number]['id']

export const OFFICIAL_WRITING_TYPES = [
  {
    id: 'decision',
    categoryId: 'downward',
    title: '决定',
    summaryKey: 'workbench.writing.type.decision.summary',
    outline: ['决定依据与事项', '执行要求']
  },
  {
    id: 'notice',
    categoryId: 'downward',
    title: '通知',
    summaryKey: 'workbench.writing.type.notice.summary',
    outline: ['事由与依据', '具体事项', '有关要求']
  },
  {
    id: 'bulletin',
    categoryId: 'downward',
    title: '通报',
    summaryKey: 'workbench.writing.type.bulletin.summary',
    outline: ['情况概述', '分析评价', '希望要求']
  },
  {
    id: 'reply',
    categoryId: 'downward',
    title: '批复',
    summaryKey: 'workbench.writing.type.reply.summary',
    outline: ['批复意见', '执行要求']
  },
  {
    id: 'request',
    categoryId: 'upward',
    title: '请示',
    summaryKey: 'workbench.writing.type.request.summary',
    outline: ['请示缘由', '请示事项']
  },
  {
    id: 'report',
    categoryId: 'upward',
    title: '报告',
    summaryKey: 'workbench.writing.type.report.summary',
    outline: ['报告目的', '主要情况', '意见建议']
  },
  {
    id: 'letter',
    categoryId: 'parallel',
    title: '函',
    summaryKey: 'workbench.writing.type.letter.summary',
    outline: ['事由', '具体事项']
  },
  {
    id: 'opinion',
    categoryId: 'parallel',
    title: '意见',
    summaryKey: 'workbench.writing.type.opinion.summary',
    outline: ['背景与问题', '意见建议', '组织保障']
  },
  {
    id: 'minutes',
    categoryId: 'meeting',
    title: '纪要',
    summaryKey: 'workbench.writing.type.minutes.summary',
    outline: ['会议概况', '议定事项', '分工与时限']
  },
  {
    id: 'proposal',
    categoryId: 'meeting',
    title: '议案',
    summaryKey: 'workbench.writing.type.proposal.summary',
    outline: ['案由', '方案内容', '审议请求']
  },
  {
    id: 'general',
    categoryId: 'general',
    title: '通用公文',
    summaryKey: 'workbench.writing.type.general.summary',
    outline: ['正文']
  }
] as const

export type OfficialWritingTypeId = (typeof OFFICIAL_WRITING_TYPES)[number]['id']

export const WRITING_OPTIONS = [
  'banner',
  'copyNo',
  'secret',
  'docNo',
  'signer',
  'urgent',
  'attachment',
  'copyTo',
  'attendance',
  'amount',
  'seal',
  'note',
  'imprint'
] as const

export type WritingOption = (typeof WRITING_OPTIONS)[number]

export type WritingChoices = Partial<Record<WritingOption, boolean>>

export const WRITING_FIELDS = [
  'org',
  'copyNoText',
  'secretText',
  'urgency',
  'docNumber',
  'signerName',
  'subject',
  'mainTo',
  'attachmentText',
  'amountText',
  'signature',
  'issueDate',
  'noteText',
  'present',
  'absent',
  'observer',
  'copyToText',
  'printOrg',
  'printDate'
] as const

export type WritingField = (typeof WRITING_FIELDS)[number]

export type WritingFields = Partial<Record<WritingField, string>>

type OptionSpec = { id: WritingOption; defaultOn: boolean }

const SHARED_OPTIONS: readonly OptionSpec[] = [
  { id: 'copyNo', defaultOn: false },
  { id: 'secret', defaultOn: false },
  { id: 'seal', defaultOn: true },
  { id: 'note', defaultOn: false },
  { id: 'imprint', defaultOn: false }
]

function withShared(items: readonly OptionSpec[]): readonly OptionSpec[] {
  return [...items, ...SHARED_OPTIONS]
}

const TYPE_OPTIONS: Record<OfficialWritingTypeId, readonly OptionSpec[]> = {
  decision: withShared([
    { id: 'banner', defaultOn: true },
    { id: 'docNo', defaultOn: true },
    { id: 'urgent', defaultOn: false },
    { id: 'attachment', defaultOn: false },
    { id: 'copyTo', defaultOn: false }
  ]),
  notice: withShared([
    { id: 'banner', defaultOn: true },
    { id: 'docNo', defaultOn: true },
    { id: 'urgent', defaultOn: false },
    { id: 'attachment', defaultOn: false },
    { id: 'copyTo', defaultOn: false }
  ]),
  bulletin: withShared([
    { id: 'banner', defaultOn: true },
    { id: 'docNo', defaultOn: true },
    { id: 'urgent', defaultOn: false },
    { id: 'attachment', defaultOn: false },
    { id: 'copyTo', defaultOn: false }
  ]),
  reply: withShared([
    { id: 'banner', defaultOn: true },
    { id: 'docNo', defaultOn: true },
    { id: 'urgent', defaultOn: false },
    { id: 'attachment', defaultOn: false },
    { id: 'copyTo', defaultOn: false }
  ]),
  request: withShared([
    { id: 'banner', defaultOn: true },
    { id: 'docNo', defaultOn: true },
    { id: 'signer', defaultOn: true },
    { id: 'urgent', defaultOn: false },
    { id: 'amount', defaultOn: false },
    { id: 'attachment', defaultOn: false },
    { id: 'copyTo', defaultOn: false }
  ]),
  report: withShared([
    { id: 'banner', defaultOn: false },
    { id: 'docNo', defaultOn: false },
    { id: 'signer', defaultOn: false },
    { id: 'urgent', defaultOn: false },
    { id: 'attachment', defaultOn: false },
    { id: 'copyTo', defaultOn: false }
  ]),
  letter: withShared([
    { id: 'banner', defaultOn: false },
    { id: 'docNo', defaultOn: true },
    { id: 'urgent', defaultOn: false },
    { id: 'attachment', defaultOn: false },
    { id: 'copyTo', defaultOn: false }
  ]),
  opinion: withShared([
    { id: 'banner', defaultOn: true },
    { id: 'docNo', defaultOn: true },
    { id: 'urgent', defaultOn: false },
    { id: 'attachment', defaultOn: false },
    { id: 'copyTo', defaultOn: false }
  ]),
  minutes: withShared([
    { id: 'banner', defaultOn: true },
    { id: 'docNo', defaultOn: false },
    { id: 'urgent', defaultOn: false },
    { id: 'attendance', defaultOn: true },
    { id: 'attachment', defaultOn: false },
    { id: 'copyTo', defaultOn: false }
  ]),
  proposal: withShared([
    { id: 'banner', defaultOn: true },
    { id: 'docNo', defaultOn: true },
    { id: 'signer', defaultOn: false },
    { id: 'urgent', defaultOn: false },
    { id: 'attachment', defaultOn: false },
    { id: 'copyTo', defaultOn: false }
  ]),
  general: withShared([
    { id: 'banner', defaultOn: false },
    { id: 'docNo', defaultOn: false },
    { id: 'signer', defaultOn: false },
    { id: 'urgent', defaultOn: false },
    { id: 'attachment', defaultOn: false },
    { id: 'copyTo', defaultOn: false }
  ])
}

const ON_PHRASE: Record<WritingOption, string> = {
  banner: '使用红头',
  copyNo: '保留份号',
  secret: '标明密级和保密期限',
  docNo: '保留文号',
  signer: '保留签发人',
  urgent: '标明紧急',
  attachment: '列出附件',
  copyTo: '保留抄送',
  attendance: '列出出席、请假和列席',
  amount: '列明金额',
  seal: '加盖印章',
  note: '保留附注',
  imprint: '保留印发机关和印发日期'
}

const OFF_PHRASE: Record<WritingOption, string> = {
  banner: '不使用红头',
  copyNo: '不写份号',
  secret: '不标密级',
  docNo: '不写文号',
  signer: '不写签发人',
  urgent: '不标明紧急',
  attachment: '不列附件',
  copyTo: '不写抄送',
  attendance: '不列出出席、请假和列席',
  amount: '不列金额',
  seal: '不加盖印章',
  note: '不写附注',
  imprint: '不写印发机关和印发日期'
}

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

const ORDINALS = ['一', '二', '三', '四', '五', '六', '七', '八']

export function officialWritingType(id: string) {
  return OFFICIAL_WRITING_TYPES.find((item) => item.id === id) ?? null
}

export function officialWritingTypesIn(categoryId: OfficialWritingCategoryId) {
  return OFFICIAL_WRITING_TYPES.filter((item) => item.categoryId === categoryId)
}

export function writingOptionsFor(id: string): readonly OptionSpec[] {
  const type = officialWritingType(id)
  return type ? TYPE_OPTIONS[type.id] : []
}

export function writingChoiceOn(id: string, option: WritingOption, choices: WritingChoices = {}): boolean {
  const spec = writingOptionsFor(id).find((item) => item.id === option)
  if (!spec) return false
  return choices[option] ?? spec.defaultOn
}

function compact(lines: Array<string | null>): string {
  const out: string[] = []
  for (const line of lines) {
    if (line == null) continue
    if (line === '' && (out.length === 0 || out[out.length - 1] === '')) continue
    out.push(line)
  }
  while (out[0] === '') out.shift()
  while (out[out.length - 1] === '') out.pop()
  return out.join('\n')
}

function fieldText(fields: WritingFields, key: WritingField): string {
  return fields[key]?.trim() ?? ''
}

function orText(value: string, fallback: string): string {
  return value === '' ? fallback : value
}

function bannerLine(
  type: (typeof OFFICIAL_WRITING_TYPES)[number],
  choices: WritingChoices,
  fields: WritingFields
): string | null {
  if (!writingChoiceOn(type.id, 'banner', choices)) return null
  const org = orText(fieldText(fields, 'org'), '发文机关')
  return type.id === 'minutes' ? `${org}会议纪要` : `${org}文件`
}

export function officialWritingBannerLine(
  id: string,
  choices: WritingChoices = {},
  fields: WritingFields = {}
): string | null {
  const type = officialWritingType(id)
  return type ? bannerLine(type, choices, fields) : null
}

function documentLines(
  type: (typeof OFFICIAL_WRITING_TYPES)[number],
  note: string,
  choices: WritingChoices,
  fields: WritingFields
): Array<string | null> {
  const on = (option: WritingOption) => writingChoiceOn(type.id, option, choices)
  const orgName = orText(fieldText(fields, 'org'), '发文机关')
  const subject = orText(fieldText(fields, 'subject'), '事由')
  const sections = type.outline.map((topic, index) => `${ORDINALS[index]}、${topic}\n（待填写）`).join('\n\n')
  const docLine = [
    on('docNo') ? orText(fieldText(fields, 'docNumber'), '文号：待填写') : null,
    on('signer') ? `签发人：${orText(fieldText(fields, 'signerName'), '待填写')}` : null
  ]
    .filter(Boolean)
    .join('    ')
  const signature = orText(fieldText(fields, 'signature'), orgName)
  const date = orText(fieldText(fields, 'issueDate'), '成文日期')
  const main = fieldText(fields, 'mainTo')
  const mainLine = main === '' ? '主送机关：' : `${main}：`
  const tail = [
    on('note') ? `（${orText(fieldText(fields, 'noteText'), '附注：待填写')}）` : null,
    on('copyTo') ? '' : null,
    on('copyTo') ? `抄送：${orText(fieldText(fields, 'copyToText'), '待填写')}` : null,
    on('imprint') ? '' : null,
    on('imprint') ? `印发机关：${orText(fieldText(fields, 'printOrg'), '待填写')}` : null,
    on('imprint') ? `印发日期：${orText(fieldText(fields, 'printDate'), '待填写')}` : null
  ]
  const head = [
    on('copyNo') ? orText(fieldText(fields, 'copyNoText'), '000001') : null,
    on('secret') ? orText(fieldText(fields, 'secretText'), '机密★1年') : null,
    bannerLine(type, choices, fields),
    on('banner') ? '' : null,
    on('urgent') ? orText(fieldText(fields, 'urgency'), '紧急') : null,
    on('urgent') ? '' : null,
    docLine || null,
    docLine ? '' : null
  ]
  if (type.id === 'minutes') {
    return [
      ...head,
      `关于${subject}的会议纪要`,
      '',
      `会议听取了有关情况。${note}`,
      '',
      '议定以下事项：',
      '',
      sections,
      '',
      on('attendance') ? `出席：${orText(fieldText(fields, 'present'), '待填写')}` : null,
      on('attendance') ? `请假：${orText(fieldText(fields, 'absent'), '待填写')}` : null,
      on('attendance') ? `列席：${orText(fieldText(fields, 'observer'), '待填写')}` : null,
      date,
      ...tail
    ]
  }
  const title =
    type.id === 'proposal'
      ? `${orgName}关于提请审议${subject}的议案`
      : type.id === 'general'
        ? `${orgName}关于${subject}的公文`
        : `${orgName}关于${subject}的${type.title}`
  const amount = on('amount') ? `\n\n金额：${orText(fieldText(fields, 'amountText'), '待填写')}` : ''
  const lead =
    type.id === 'request'
      ? `为办理下列事项，现请示如下：\n\n${note}${amount}`
      : type.id === 'reply'
        ? `来文收悉。${note}\n\n现批复如下：`
        : type.id === 'report'
          ? `${note}\n\n现将有关情况报告如下：`
          : note
  return [
    ...head,
    title,
    '',
    mainLine,
    '',
    lead,
    '',
    sections,
    '',
    CLOSERS[type.id] ?? null,
    CLOSERS[type.id] ? '' : null,
    on('attachment') ? `附件：${orText(fieldText(fields, 'attachmentText'), '待填写')}` : null,
    on('attachment') ? '' : null,
    signature,
    date,
    ...tail
  ]
}

export function officialWritingDraft(
  id: string,
  brief: string,
  choices: WritingChoices = {},
  fields: WritingFields = {}
): string | null {
  const type = officialWritingType(id)
  const note = brief.trim()
  if (!type || note === '') return null
  return compact(documentLines(type, note, choices, fields))
}

export function officialWritingPreview(
  id: string,
  brief: string,
  choices: WritingChoices = {},
  fields: WritingFields = {}
): string | null {
  const type = officialWritingType(id)
  if (!type) return null
  const note = brief.trim()
  return compact(documentLines(type, note === '' ? '（待填写）' : note, choices, fields))
}

const TASK_NOTE = '\n\n写作说明：\n'

/** The on/off layout sentence for one document type. Empty when the type is unknown. */
export function officialWritingLayoutPhrase(id: string, choices: WritingChoices = {}): string {
  const type = officialWritingType(id)
  if (!type) return ''
  return writingOptionsFor(id)
    .map((spec) => (writingChoiceOn(type.id, spec.id, choices) ? ON_PHRASE[spec.id] : OFF_PHRASE[spec.id]))
    .join('，')
}

export function officialWritingTaskText(
  id: string,
  brief: string,
  choices: WritingChoices = {},
  fields: WritingFields = {}
): string | null {
  const type = officialWritingType(id)
  const draft = officialWritingDraft(id, brief, choices, fields)
  if (!type || !draft) return null
  const layout = officialWritingLayoutPhrase(id, choices)
  return `请按公文体例完成这份${type.title}。补上发文机关、事由和主送机关，保留层次和结束语。版式：${layout}。${TASK_NOTE}${brief.trim()}\n\n${draft}`
}

/** The draft body carried after the writing note. An instruction without that note has no document. */
export function officialWritingDocumentFromTask(task: string): string {
  const at = task.indexOf(TASK_NOTE)
  if (at < 0) return ''
  const rest = task.slice(at + TASK_NOTE.length)
  const gap = rest.indexOf('\n\n')
  if (gap < 0) return ''
  return rest.slice(gap + 2)
}

export interface OfficialWritingHandoff {
  text: string
  typeId?: string
  brief?: string
  choices?: WritingChoices
  fields?: WritingFields
}

/** The template the dialog configured. Absent when the event only carries the task sentence. */
export interface OfficialWritingCopyRequest {
  typeId: OfficialWritingTypeId
  brief: string
  choices: WritingChoices
  fields: WritingFields
}

export function isOfficialWritingHandoff(value: unknown): value is OfficialWritingHandoff {
  if (!value || typeof value !== 'object') return false
  const text = (value as { text?: unknown }).text
  return typeof text === 'string' && text.trim().length > 0 && text.length <= 8000
}

function recordOf(value: unknown): Record<string, unknown> | null {
  if (!value || typeof value !== 'object' || Array.isArray(value)) return null
  return value as Record<string, unknown>
}

/** Fields the dialog actually set. Unknown keys are dropped. */
export function officialWritingCopyRequest(value: OfficialWritingHandoff): OfficialWritingCopyRequest | null {
  const typeId = value.typeId
  const type = typeof typeId === 'string' ? officialWritingType(typeId) : null
  if (!type || typeof value.brief !== 'string') return null
  const choices: WritingChoices = {}
  const choiceRecord = recordOf(value.choices)
  if (choiceRecord) {
    for (const option of WRITING_OPTIONS) {
      if (typeof choiceRecord[option] === 'boolean') choices[option] = choiceRecord[option]
    }
  }
  const fields: WritingFields = {}
  const fieldRecord = recordOf(value.fields)
  if (fieldRecord) {
    for (const field of WRITING_FIELDS) {
      const next = fieldRecord[field]
      if (typeof next === 'string') fields[field] = next
    }
  }
  return { typeId: type.id, brief: value.brief, choices, fields }
}
