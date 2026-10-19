import { describe, expect, it, vi } from 'vitest'

import {
  codeSessionDocument,
  ensureCodeSessionDirectory,
  inboxSessionDirectory,
  localDateFolder,
  moveCodeSessionDirectory,
  moveCodeSessionToInbox,
  placeTaskInInbox,
  placeTaskInProject,
  projectDirectory,
  projectSessionDirectory,
  workspaceEntryName,
  type WorkspaceFiles
} from '../workbenchWorkspace'

const root = '/Users/roylin/.a3s'
const day = new Date(2026, 9, 2, 23, 30)
const sessionId = '6f1c1b2a-3d4e-4f50-8a61-72b3c4d5e6f7'

function files(overrides: Partial<WorkspaceFiles> = {}): WorkspaceFiles & {
  mkdir: ReturnType<typeof vi.fn>
  write: ReturnType<typeof vi.fn>
  moveDir: ReturnType<typeof vi.fn>
} {
  return {
    mkdir: vi.fn(async () => undefined),
    write: vi.fn(async () => undefined),
    moveDir: vi.fn(async () => undefined),
    ...overrides
  }
}

describe('workbench workspace layout', () => {
  it('uses the local calendar day and a single path segment', () => {
    expect(localDateFolder(day)).toBe('2026-10-02')
    expect(localDateFolder(new Date(Number.NaN))).toBeNull()
    expect(workspaceEntryName('  测试  ')).toBe('测试')
    expect(workspaceEntryName('..')).toBeNull()
    expect(workspaceEntryName('a/b')).toBeNull()
    expect(workspaceEntryName('a\\b')).toBeNull()
    expect(workspaceEntryName('.')).toBeNull()
  })

  it('creates a same-named project folder and a dated session folder', () => {
    expect(projectDirectory(root, '测试')).toBe('/Users/roylin/.a3s/projects/测试')
    expect(projectDirectory(root, '../outside')).toBeNull()
    expect(inboxSessionDirectory(root, sessionId, day)).toBe(`/Users/roylin/.a3s/sessions/2026-10-02/${sessionId}`)
    expect(projectSessionDirectory('/Users/roylin/.a3s/projects/测试', sessionId)).toBe(
      `/Users/roylin/.a3s/projects/测试/${sessionId}`
    )
    expect(inboxSessionDirectory(root, '../nope', day)).toBeNull()
  })

  it('writes the a3s-code session id into the inbox directory', async () => {
    const api = files()
    const directory = await ensureCodeSessionDirectory(api, {
      root,
      sessionId,
      createdAt: day.toISOString()
    })

    expect(directory).toBe(`/Users/roylin/.a3s/sessions/2026-10-02/${sessionId}`)
    expect(api.mkdir).toHaveBeenCalledWith(`${directory}/meta`)
    expect(api.write).toHaveBeenCalledWith(
      `${directory}/meta/a3s-session.json`,
      codeSessionDocument(sessionId, directory!, day.toISOString())
    )
    expect(JSON.parse(String(api.write.mock.calls[0]?.[1]))).toMatchObject({
      runtime: 'a3s-code',
      id: sessionId,
      store: directory
    })
  })

  it('puts a task created in a project directly under that project', async () => {
    const api = files()
    const project = '/Users/roylin/.a3s/projects/测试'
    const directory = await ensureCodeSessionDirectory(api, {
      root,
      sessionId,
      createdAt: day.toISOString(),
      projectPath: project
    })

    expect(directory).toBe(`${project}/${sessionId}`)
    expect(api.moveDir).not.toHaveBeenCalled()
  })

  it('moves the inbox session folder into the project', async () => {
    const api = files()
    const moved = await moveCodeSessionDirectory(api, {
      root,
      sessionId,
      createdAt: day.toISOString(),
      fromProjectPath: null,
      toProjectPath: '/Users/roylin/.a3s/projects/测试'
    })

    expect(api.mkdir).toHaveBeenCalledWith('/Users/roylin/.a3s/projects/测试')
    expect(api.moveDir).toHaveBeenCalledWith(
      `/Users/roylin/.a3s/sessions/2026-10-02/${sessionId}`,
      `/Users/roylin/.a3s/projects/测试/${sessionId}`
    )
    expect(moved).toEqual({
      directory: `/Users/roylin/.a3s/projects/测试/${sessionId}`,
      movedFrom: `/Users/roylin/.a3s/sessions/2026-10-02/${sessionId}`
    })
  })

  it('creates the project session folder when the inbox folder is missing', async () => {
    const api = files({
      moveDir: vi.fn(async () => {
        throw new Error('Source directory does not exist: /missing')
      })
    })
    const moved = await moveCodeSessionDirectory(api, {
      root,
      sessionId,
      createdAt: day.toISOString(),
      fromProjectPath: null,
      toProjectPath: '/Users/roylin/.a3s/projects/测试'
    })

    expect(moved?.movedFrom).toBeNull()
    expect(api.mkdir).toHaveBeenCalledWith(`/Users/roylin/.a3s/projects/测试/${sessionId}/meta`)
  })

  it('moves the recorded workspace folder instead of the dated inbox', async () => {
    const api = files()
    const recorded = `/Users/roylin/.a3s/workspace/${sessionId}`
    const moved = await moveCodeSessionDirectory(api, {
      root,
      sessionId,
      createdAt: day.toISOString(),
      fromProjectPath: null,
      fromFolder: recorded,
      toProjectPath: '/Users/roylin/.a3s/projects/测试'
    })

    expect(api.moveDir).toHaveBeenCalledWith(recorded, `/Users/roylin/.a3s/projects/测试/${sessionId}`)
    expect(moved).toEqual({
      directory: `/Users/roylin/.a3s/projects/测试/${sessionId}`,
      movedFrom: recorded
    })
  })

  it('keeps assigning when the project folder is already there', async () => {
    const api = files({
      moveDir: vi.fn(async () => {
        throw new Error('workspace entry 409')
      })
    })
    const assigned = await placeTaskInProject(
      api,
      {
        root,
        sessionId,
        createdAt: day.toISOString(),
        fromProjectPath: null,
        fromFolder: `/Users/roylin/.a3s/workspace/${sessionId}`,
        toProjectPath: '/Users/roylin/.a3s/projects/测试'
      },
      async () => true
    )

    expect(assigned).toBe(true)
    expect(api.write).not.toHaveBeenCalled()
  })

  it('leaves the destination free when the recorded folder cannot be moved here', async () => {
    const api = files({
      moveDir: vi.fn(async () => {
        throw new Error('workspace entry 400')
      })
    })
    const moved = await moveCodeSessionDirectory(api, {
      root,
      sessionId,
      createdAt: day.toISOString(),
      fromProjectPath: null,
      fromFolder: `/Users/roylin/.a3s/workspace/${sessionId}`,
      toProjectPath: '/Users/roylin/.a3s/projects/测试'
    })

    expect(moved).toEqual({
      directory: `/Users/roylin/.a3s/projects/测试/${sessionId}`,
      movedFrom: null
    })
    expect(api.write).not.toHaveBeenCalled()
  })

  it('moves the folder back when the project assignment fails', async () => {
    const api = files()
    const assigned = await placeTaskInProject(
      api,
      {
        root,
        sessionId,
        createdAt: day.toISOString(),
        fromProjectPath: null,
        toProjectPath: '/Users/roylin/.a3s/projects/测试'
      },
      async () => false
    )

    expect(assigned).toBe(false)
    expect(api.moveDir).toHaveBeenNthCalledWith(
      2,
      `/Users/roylin/.a3s/projects/测试/${sessionId}`,
      `/Users/roylin/.a3s/sessions/2026-10-02/${sessionId}`
    )
  })

  it('moves a task folder between projects without nesting the session id twice', async () => {
    const api = files()
    const moved = await moveCodeSessionDirectory(api, {
      root,
      sessionId,
      createdAt: day.toISOString(),
      fromProjectPath: `/Users/roylin/.a3s/projects/测试/${sessionId}`,
      toProjectPath: '/Users/roylin/.a3s/projects/其他'
    })

    expect(api.moveDir).toHaveBeenCalledWith(
      `/Users/roylin/.a3s/projects/测试/${sessionId}`,
      `/Users/roylin/.a3s/projects/其他/${sessionId}`
    )
    expect(moved?.directory).toBe(`/Users/roylin/.a3s/projects/其他/${sessionId}`)
  })

  it('moves a project task folder back to the dated inbox', async () => {
    const api = files()
    const moved = await moveCodeSessionToInbox(api, {
      root,
      sessionId,
      createdAt: day.toISOString(),
      fromProjectPath: '/Users/roylin/.a3s/projects/测试'
    })

    expect(api.mkdir).toHaveBeenCalledWith('/Users/roylin/.a3s/sessions/2026-10-02')
    expect(api.moveDir).toHaveBeenCalledWith(
      `/Users/roylin/.a3s/projects/测试/${sessionId}`,
      `/Users/roylin/.a3s/sessions/2026-10-02/${sessionId}`
    )
    expect(moved).toEqual({
      directory: `/Users/roylin/.a3s/sessions/2026-10-02/${sessionId}`,
      movedFrom: `/Users/roylin/.a3s/projects/测试/${sessionId}`
    })
  })

  it('still unassigns when the dated inbox folder is already there', async () => {
    const api = files({
      moveDir: vi.fn(async () => {
        throw new Error('workspace entry 409')
      })
    })
    const assigned = await placeTaskInInbox(
      api,
      {
        root,
        sessionId,
        createdAt: day.toISOString(),
        fromProjectPath: '/Users/roylin/.a3s/projects/测试'
      },
      async () => true
    )

    expect(assigned).toBe(true)
    expect(api.write).not.toHaveBeenCalled()
  })

  it('moves the folder back into the project when leaving the inbox fails', async () => {
    const api = files()
    const assigned = await placeTaskInInbox(
      api,
      {
        root,
        sessionId,
        createdAt: day.toISOString(),
        fromProjectPath: '/Users/roylin/.a3s/projects/测试'
      },
      async () => false
    )

    expect(assigned).toBe(false)
    expect(api.moveDir).toHaveBeenNthCalledWith(
      2,
      `/Users/roylin/.a3s/sessions/2026-10-02/${sessionId}`,
      `/Users/roylin/.a3s/projects/测试/${sessionId}`
    )
  })
})
