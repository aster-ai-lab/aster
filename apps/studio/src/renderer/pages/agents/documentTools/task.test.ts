import { describe, expect, it } from 'vitest'

import { PDF_TOOL_BYTE_LIMIT, PDF_TOOL_FILE_LIMIT } from '../pdfToolsJob'
import {
  acceptDocumentCandidate,
  appendDocumentSources,
  removeDocumentSources,
  acknowledgeDocumentCancellation,
  createDocumentTask,
  documentItemStateAsStr,
  documentSaveStateAsStr,
  documentTaskProgress,
  failDocumentItem,
  failDocumentSave,
  finishDocumentSave,
  markDocumentItemRunning,
  parseDocumentItemState,
  parseDocumentSaveState,
  publishDocumentCandidate,
  reorderDocumentSources,
  requestDocumentCancellation,
  reviseDocumentTask,
  reviseDocumentItems,
  startDocumentAttempt,
  startDocumentSave,
  type DocumentTask
} from './task'

const source = (id: string, name = `${id}.png`, size = 20) => ({
  id,
  name,
  size,
  type: 'image/png'
})

function begin(
  task: DocumentTask<Uint8Array>,
  ids = task.items.filter((item) => item.state === 'ready').map((item) => item.source.id)
) {
  const next = startDocumentAttempt(task, ids)
  if (!next.attempt) throw new Error('Missing attempt')
  return { task: next, attempt: next.attempt }
}

function reviewed() {
  const running = begin(createDocumentTask<Uint8Array>('one', 'filter', [source('a')]))
  const value = Uint8Array.of(137, 80, 78, 71)
  const task = publishDocumentCandidate(
    markDocumentItemRunning(running.task, running.attempt, 'a'),
    running.attempt,
    'a',
    value
  )
  const candidate = task.items[0].candidate
  if (!candidate) throw new Error('Missing candidate')
  return { task, candidate, value }
}

