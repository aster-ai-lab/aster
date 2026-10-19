import { MessageChannel } from 'node:worker_threads'

import { afterEach, expect, it, vi } from 'vitest'

import { bindAixEditorRecovery, checkpointAixEditor, hasAixEditorRecovery } from '../aixEditorRecovery'

const cleanup: (() => void)[] = []
function document(view = 'editor') {
  const channel = new MessageChannel()
  channel.port1.start()
  channel.port2.start()
  const release = bindAixEditorRecovery('recovery-proof', view, channel.port1 as unknown as globalThis.MessagePort)
  cleanup.push(() => {
    release()
    channel.port1.close()
    channel.port2.close()
  })
  return { peer: channel.port2, release }
}
afterEach(() => {
  cleanup.splice(0).forEach((close) => close())
  vi.useRealTimers()
})

it('waits for a matching durable checkpoint before permitting navigation', async () => {
  const doc = document()
  doc.peer.postMessage({ protocolVersion: 1, type: 'aix.view.recovery', dirty: true })
  await vi.waitFor(() => expect(hasAixEditorRecovery('recovery-proof', 'editor')).toBe(true))
  const request = new Promise<Record<string, unknown>>((resolve) => doc.peer.once('message', resolve))
  const checked = checkpointAixEditor('recovery-proof', 'editor', 'renew')
  const message = await request
  expect(message).toEqual({ protocolVersion: 1, type: 'aix.view.checkpoint', id: 1, purpose: 'renew' })
  doc.peer.postMessage({ protocolVersion: 1, type: 'aix.view.checkpoint.result', id: 2, ok: true, dirty: false })
  doc.peer.postMessage({ protocolVersion: 1, type: 'aix.view.checkpoint.result', id: 1, ok: true, dirty: true })
  await expect(checked).resolves.toBe(true)
})

it('preserves the window when a checkpoint fails or its document is withdrawn', async () => {
  const doc = document()
  doc.peer.postMessage({ protocolVersion: 1, type: 'aix.view.recovery', dirty: true })
  await vi.waitFor(() => expect(hasAixEditorRecovery('recovery-proof', 'editor')).toBe(true))
  doc.peer.once('message', (request) =>
    doc.peer.postMessage({
      protocolVersion: 1,
      type: 'aix.view.checkpoint.result',
      id: request.id,
      ok: false,
      dirty: true
    })
  )
  await expect(checkpointAixEditor('recovery-proof', 'editor')).rejects.toThrow('unavailable')
  const pending = checkpointAixEditor('recovery-proof', 'editor')
  const rejected = expect(pending).rejects.toThrow('closed')
  doc.release()
  await rejected
  await expect(checkpointAixEditor('recovery-proof', 'editor')).resolves.toBe(false)
})
