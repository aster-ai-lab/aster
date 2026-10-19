import { beforeEach, describe, expect, it, vi } from 'vitest'

import { dataApiService } from '@data/DataApiService'

import { isKnowledgeCompileWorkbenchRow, startKnowledgeCompileSession } from '../knowledgeCompileAgent'

vi.mock('@data/DataApiService', () => ({
  dataApiService: {
    get: vi.fn(),
    post: vi.fn()
  }
}))

const get = vi.mocked(dataApiService.get)
const post = vi.mocked(dataApiService.post)

beforeEach(() => {
  get.mockReset()
  post.mockReset()
})

describe('isKnowledgeCompileWorkbenchRow', () => {
  it('omits the compile session and the personal vault from the work page', () => {
    expect(isKnowledgeCompileWorkbenchRow({ description: 'knowledge-compile', path: '/notes' })).toBe(true)
    expect(isKnowledgeCompileWorkbenchRow({ path: '/workspace/wiki/local' })).toBe(true)
    expect(isKnowledgeCompileWorkbenchRow({ path: '/workspace/wiki/local/' })).toBe(true)
    expect(isKnowledgeCompileWorkbenchRow({ description: 'notes', path: '/workspace/proj' })).toBe(false)
  })
})

describe('startKnowledgeCompileSession', () => {
  it('opens an a3s code session on the vault', async () => {
    get.mockResolvedValue({
      items: [
        { id: 'a3s-code', type: 'a3s-code', createdAt: '2026-01-01T00:00:00.000Z' },
        { id: 'work', type: 'a3s-code', createdAt: '2026-01-02T00:00:00.000Z' },
        { id: 'other', type: 'claude-code', createdAt: '2026-01-01T00:00:00.000Z' }
      ],
      total: 3,
      page: 1
    } as never)
    post.mockImplementation(async (path) => {
      if (path === '/agent-workspaces') return { id: 'vault-workspace' } as never
      if (path === '/agent-sessions') return { id: 'session-1' } as never
      throw new Error(path)
    })

    await expect(startKnowledgeCompileSession('/workspace/wiki/local')).resolves.toEqual({
      sessionId: 'session-1',
      agentId: 'work'
    })
    expect(post).toHaveBeenNthCalledWith(1, '/agent-workspaces', {
      body: { path: '/workspace/wiki/local', name: '知识' }
    })
    expect(post).toHaveBeenNthCalledWith(2, '/agent-sessions', {
      body: {
        agentId: 'work',
        name: '知识编译',
        description: 'knowledge-compile',
        workspace: { type: 'user', workspaceId: 'vault-workspace' }
      }
    })
  })

  it('does not create a session when no a3s code agent exists', async () => {
    get.mockResolvedValue({
      items: [{ id: 'other', type: 'claude-code', createdAt: '2026-01-01T00:00:00.000Z' }],
      total: 1,
      page: 1
    } as never)
    await expect(startKnowledgeCompileSession('/workspace/wiki/local')).rejects.toThrow('no a3s code agent')
    expect(post).not.toHaveBeenCalled()
  })
})