describe('document task ownership', () => {
  it('revises only addressed sources and fences old references without promoting obsolete bytes', () => {
    let task = createDocumentTask<Uint8Array>('scoped', 'filter', [source('a'), source('b')])
    const running = begin(task)
    task = markDocumentItemRunning(running.task, running.attempt, 'a')
    task = publishDocumentCandidate(task, running.attempt, 'a', Uint8Array.of(10))
    task = markDocumentItemRunning(task, running.attempt, 'b')
    task = publishDocumentCandidate(task, running.attempt, 'b', Uint8Array.of(20))
    const first = task.items[0].candidate,
      second = task.items[1].candidate
    if (!first || !second) throw new Error('Expected reviewed batch')
    task = acceptDocumentCandidate(task, 'b', second)
    const scoped = reviseDocumentItems(task, ['a'])
    expect(scoped.items.map((item) => item.state)).toEqual(['ready', 'accepted'])
    expect(scoped.items[1].accepted?.value).toBe(second.value)
    expect(scoped.items[1].accepted?.revision).toBe(scoped.revision)
    expect(startDocumentSave(scoped, 'b').save.artifact.value).toBe(second.value)
    expect(() => acceptDocumentCandidate(scoped, 'b', second)).toThrow('stale-candidate')
    const all = reviseDocumentTask(scoped)
    const next = reviseDocumentItems(all, ['a'])
    expect(next.items[1].accepted?.revision).toBe(scoped.revision)
    expect(() => startDocumentSave(next, 'b')).toThrow('stale-candidate')
    expect(() => reviseDocumentItems(startDocumentSave(scoped, 'b').task, ['a'])).toThrow('busy')
    expect(() => reviseDocumentItems(scoped, ['foreign'])).toThrow('invalid-selection')
  })

  it('revalidates capacity after removal and reordering instead of stranding overflow sources', () => {
    const sources = Array.from({ length: PDF_TOOL_FILE_LIMIT + 1 }, (_, index) => source(`p${index}`))
    const original = createDocumentTask('capacity', 'filter', sources)
    const overflow = sources.at(-1)
    if (!overflow) throw new Error('Missing overflow fixture')
    expect(original.items.at(-1)?.refusal).toBe('too-many')
    const removed = removeDocumentSources(original, ['p0'])
    expect(removed.items.at(-1)?.state).toBe('ready')
    expect(removed.revision).toBe(original.revision + 1)
    const reordered = reorderDocumentSources(original, [overflow.id, ...sources.slice(0, -1).map((item) => item.id)])
    expect(reordered.items[0].state).toBe('ready')
    expect(reordered.items.at(-1)?.refusal).toBe('too-many')
    expect(original.items.at(-1)?.refusal).toBe('too-many')
  })

  it('fences obsolete review after source edits and retains accepted bytes without permitting stale saves', () => {
    const review = reviewed()
    const accepted = acceptDocumentCandidate(review.task, 'a', review.candidate)
    const appended = appendDocumentSources(accepted, [source('b')])
    expect(appended.items[0].accepted).toBe(review.candidate)
    expect(appended.items[0].candidate).toBeUndefined()
    expect(appended.items.map((item) => item.state)).toEqual(['ready', 'ready'])
    expect(() => startDocumentSave(appended, 'a')).toThrow('stale-candidate')
    expect(() => acceptDocumentCandidate(appended, 'a', review.candidate)).toThrow('stale-candidate')
    expect(() => appendDocumentSources(appended, [source('a')])).toThrow('duplicate-source')
    expect(() => removeDocumentSources(appended, ['missing'])).toThrow('invalid-selection')
    expect(() => removeDocumentSources(appended, ['a', 'a'])).toThrow('invalid-selection')
    expect(removeDocumentSources(appended, ['a', 'b']).items).toHaveLength(0)
    expect(appendDocumentSources(appended, [])).toBe(appended)
    expect(reorderDocumentSources(appended, ['a', 'b'])).toBe(appended)
  })

  it('refuses all source edits until execution or save actually settles', () => {
    const review = reviewed()
    const accepted = acceptDocumentCandidate(review.task, 'a', review.candidate)
    const saving = startDocumentSave(accepted, 'a').task
    const running = begin(createDocumentTask<Uint8Array>('run', 'filter', [source('a')])).task
    for (const task of [saving, running]) {
      expect(() => appendDocumentSources(task, [source('b')])).toThrow('busy')
      expect(() => removeDocumentSources(task, ['a'])).toThrow('busy')
      expect(() => reorderDocumentSources(task, ['a'])).toThrow('busy')
    }
  })

  it('keeps valid sources alongside independent input refusals', () => {
    const task = createDocumentTask('mixed', 'filter', [
      source('ok'),
      source('empty', 'empty.png', 0),
      source('huge', 'huge.png', PDF_TOOL_BYTE_LIMIT + 1),
      source('word', 'a.docx')
    ])
    expect(task.items.map((item) => [item.state, item.refusal])).toEqual([
      ['ready', undefined],
      ['rejected', 'empty'],
      ['rejected', 'too-big'],
      ['rejected', 'type']
    ])
    expect(documentTaskProgress(task).total).toBe(1)
  })

  it('does not let invalid inputs consume valid batch capacity', () => {
    const task = createDocumentTask('batch', 'filter', [
      source('bad', 'x.exe'),
      ...Array.from({ length: PDF_TOOL_FILE_LIMIT + 1 }, (_, index) => source(`p${index}`))
    ])
    expect(task.items.filter((item) => item.state === 'ready')).toHaveLength(PDF_TOOL_FILE_LIMIT)
    expect(task.items.at(-1)?.refusal).toBe('too-many')
  })

  it('refuses legacy Office input and invalid identity/size instead of coercion', () => {
    expect(createDocumentTask('legacy', 'word-to-pdf', [source('doc', 'old.doc')]).items[0].refusal).toBe(
      'legacy-office'
    )
    expect(createDocumentTask('legacy', 'excel-to-pdf', [source('xls', 'old.xls')]).items[0].refusal).toBe(
      'legacy-office'
    )
    expect(() => createDocumentTask('', 'filter', [source('a')])).toThrow('invalid-task')
    expect(() => createDocumentTask('one', 'filter', [source('a'), source('a')])).toThrow('duplicate-source')
    for (const size of [-1, Number.NaN, Infinity, 1.5])
      expect(() => createDocumentTask('one', 'filter', [source('a', 'a.png', size)])).toThrow('invalid-source')
  })

  it('accepts exact reviewed bytes once without generating a new attempt', () => {
    const review = reviewed()
    const accepted = acceptDocumentCandidate(review.task, 'a', review.candidate)
    expect(accepted.sequence).toBe(review.task.sequence)
    expect(accepted.items[0].accepted?.value).toBe(review.value)
    expect(accepted.items[0].saveState).toBe('unsaved')
    expect(accepted.items[0].receipt).toBeUndefined()
    expect(acceptDocumentCandidate(accepted, 'a', review.candidate)).toBe(accepted)
    expect(() => acceptDocumentCandidate(review.task, 'a', { ...review.candidate })).toThrow('stale-candidate')
  })

  it('fences completion after cancellation request until actual acknowledgement', () => {
    const running = begin(createDocumentTask<Uint8Array>('one', 'filter', [source('a')]))
    const started = markDocumentItemRunning(running.task, running.attempt, 'a')
    expect(acknowledgeDocumentCancellation(started, running.attempt)).toBe(started)
    const cancelling = requestDocumentCancellation(started)
    expect(cancelling.items[0].state).toBe('cancelling')
    expect(
      publishDocumentCandidate(cancelling, running.attempt, 'a', Uint8Array.of(1)).items[0].candidate
    ).toBeUndefined()
    expect(failDocumentItem(cancelling, running.attempt, 'a', 'late-failure').items[0].state).toBe('cancelling')
    expect(() => reviseDocumentTask(cancelling)).toThrow('busy')
    const cancelled = acknowledgeDocumentCancellation(cancelling, running.attempt)
    expect(cancelled.items[0].state).toBe('cancelled')
    expect(cancelled.attempt).toBeUndefined()
    const retry = begin(cancelled, ['a'])
    expect(publishDocumentCandidate(retry.task, running.attempt, 'a', Uint8Array.of(2))).toBe(retry.task)
    expect(acknowledgeDocumentCancellation(retry.task, running.attempt)).toBe(retry.task)
  })

  it('preserves successful files and retries only failed items', () => {
    const running = begin(createDocumentTask<Uint8Array>('batch', 'filter', [source('a'), source('b')]))
    let task = markDocumentItemRunning(running.task, running.attempt, 'a')
    task = publishDocumentCandidate(task, running.attempt, 'a', Uint8Array.of(1))
    const candidate = task.items[0].candidate
    if (!candidate) throw new Error('Missing candidate')
    task = acceptDocumentCandidate(task, 'a', candidate)
    task = failDocumentItem(task, running.attempt, 'b', 'engine-failed')
    expect(task.attempt).toBeUndefined()
    expect(documentTaskProgress(task)).toEqual({
      total: 2,
      completed: 2,
      failed: 1,
      reviewing: 0,
      accepted: 1,
      cancelling: 0
    })
    const retry = begin(task, ['b'])
    expect(retry.task.items[0]).toBe(task.items[0])
    expect(retry.task.items[0].accepted?.value).toEqual(Uint8Array.of(1))
    expect(() => startDocumentAttempt(task, ['a'])).toThrow('invalid-selection')
  })

  it('invalidates outdated review and preserves previous accepted output on option revision', () => {
    const review = reviewed()
    const accepted = acceptDocumentCandidate(review.task, 'a', review.candidate)
    const revised = reviseDocumentTask(accepted)
    expect(revised.items[0].state).toBe('ready')
    expect(revised.items[0].accepted).toBe(review.candidate)
    expect(() => acceptDocumentCandidate(revised, 'a', review.candidate)).toThrow('stale-candidate')
    expect(() => startDocumentSave(revised, 'a')).toThrow('stale-candidate')
  })

  it('validates complete reorder identities and prevents edits during execution', () => {
    const task = createDocumentTask<Uint8Array>('batch', 'filter', [source('a'), source('b')])
    expect(reorderDocumentSources(task, ['b', 'a']).items.map((item) => item.source.id)).toEqual(['b', 'a'])
    for (const order of [['a'], ['a', 'a'], ['a', 'foreign']])
      expect(() => reorderDocumentSources(task, order)).toThrow('invalid-selection')
    expect(() => reorderDocumentSources(begin(task).task, ['b', 'a'])).toThrow('busy')
    expect(() => startDocumentAttempt(task, [])).toThrow('empty-selection')
    expect(() => startDocumentAttempt(task, ['foreign'])).toThrow('invalid-selection')
    expect(() => startDocumentAttempt(task, ['a', 'a'])).toThrow('invalid-selection')
  })

  it('requires a real save receipt and fences late save callbacks after retry', () => {
    const review = reviewed()
    const accepted = acceptDocumentCandidate(review.task, 'a', review.candidate)
    const first = startDocumentSave(accepted, 'a')
    expect(() => reviseDocumentTask(first.task)).toThrow('busy')
    expect(() => startDocumentSave(first.task, 'a')).toThrow('busy')
    expect(() => finishDocumentSave(first.task, first.save, { destination: '', savedAt: 'now' })).toThrow(
      'invalid-receipt'
    )
    const failed = failDocumentSave(first.task, first.save, 'disk-full')
    expect(failed.items[0].accepted).toBe(review.candidate)
    expect(failed.items[0].saveState).toBe('failed')
    const retry = startDocumentSave(failed, 'a')
    const receipt = { destination: '/tmp/output.png', savedAt: '2026-10-08T12:00:00Z' }
    expect(finishDocumentSave(retry.task, first.save, receipt)).toBe(retry.task)
    const saved = finishDocumentSave(retry.task, retry.save, receipt)
    expect(saved.items[0].receipt).toEqual(receipt)
    expect(saved.items[0].saveState).toBe('saved')
    expect(failDocumentSave(saved, retry.save, 'late-failure')).toBe(saved)
  })

  it('names and refuses unknown closed-set statuses', () => {
    expect(documentItemStateAsStr(parseDocumentItemState('cancelling'))).toBe('cancelling')
    expect(documentSaveStateAsStr(parseDocumentSaveState('saved'))).toBe('saved')
    expect(() => parseDocumentItemState('done')).toThrow('document item state')
    expect(() => parseDocumentSaveState('uploaded')).toThrow('document save state')
  })
})
