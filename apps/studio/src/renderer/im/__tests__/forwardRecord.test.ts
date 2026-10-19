import { describe, expect, it } from 'vitest'

import { CHAT_RECORD_MARK, encodeChatRecord, parseChatRecord, readableChatText } from '../forwardRecord'

describe('forward records', () => {
  const lines = [
    { sender: '周予安', text: 'hello 1' },
    { sender: '林知夏', text: 'hello\n2' }
  ]

  it('round-trips a merged record and hides the marker from readable text', () => {
    const raw = encodeChatRecord('林知夏和周予安的聊天记录', lines)
    expect(raw.startsWith(CHAT_RECORD_MARK)).toBe(true)
    expect(parseChatRecord(raw)).toEqual({
      title: '林知夏和周予安的聊天记录',
      lines: [
        { sender: '周予安', text: 'hello 1' },
        { sender: '林知夏', text: 'hello 2' }
      ]
    })
    expect(readableChatText(raw)).toBe('周予安: hello 1\n林知夏: hello 2')
    expect(readableChatText('plain')).toBe('plain')
    expect(parseChatRecord('plain')).toBeNull()
  })
})
