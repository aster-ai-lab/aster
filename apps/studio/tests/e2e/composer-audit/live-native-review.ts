/** Real configured models and native files. API evidence never substitutes for CUA acceptance. */
import assert from 'node:assert/strict'
import { createHash } from 'node:crypto'
import { mkdirSync, readFileSync, writeFileSync } from 'node:fs'
import { resolve } from 'node:path'

import { parseStudioRuntime } from '../../../src/shared/studioRuntime'
import { workbenchProofSchema, type WorkbenchProof } from '../../../src/shared/types/workbenchProof'

const [runtimePath, agentId, output, onlyCase] = process.argv.slice(2)
assert(
  runtimePath && agentId && output,
  'Usage: live-native-review.ts <runtime-file> <owned-test-agent-id> <new-evidence-directory> [00|01|10|11]'
)
assert(onlyCase === undefined || ['00', '01', '10', '11'].includes(onlyCase), 'Unknown policy case')
const runtime = parseStudioRuntime(JSON.parse(readFileSync(runtimePath, 'utf8')))
assert(runtime, 'A valid native loopback runtime is required')
const boundary = 'Real configured author/reviewer and native files; API-only, CUA remains a separate gate'
const filename = 'live-provenance.txt'
const content = 'alpha\n'
const command = 'test -f live-provenance.txt && test "$(cat live-provenance.txt)" = alpha'
const headers = { authorization: `Bearer ${runtime.token}`, 'content-type': 'application/json' }

async function request(base: string, path: string, body?: unknown): Promise<any> {
  assert(/^http:\/\/127\.0\.0\.1:\d+$/.test(base), 'Native endpoints must stay on loopback')
  const response = await fetch(base + path, {
    method: body === undefined ? 'GET' : 'POST',
    headers,
    ...(body === undefined ? {} : { body: JSON.stringify(body) }),
    signal: AbortSignal.timeout(20_000)
  })
  assert(response.ok, `Native HTTP ${response.status}: ${path}`)
  return response.json()
}

function admittedAction(kind: unknown, input: Record<string, unknown>, workspace: string) {
  if (input.sandbox_permissions !== undefined && input.sandbox_permissions !== 'use_default') return false
  const exactFile = input.file_path === filename || input.file_path === `${workspace}/${filename}`
  return (
    (kind === 'edit' && exactFile && input.content === content) ||
    (kind === 'read' && exactFile) ||
    (kind === 'execute' &&
      (input.command === command || input.command === 'pwd; ls -la' || input.command === `sha256sum ${filename}`))
  )
}
assert(admittedAction('edit', { file_path: filename, content }, '/owned'))
assert(!admittedAction('edit', { file_path: '/other/live-provenance.txt', content }, '/owned'))
assert(!admittedAction('execute', { command: `${command}; touch other.txt` }, '/owned'))
assert(!admittedAction('execute', { command, sandbox_permissions: 'require_escalated' }, '/owned'))
assert(admittedAction('execute', { command: 'pwd; ls -la' }, '/owned'))
assert(!admittedAction('execute', { command: 'pwd; ls -la; cat /other' }, '/owned'))
assert(!admittedAction('execute', { command: `sha256sum ${filename}; touch other` }, '/owned'))

