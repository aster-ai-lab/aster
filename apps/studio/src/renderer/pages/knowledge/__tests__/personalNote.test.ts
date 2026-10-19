import { describe, expect, it } from 'vitest'

import { isPersonalNotePath } from '../knowledgeVault'
import { personalNoteStem } from '../personalNote'

describe('personalNoteStem', () => {
  it('keeps a short message title and drops characters a file name cannot use', () => {
    expect(personalNoteStem('工作区有哪些文件？', '未命名.md')).toBe('工作区有哪些文件？')
    expect(personalNoteStem('a/b:c', '未命名.md')).toBe('a b c')
    expect(personalNoteStem('   ', '未命名.md')).toBe('未命名')
  })
})

describe('isPersonalNotePath', () => {
  it('accepts a file inside the personal vault only', () => {
    expect(isPersonalNotePath('/workspace', '/workspace/wiki/local/甲.md')).toBe(true)
    expect(isPersonalNotePath('/workspace', '/workspace/wiki/cloud/甲.md')).toBe(false)
    expect(isPersonalNotePath('/workspace', '/workspace/wiki/local/../甲.md')).toBe(false)
  })
})
