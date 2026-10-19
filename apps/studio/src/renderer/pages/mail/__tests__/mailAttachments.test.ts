import { describe, expect, it } from 'vitest'

import {
  ATTACHMENT_BYTES,
  ATTACHMENT_CAP,
  attachmentProblem,
  cleanAttachmentName,
  decodedBytes
} from '../mailAttachments'

const file = (name: string, data: string) => ({ name, mime: 'text/plain', data })

describe('mail attachments', () => {
  it('keeps the file name, and stops at the size and count limits', () => {
    expect(cleanAttachmentName('notes/计划.txt')).toBe('计划.txt')
    expect(cleanAttachmentName('../secret.txt')).toBe('secret.txt')
    expect(cleanAttachmentName('a/../../etc/passwd')).toBe('passwd')
    expect(cleanAttachmentName('   ')).toBeNull()
    expect(cleanAttachmentName(`${'a'.repeat(121)}.txt`)).toBeNull()

    const small = file('a.txt', 'aGk=')
    expect(decodedBytes('aGk=')).toBe(2)
    expect(attachmentProblem([], ATTACHMENT_BYTES + 1)).toBe('too_large')
    expect(
      attachmentProblem(
        Array.from({ length: ATTACHMENT_CAP }, () => small),
        2
      )
    ).toBe('too_many')
    expect(attachmentProblem([small], 2)).toBeNull()
  })
})
