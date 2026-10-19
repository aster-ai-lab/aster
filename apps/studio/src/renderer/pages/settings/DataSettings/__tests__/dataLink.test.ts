import { describe, expect, it, vi } from 'vitest'

import {
  buildDataDirectives,
  channelQuery,
  channelReady,
  configuredChannels,
  dataPanelUsesToBot,
  dataVerb,
  emptyDataSnapshot,
  mentionedChannel,
  prepareToBotDataTurn,
  type DataSnapshot
} from '../dataLink'

function snapshot(patch: Partial<DataSnapshot> = {}): DataSnapshot {
  return { ...emptyDataSnapshot(), ...patch }
}

describe('ToBot data channels', () => {
  it('names the channel the user wrote and ignores a lookalike token', () => {
    expect(mentionedChannel('读取 Notion 里的会议')).toBe('notion')
    expect(dataVerb('读取 Notion 里的会议')).toBe('read')
    expect(channelQuery('读取 Notion 里的会议', 'notion')).toBe('里的会议')
    expect(mentionedChannel('保存到语雀')).toBe('yuque')
    expect(dataVerb('保存到语雀')).toBe('write')
    expect(mentionedChannel('打开思源笔记')).toBe('siyuan')
    expect(mentionedChannel('备份到 S3')).toBe('s3')
    expect(dataVerb('备份到 S3')).toBe('write')
    expect(mentionedChannel('class3')).toBeNull()
    expect(mentionedChannel('hello')).toBeNull()
    expect(dataVerb('hello')).toBeNull()
  })

  it('treats a channel as ready only when the required fields are present', () => {
    const ready = snapshot({ notionApiKey: 'secret-token-value', notionDatabaseId: 'db-1' })
    expect(channelReady('notion', ready)).toBe(true)
    expect(channelReady('yuque', ready)).toBe(false)
    expect(channelReady('webdav', snapshot({ webdavHost: 'https://dav.example' }))).toBe(true)
    expect(configuredChannels(ready)).toEqual(['notion'])
    expect(dataPanelUsesToBot('notion')).toBe(true)
    expect(dataPanelUsesToBot('data')).toBe(false)
    expect(dataPanelUsesToBot('import_settings')).toBe(false)
  })

  it('tells ToBot which channels are connected and never repeats a secret', () => {
    const ready = snapshot({
      notionApiKey: 'secret-token-value',
      notionDatabaseId: 'db-1',
      webdavPass: 'secret-token-value'
    })
    const listed = buildDataDirectives({ text: 'hello', snapshot: ready, excerpt: null, readFailed: false })
    expect(listed.write).toBeUndefined()
    expect(listed.directives.join('\n')).toContain('Notion')
    expect(listed.directives.join('\n')).not.toContain('secret-token-value')
    expect(listed.directives.join('\n')).not.toContain('WebDAV')

    const missing = buildDataDirectives({
      text: '读取 WebDAV',
      snapshot: ready,
      excerpt: null,
      readFailed: false
    })
    expect(missing.directives.join('\n')).toContain('WebDAV is not connected')
    expect(missing.directives.join('\n')).not.toContain('secret-token-value')
    expect(missing.write).toBeUndefined()
  })

  it('keeps a read excerpt and a write request inside one directive', async () => {
    const ready = snapshot({ notionApiKey: 'secret-token-value', notionDatabaseId: 'db-1' })
    const read = vi.fn(async () => '会议纪要 '.repeat(200))
    const loaded = await prepareToBotDataTurn('读取 Notion', {
      load: async () => ready,
      read
    })
    expect(read).toHaveBeenCalledOnce()
    expect(loaded.write).toBeUndefined()
    expect(loaded.directives.some((line) => line.startsWith('Notion read: 会议纪要'))).toBe(true)
    expect(loaded.directives.every((line) => line.length <= 500)).toBe(true)
    expect(loaded.directives.join('\n')).not.toContain('secret-token-value')

    const skipped = vi.fn(async () => 'should-not-read')
    const write = await prepareToBotDataTurn('保存到 Notion', {
      load: async () => ready,
      read: skipped
    })
    expect(skipped).not.toHaveBeenCalled()
    expect(write.write).toBe('notion')
    expect(write.directives.join('\n')).toContain('save this reply to Notion')
    expect(write.directives.join('\n')).not.toContain('secret-token-value')
  })

  it('says a read failed instead of inventing titles', () => {
    const ready = snapshot({ yuqueToken: 'secret-token-value', yuqueUrl: 'https://www.yuque.com/a/b' })
    const failed = buildDataDirectives({ text: '查看语雀', snapshot: ready, excerpt: null, readFailed: true })
    expect(failed.directives.join('\n')).toContain('语雀 read failed')
    expect(failed.directives.join('\n')).not.toContain('secret-token-value')
  })
})
