import { visibleText } from './quote'

/** First line of a merged chat record. The rest is a title, then sender and text separated by a tab. */
export const CHAT_RECORD_MARK = '[[a3s-chat-record]]'

export type ChatRecordLine = { sender: string; text: string }

export type ChatRecord = { title: string; lines: ChatRecordLine[] }

function oneLine(value: string): string {
  return value.replace(/[\t\n\r]+/g, ' ').trim()
}

/** One text body a human chat can store. The thread renders it as a record card. */
export function encodeChatRecord(title: string, lines: readonly ChatRecordLine[]): string {
  const body = lines.flatMap((line) => {
    const sender = oneLine(line.sender)
    const text = oneLine(line.text)
    if (!sender || !text) return []
    return [`${sender}\t${text}`]
  })
  return [CHAT_RECORD_MARK, oneLine(title) || '…', ...body].join('\n')
}

/** Null when the text is an ordinary message. */
export function parseChatRecord(raw: string): ChatRecord | null {
  const rows = visibleText(raw).split('\n')
  if (rows[0] !== CHAT_RECORD_MARK) return null
  const title = oneLine(rows[1] ?? '')
  if (!title) return null
  const lines = rows.slice(2).flatMap((row) => {
    const tab = row.indexOf('\t')
    if (tab <= 0) return []
    const sender = row.slice(0, tab).trim()
    const text = row.slice(tab + 1).trim()
    if (!sender || !text) return []
    return [{ sender, text }]
  })
  if (lines.length === 0) return null
  return { title, lines }
}

/** Searchable body. An ordinary message keeps its visible text. */
export function readableChatText(raw: string): string {
  const record = parseChatRecord(raw)
  if (!record) return visibleText(raw)
  return record.lines.map((line) => `${line.sender}: ${line.text}`).join('\n')
}

/** Inbox preview uses the record title. Ordinary text returns null. */
export function chatRecordTitle(raw: string): string | null {
  return parseChatRecord(raw)?.title ?? null
}
