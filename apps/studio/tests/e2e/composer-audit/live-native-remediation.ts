/** Genuine staged draft -> independent finding -> repair -> explicit acceptance. API-only. */
import assert from 'node:assert/strict'
import { createHash } from 'node:crypto'
import { mkdirSync, readFileSync, writeFileSync } from 'node:fs'
import { resolve } from 'node:path'

import { parseStudioRuntime } from '../../../src/shared/studioRuntime'
import { workbenchProofSchema, type WorkbenchProof } from '../../../src/shared/types/workbenchProof'

const [runtimePath, agentId, output] = process.argv.slice(2)
assert(runtimePath && agentId && output, 'Usage: live-native-remediation.ts <runtime> <owned-agent> <new-output>')
const runtime = parseStudioRuntime(JSON.parse(readFileSync(runtimePath, 'utf8')))
assert(runtime)
const headers = { authorization: `Bearer ${runtime.token}`, 'content-type': 'application/json' }
const file = 'arithmetic.txt'
const checks = ['5', '4'].map((value) => `test -f ${file} && test "$(cat ${file})" = ${value}`)
const prompt = `This is a staged evaluation of automatic review. The final deliverable must be ${file} with the mathematically correct value of 2+2 and one newline. In the initial producer stage only, use write to seed the intentionally incorrect draft with exactly the digit 5 followed by one newline, verify its actual bytes using exactly: ${checks[0]}, then end this first turn with "Draft contains 5; awaiting automatic review." The configured independent review must identify the incorrect arithmetic and ask the main agent to repair the final deliverable to exactly the digit 4 followed by one newline. Any later repair should use write and then exactly: ${checks[1]}. Do not change other files, waive verification, escalate, or perform the review yourself. Initial draft instructions do not override the final correctness requirement.`

async function request(base: string, path: string, body?: unknown): Promise<any> {
  assert(/^http:\/\/127\.0\.0\.1:\d+$/.test(base))
  const response = await fetch(base + path, {
    method: body === undefined ? 'GET' : 'POST',
    headers,
    ...(body === undefined ? {} : { body: JSON.stringify(body) }),
    signal: AbortSignal.timeout(20_000)
  })
  assert(response.ok, `Native HTTP ${response.status}: ${path}`)
  return response.json()
}

function allowed(kind: unknown, input: Record<string, unknown>, root: string) {
  if (input.sandbox_permissions !== undefined && input.sandbox_permissions !== 'use_default') return false
  const path = input.file_path === file || input.file_path === `${root}/${file}`
  return (
    (kind === 'read' && path) ||
    (kind === 'edit' &&
      path &&
      (input.content === '5\n' ||
        input.content === '4\n' ||
        ((input.old_string === '5' || input.old_string === '5\n') &&
          (input.new_string === '4' || input.new_string === '4\n')))) ||
    (kind === 'execute' &&
      (checks.includes(String(input.command)) ||
        input.command === 'pwd; ls -la' ||
        input.command === `sha256sum ${file}`))
  )
}
assert(allowed('edit', { file_path: file, content: '4\n' }, '/owned'))
assert(!allowed('edit', { file_path: '/other/arithmetic.txt', content: '4\n' }, '/owned'))
assert(!allowed('execute', { command: checks[1], sandbox_permissions: 'require_escalated' }, '/owned'))
assert(!allowed('execute', { command: `${checks[1]}; rm other` }, '/owned'))
assert(allowed('execute', { command: 'pwd; ls -la' }, '/owned'))
assert(!allowed('execute', { command: 'pwd; ls -la; cat /other' }, '/owned'))

