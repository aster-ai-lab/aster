/** Work-parity audit of production composer logic. Failures are open product findings. */
import assert from 'node:assert/strict'
import { mkdirSync, writeFileSync } from 'node:fs'
import { resolve } from 'node:path'

import {
  EMPTY_HOME_COMPOSER_FLAGS,
  homeTurnContext,
  readHomeComposerOptions,
  writeHomeComposerOptions
} from '../../../src/renderer/pages/agents/workbenchHomeCapabilities'
import { makeWorkbenchProof } from '../../../src/shared/types/__tests__/workbenchProof.fixture'
import { proofFromParts, workbenchProofSchema } from '../../../src/shared/types/workbenchProof'

const output = resolve(process.argv[2] ?? '.a3s-test/composer-audit/logic')
mkdirSync(output, { recursive: true })
const results: Array<{ id: string; name: string; status: string; detail?: string }> = []
function check(id: string, name: string, run: () => void) {
  try {
    run()
    results.push({ id, name, status: 'passed' })
  } catch (error) {
    results.push({ id, name, status: 'failed', detail: error instanceof Error ? error.message : String(error) })
  }
}
function storage() {
  const values = new Map<string, string>()
  return {
    getItem: (key: string) => values.get(key) ?? null,
    setItem: (key: string, value: string) => {
      values.set(key, value)
    },
    removeItem: (key: string) => {
      values.delete(key)
    }
  }
}
const flags = (reviewMode: boolean, artifactTrace: boolean) => ({ reviewMode, artifactTrace, researchMode: false })

check('LOG-01', 'A new profile starts with both policies off', () => {
  assert.deepEqual(readHomeComposerOptions(storage()), EMPTY_HOME_COMPOSER_FLAGS)
})
for (const review of [false, true])
  for (const trace of [false, true]) {
    check(`LOG-02-${Number(review)}${Number(trace)}`, 'Both policies persist and serialize independently', () => {
      const state = storage()
      const expected = flags(review, trace)
      writeHomeComposerOptions(expected, state)
      assert.deepEqual(readHomeComposerOptions(state), expected)
      const context = homeTurnContext(expected)
      assert.deepEqual(context.workbench, { reviewMode: review, artifactTrace: trace })
    })
  }
check('LOG-03', 'Malformed cached settings fail closed', () => {
  assert.deepEqual(readHomeComposerOptions({ getItem: () => '{broken' }), EMPTY_HOME_COMPOSER_FLAGS)
})
check('LOG-04', 'Ordinary reply text cannot create a host policy binding', () => {
  assert.equal(proofFromParts([{ type: 'text', data: makeWorkbenchProof() }]), null)
})
check('LOG-05', 'Out-of-order tasks keep their native message identities', () => {
  const first = makeWorkbenchProof({ messageId: 'A', policy: { reviewMode: true, artifactTrace: false } })
  const second = makeWorkbenchProof({ messageId: 'B', policy: { reviewMode: false, artifactTrace: true } })
  assert.deepEqual(proofFromParts([{ type: 'data-workbench-proof', data: second }], 'B')?.policy, second.policy)
  assert.equal(proofFromParts([{ type: 'data-workbench-proof', data: first }], 'B'), null)
})
check('LOG-06', 'History cannot acquire a policy without host evidence', () => {
  assert.equal(proofFromParts([{ type: 'text' }], 'old-history'), null)
})
check('LOG-07', 'A new turn cannot inherit a different message proof', () => {
  assert.equal(proofFromParts([{ type: 'data-workbench-proof', data: makeWorkbenchProof() }], 'tomorrow'), null)
})
check('LOG-08', 'Same-time submits carry independent typed policy values', () => {
  const first = homeTurnContext(flags(true, false))
  const second = homeTurnContext(flags(false, true))
  assert.deepEqual(first.workbench, { reviewMode: true, artifactTrace: false })
  assert.deepEqual(second.workbench, { reviewMode: false, artifactTrace: true })
})
check('LOG-09', 'Storage failure is reported without throwing', () => {
  assert.equal(
    writeHomeComposerOptions(flags(true, true), {
      setItem() {
        throw new Error('quota')
      }
    }),
    false
  )
})
for (const [id, text] of [
  ['LOG-10', 'Completed /workspace/report.md'],
  ['LOG-11', 'The original source is /workspace/source.csv'],
  ['LOG-12', 'I have not created any files.'],
  ['LOG-13', 'The final report now exists on disk and is ready for download.']
])
  check(id, 'Semantic claims require an independent native result, never a renderer matcher', () => {
    assert.equal(proofFromParts([{ type: 'text', data: text }]), null)
  })
check('LOG-14', 'A path cannot substitute for a content digest', () => {
  const proof = makeWorkbenchProof()
  proof.artifacts[0].contentDigest = '/workspace/report.md'
  assert.equal(workbenchProofSchema.safeParse(proof).success, false)
})
check('LOG-15', 'An invented success state is refused', () => {
  const proof = makeWorkbenchProof()
  assert.equal(
    workbenchProofSchema.safeParse({ ...proof, review: { ...proof.review, status: 'assumed-success' } }).success,
    false
  )
})
check('LOG-16', 'A successful path-only tool does not create provenance', () => {
  assert.equal(
    proofFromParts([{ type: 'tool-write', data: { path: '/workspace/report.md', state: 'output-available' } }]),
    null
  )
})

const passed = results.filter((row) => row.status === 'passed').length
const report = {
  boundary: 'production TypeScript functions; no native/model execution',
  passed,
  failed: results.length - passed,
  results
}
writeFileSync(resolve(output, 'logic-results.json'), JSON.stringify(report, null, 2) + '\n')
for (const row of results) process.stdout.write(`${row.status.toUpperCase()} ${row.id} ${row.name}\n`)
process.stdout.write(`TOTAL ${results.length}: ${passed} passed, ${results.length - passed} failed\n`)
process.exitCode = report.failed ? 1 : 0
