import { describe, expect, it, vi } from 'vitest'

import { DocumentTaskApplication } from './application'
import { createDocumentTask } from './task'

function deferred<T>() {
  let resolve: (value: T) => void = () => undefined
  let reject: (reason: unknown) => void = () => undefined
  const promise = new Promise<T>((accept, refuse) => {
    resolve = accept
    reject = refuse
  })
  return { promise, resolve, reject }
}

const sources = ['a', 'b', 'c'].map((id) => ({
  id,
  name: `${id}.png`,
  type: 'image/png',
  size: 10
}))

describe('document task queue', () => {
  it('runs sequentially, preserves partial success and retries only failed sources', async () => {
    const first = deferred<string>()
    const engine = vi.fn(async (source) => {
      if (source.id === 'a') return first.promise
      if (source.id === 'b') throw new Error('unreadable')
      return 'third'
    })
    const application = new DocumentTaskApplication(createDocumentTask<string>('queue', 'filter', sources), engine)
    const run = application.run(['a', 'b', 'c'])
    expect(engine).toHaveBeenCalledTimes(1)
    expect(application.getSnapshot().items.map((item) => item.state)).toEqual(['running', 'queued', 'queued'])
    first.resolve('first')
    await run
    const candidate = application.getSnapshot().items[0].candidate
    expect(candidate?.value).toBe('first')
    expect(application.getSnapshot().items.map((item) => item.state)).toEqual(['review', 'failed', 'review'])
    engine.mockImplementation(async () => 'recovered')
    await application.run(['b'])
    expect(application.getSnapshot().items[0].candidate).toBe(candidate)
    expect(engine).toHaveBeenCalledTimes(4)
    expect(application.getSnapshot().attempt).toBeUndefined()
  })

  it('publishes an automatic conversion once and preserves partial success on retry', async () => {
    const result = { bytes: new Uint8Array([1, 2, 3]) }
    const engine = vi.fn(async (source) => {
      if (source.id === 'b') throw new Error('conversion failed')
      return result
    })
    const application = new DocumentTaskApplication(
      createDocumentTask<typeof result>('automatic', 'word-to-pdf', [
        { id: 'a', name: 'a.docx', type: '', size: 10 },
        { id: 'b', name: 'b.docx', type: '', size: 10 }
      ]),
      engine,
      true
    )
    await application.run(['a', 'b'])
    const accepted = application.getSnapshot().items[0].accepted
    expect(accepted?.value).toBe(result)
    expect(application.getSnapshot().items.map((item) => item.state)).toEqual(['accepted', 'failed'])
    expect(application.getSnapshot().items[0].saveState).toBe('unsaved')
    engine.mockImplementation(async () => result)
    await application.run(['b'])
    expect(application.getSnapshot().items[0].accepted).toBe(accepted)
    expect(application.getSnapshot().items[1].accepted?.value).toBe(result)
    expect(engine).toHaveBeenCalledTimes(3)
  })

  it('does not acknowledge cancellation while non-cooperating work is still active', async () => {
    const running = deferred<string>()
    const engine = vi.fn(() => running.promise)
    const application = new DocumentTaskApplication(createDocumentTask<string>('queue', 'filter', sources), engine)
    const run = application.run(['a', 'b'])
    application.cancel()
    expect(application.getSnapshot().items.map((item) => item.state)).toEqual(['cancelling', 'cancelling', 'ready'])
    await expect(application.run(['c'])).rejects.toThrow('busy')
    running.resolve('late result')
    await run
    expect(engine).toHaveBeenCalledTimes(1)
    expect(application.getSnapshot().items.map((item) => item.state)).toEqual(['cancelled', 'cancelled', 'ready'])
    expect(application.getSnapshot().items[0].candidate).toBeUndefined()
    await application.run(['c'])
    expect(application.getSnapshot().items[2].candidate?.value).toBe('late result')
  })

  it('accepts without generation and only marks a completed write as saved', async () => {
    const artifact = { bytes: new Uint8Array([1, 2, 3]) }
    const engine = vi.fn(async () => artifact)
    const application = new DocumentTaskApplication(
      createDocumentTask<typeof artifact>('queue', 'filter', sources.slice(0, 1)),
      engine
    )
    await application.run(['a'])
    const candidate = application.getSnapshot().items[0].candidate
    if (!candidate) throw new Error('Missing candidate')
    application.accept('a', candidate)
    application.accept('a', candidate)
    expect(engine).toHaveBeenCalledTimes(1)
    const written = deferred<{ destination: string; savedAt: string }>()
    const writer = vi.fn(() => written.promise)
    const save = application.save('a', writer)
    expect(writer).toHaveBeenCalledWith(artifact)
    expect(application.getSnapshot().items[0].saveState).toBe('saving')
    written.resolve({ destination: '/exports/a.png', savedAt: '2026-10-08T00:00:00Z' })
    await save
    expect(application.getSnapshot().items[0].saveState).toBe('saved')
  })

  it('retains accepted bytes after a denied save and permits a save retry', async () => {
    const application = new DocumentTaskApplication(
      createDocumentTask<string>('queue', 'filter', sources.slice(0, 1)),
      async () => 'bytes'
    )
    await application.run(['a'])
    const candidate = application.getSnapshot().items[0].candidate
    if (!candidate) throw new Error('Missing candidate')
    application.accept('a', candidate)
    await application.save('a', async () => {
      throw new Error('denied')
    })
    expect(application.getSnapshot().items[0].accepted).toBe(candidate)
    expect(application.getSnapshot().items[0].saveState).toBe('failed')
    await application.save('a', async () => ({
      destination: '/exports/a.png',
      savedAt: '2026-10-08T00:00:00Z'
    }))
    expect(application.getSnapshot().items[0].saveState).toBe('saved')
  })

  it('publishes source mutations and executes the revised order with stable source identities', async () => {
    const engine = vi.fn(async (source) => source.id)
    const application = new DocumentTaskApplication(
      createDocumentTask<string>('sources', 'filter', sources.slice(0, 2)),
      engine
    )
    await application.run(['a', 'b'])
    const old = application.getSnapshot().items[0].candidate
    application.append([sources[2]])
    application.remove(['a'])
    application.reorder(['c', 'b'])
    expect(application.getSnapshot().items.map((item) => item.source.id)).toEqual(['c', 'b'])
    await application.run(['b', 'c'])
    expect(engine.mock.calls.map(([source]) => source.id)).toEqual(['a', 'b', 'c', 'b'])
    expect(application.getSnapshot().items.map((item) => item.candidate?.value)).toEqual(['c', 'b'])
    if (!old) throw new Error('Missing old candidate')
    expect(() => application.accept('a', old)).toThrow('stale-candidate')
    application.dispose()
    const disposed = application.getSnapshot()
    application.append([sources[0]])
    expect(application.getSnapshot()).toBe(disposed)
  })

  it('unsubscribes and prevents late results after disposal', async () => {
    const pending = deferred<string>()
    const application = new DocumentTaskApplication(
      createDocumentTask<string>('queue', 'filter', sources),
      () => pending.promise
    )
    const listener = vi.fn()
    const unsubscribe = application.subscribe(listener)
    const run = application.run(['a'])
    expect(listener).toHaveBeenCalled()
    unsubscribe()
    application.dispose()
    const snapshot = application.getSnapshot()
    pending.resolve('late')
    await run
    expect(application.getSnapshot()).toBe(snapshot)
    expect(snapshot.items[0].candidate).toBeUndefined()
  })
})