async function main() {
  mkdirSync(output, { recursive: true })
  const { baseUrl: base } = await request(runtime!.apiBaseUrl, '/api/agent-plane')
  const agent = await request(base, `/api/agents/${encodeURIComponent(agentId)}`)
  assert(agent.configuration?.permission_mode === 'default')
  const session = await request(base, '/api/agent-sessions', {
    agentId,
    name: 'Native model remediation',
    workspace: { type: 'system' }
  })
  const startedAt = new Date().toISOString()
  const decisions: Array<{ id: string; kind: unknown; approved: boolean }> = []
  const nativeErrors: string[] = []
  const handled = new Set<string>()
  let proofs: WorkbenchProof[] = []
  let confirmed: WorkbenchProof | undefined
  const snapshots: Array<{ id: string; ordinal: number; content: string; digest: string }> = []
  let error: string | undefined
  try {
    await request(base, `/api/agent-sessions/${session.id}/turns`, {
      userMessageParts: [{ type: 'text', text: prompt }],
      turnContext: { directives: [], research: false, workbench: { reviewMode: true, artifactTrace: true } }
    })
    const deadline = Date.now() + 240_000
    while (Date.now() < deadline) {
      const rows = await request(base, `/api/agent-sessions/${session.id}/messages`)
      for (const row of rows.items ?? rows)
        for (const part of row.data?.parts ?? []) {
          if (
            part.type === 'data-error' &&
            typeof part.data?.message === 'string' &&
            !nativeErrors.includes(part.data.message)
          )
            nativeErrors.push(part.data.message)
          if (
            String(part.errorText ?? JSON.stringify(part.output ?? '')).includes('native_process_lifetime_unqualified')
          )
            throw new Error('The admitted verifier is unqualified; no escalation permitted')
          if (part.state !== 'approval-requested' || !part.approval?.id || handled.has(part.approval.id)) continue
          const kind = part.callProviderMetadata?.cherry?.toolKind
          const approved = allowed(kind, part.input ?? {}, session.workspace.path)
          handled.add(part.approval.id)
          decisions.push({ id: part.approval.id, kind, approved })
          await request(base, `/api/agent-approvals/${part.approval.id}`, { approved })
        }
      proofs = (await request(base, `/api/agent-sessions/${session.id}/proofs`)).map((value: unknown) =>
        workbenchProofSchema.parse(value)
      )
      assert(proofs.every((proof) => proof.sessionId === session.id))
      const original = proofs.find((proof) => proof.policy.reviewMode)
      if (original && ['completed', 'failed', 'cancelled', 'open_queue'].includes(original.review.status)) break
      await new Promise((done) => setTimeout(done, 1000))
    }
    const original = proofs.find((proof) => proof.policy.reviewMode)
    assert(original?.producerStatus === 'completed', `Initial producer incomplete: ${nativeErrors.join('; ')}`)
    assert(
      ['open_queue', 'completed'].includes(original.review.status),
      `Independent review failed: ${original.review.error ?? original.review.status}`
    )
    assert(original.review.findings.length > 0, 'The real independent reviewer did not flag the incorrect draft')
    assert(
      original.review.status === 'open_queue' &&
        original.review.findings.every((finding) => finding.status === 'addressed'),
      'Repair was not independently verified as addressed'
    )
    const versions = proofs
      .flatMap((proof) => proof.artifacts)
      .filter((version) => version.name === file)
      .sort((a, b) => a.ordinal - b.ordinal)
    assert(versions.length >= 2 && versions.every((version) => version.status === 'traced'))
    assert(new Set(versions.map((version) => version.id)).size === versions.length)
    assert(versions.every((version, index) => version.ordinal === index + 1))
    for (const version of versions) {
      const response = await request(base, `/api/agent-sessions/${session.id}/proof-artifacts/${version.id}`)
      const bytes = Buffer.from(response.data, 'base64')
      const digest = `sha256:${createHash('sha256').update(bytes).digest('hex')}`
      assert(digest === version.contentDigest && bytes.length === version.bytes)
      snapshots.push({ id: version.id, ordinal: version.ordinal, content: bytes.toString('utf8'), digest })
    }
    assert(
      snapshots[0].content === '5\n' && snapshots.at(-1)?.content === '4\n',
      'The immutable initial/final bytes differ from the staged test'
    )
    for (const finding of original.review.findings)
      confirmed = workbenchProofSchema.parse(
        await request(base, `/api/agent-sessions/${session.id}/messages/${original.messageId}/proof`, {
          action: 'accept',
          findingId: finding.id
        })
      )
    assert(confirmed, 'No accepted proof was returned')
    assert(confirmed.sessionId === session.id && confirmed.messageId === original.messageId)
    assert.deepEqual(
      confirmed.review.findings.map((finding) => finding.id),
      original.review.findings.map((finding) => finding.id)
    )
    assert(
      confirmed?.review.status === 'completed' &&
        confirmed.review.findings.every((finding) => finding.status === 'accepted')
    )
    const reopened = (await request(base, `/api/agent-sessions/${session.id}/proofs`)).map((row: unknown) =>
      workbenchProofSchema.parse(row)
    )
    const saved = reopened.find((proof: WorkbenchProof) => proof.messageId === original.messageId)
    assert(
      saved?.sourceText === original.sourceText &&
        saved.review.findings.every((finding: { status: string }) => finding.status === 'accepted')
    )
    assert.deepEqual(
      saved.review.findings.map((finding: { id: string }) => finding.id),
      original.review.findings.map((finding) => finding.id)
    )
    assert(readFileSync(resolve(session.workspace.path, file), 'utf8') === '4\n')
    const old = await request(base, `/api/agent-sessions/${session.id}/proof-artifacts/${versions[0].id}`)
    assert(Buffer.from(old.data, 'base64').toString() === '5\n')
  } catch (cause) {
    error = cause instanceof Error ? cause.message : String(cause)
    await request(base, `/api/agent-sessions/${session.id}/turn/abort`, {}).catch(() => {})
  }
  writeFileSync(
    resolve(output, 'case-remediation.json'),
    `${JSON.stringify(
      {
        boundary: 'Real configured author/reviewer, staged wrong draft and native files; API-only, not CUA',
        startedAt,
        finishedAt: new Date().toISOString(),
        sessionId: session.id,
        decisions,
        nativeErrors,
        proofs,
        confirmed,
        snapshots,
        error,
        status: error !== undefined ? 'failed' : 'passed'
      },
      null,
      2
    )}\n`,
    { flag: 'wx' }
  )
  process.stdout.write(
    `remediation ${error !== undefined ? 'failed: ' + error : 'passed: independent finding, verified repair, acceptance and immutable old bytes'}\n`
  )
  process.exitCode = error !== undefined ? 1 : 0
}
void main().catch((error) => {
  process.stderr.write(`${error instanceof Error ? error.message : String(error)}\n`)
  process.exitCode = 1
})