async function main() {
  mkdirSync(output, { recursive: true })
  const plane = await request(runtime!.apiBaseUrl, '/api/agent-plane')
  const agent = await request(plane.baseUrl, `/api/agents/${encodeURIComponent(agentId)}`)
  assert(agent.configuration?.permission_mode === 'default', 'Use an isolated test agent with per-action approval')
  let failed = false
  for (const policy of [
    { reviewMode: false, artifactTrace: false },
    { reviewMode: false, artifactTrace: true },
    { reviewMode: true, artifactTrace: false },
    { reviewMode: true, artifactTrace: true }
  ]) {
    const trace = policy.artifactTrace
    const review = policy.reviewMode
    const caseId = `${Number(review)}${Number(trace)}`
    if (onlyCase !== undefined && caseId !== onlyCase) continue
    const session = await request(plane.baseUrl, '/api/agent-sessions', {
      agentId,
      name: `Native model review ${caseId}`,
      workspace: { type: 'system' }
    })
    const startedAt = new Date().toISOString()
    const decisions: Array<{ id: string; kind: unknown; approved: boolean; escalated: boolean }> = []
    let proof: WorkbenchProof | undefined
    let bytesMatch = false
    let error: string | undefined
    let platformBlock: string | undefined
    try {
      const text = trace
        ? `Use write to create ${filename} in this task workspace, containing exactly alpha followed by one newline. For verification use bash to execute exactly this read-only command, with no additions: ${command}. Respect the native completion gate. Do not request a waiver, unsandboxed or escalated execution, or use other files. If the admitted verifier is unavailable, stop with failure. After verification reply only with the filename.`
        : 'Calculate 2+2. Answer with just the result. Do not call tools.'
      await request(plane.baseUrl, `/api/agent-sessions/${session.id}/turns`, {
        userMessageParts: [{ type: 'text', text }],
        turnContext: { directives: [], research: false, workbench: policy }
      })
      const handled = new Set<string>()
      const deadline = Date.now() + 150_000
      while (Date.now() < deadline) {
        const rows = await request(plane.baseUrl, `/api/agent-sessions/${session.id}/messages`)
        for (const row of Array.isArray(rows) ? rows : (rows.items ?? []))
          for (const part of row.data?.parts ?? row.parts ?? []) {
            if (
              !platformBlock &&
              String(part.errorText ?? JSON.stringify(part.output ?? '')).includes(
                'native_process_lifetime_unqualified'
              )
            ) {
              platformBlock =
                'native_process_lifetime_unqualified: admitted verifier unavailable; owned test aborted without escalation'
              await request(plane.baseUrl, `/api/agent-sessions/${session.id}/turn/abort`, {})
            }
            if (part.state !== 'approval-requested' || !part.approval?.id || handled.has(part.approval.id)) continue
            const kind = part.callProviderMetadata?.cherry?.toolKind
            const approved = !platformBlock && trace && admittedAction(kind, part.input ?? {}, session.workspace.path)
            handled.add(part.approval.id)
            decisions.push({
              id: part.approval.id,
              kind,
              approved,
              escalated: part.input?.sandbox_permissions === 'require_escalated'
            })
            await request(plane.baseUrl, `/api/agent-approvals/${part.approval.id}`, { approved })
          }
        const values = await request(plane.baseUrl, `/api/agent-sessions/${session.id}/proofs`)
        proof = values[0] ? workbenchProofSchema.parse(values[0]) : undefined
        if (proof)
          assert(
            proof.sessionId === session.id && proof.policy.reviewMode === review && proof.policy.artifactTrace === trace
          )
        if (
          proof &&
          ['completed', 'failed', 'cancelled'].includes(proof.producerStatus) &&
          (!review || ['completed', 'failed', 'cancelled', 'open_queue'].includes(proof.review.status))
        )
          break
        await new Promise((resolve) => setTimeout(resolve, 1000))
      }
      assert(proof, 'The native host did not persist proof')
      const artifact = proof.artifacts[0]
      if (artifact) {
        const snapshot = await request(
          plane.baseUrl,
          `/api/agent-sessions/${session.id}/proof-artifacts/${artifact.id}`
        )
        const bytes = Buffer.from(snapshot.data, 'base64')
        bytesMatch =
          bytes.equals(Buffer.from(content)) &&
          artifact.contentDigest === `sha256:${createHash('sha256').update(bytes).digest('hex')}`
        assert(bytesMatch, 'Native version bytes/digest differ from the requested fixture')
        if (proof.producerStatus !== 'completed')
          assert(artifact.status === 'incomplete', 'A partial producer must never certify a traced artifact')
      }
      assert(!platformBlock, platformBlock)
      assert(
        proof.producerStatus === 'completed' && proof.review.status === (review ? 'completed' : 'off'),
        'The producer/review did not complete'
      )
      assert(
        !trace || (bytesMatch && artifact?.status === 'traced'),
        'The completed producer has no traced byte version'
      )
    } catch (cause) {
      error = cause instanceof Error ? cause.message : String(cause)
      failed = true
      // Stop only this newly created test turn; keep its history for CUA inspection.
      await request(plane.baseUrl, `/api/agent-sessions/${session.id}/turn/abort`, {}).catch(() => {})
    }
    const status = error !== undefined ? 'failed' : 'passed'
    const report = {
      boundary,
      caseId,
      startedAt,
      finishedAt: new Date().toISOString(),
      sessionId: session.id,
      decisions,
      bytesMatch,
      platformBlock,
      proof,
      error,
      status
    }
    writeFileSync(resolve(output, `case-${caseId}.json`), `${JSON.stringify(report, null, 2)}\n`, { flag: 'wx' })
    process.stdout.write(
      `${caseId} ${status}: producer=${proof?.producerStatus}, review=${proof?.review.status}, artifacts=${proof?.artifacts.length}, exactBytes=${bytesMatch}\n`
    )
  }
  process.exitCode = failed ? 1 : 0
}
void main().catch((error) => {
  process.stderr.write(`${error instanceof Error ? error.message : String(error)}\n`)
  process.exitCode = 1
})
