import { createHash } from 'node:crypto'

import type { WorkbenchProof } from '../workbenchProof'

const digest = (text: string) => `sha256:${createHash('sha256').update(text).digest('hex')}`

/** Controlled host-contract data. It is never evidence of native execution. */
export function makeWorkbenchProof(overrides: Partial<WorkbenchProof> = {}): WorkbenchProof {
  const input = JSON.stringify({ file_path: '/workspace/report.md', content: 'hello' })
  return {
    schema: 'a3s.studio.workbench-proof.v1',
    sessionId: 'audit-session',
    messageId: 'reply',
    runId: 'run-1',
    producerStatus: 'completed',
    policy: { reviewMode: true, artifactTrace: true },
    model: 'openai::fixture',
    sourceText: 'hello',
    userText: 'Write a report',
    tools: [{ id: 'write-1', name: 'write', state: 'success', input, output: 'hello' }],
    artifacts: [
      {
        id: 'artifact-1',
        path: '/workspace/report.md',
        name: 'report.md',
        ordinal: 1,
        bytes: 5,
        mediaType: 'text/markdown',
        contentDigest: digest('hello'),
        receiptDigest: digest('fixture receipt'),
        inputDigests: [digest(input)],
        workflowDigest: digest('execution'),
        codeDigest: digest(input),
        environmentDigest: digest('environment'),
        environment: '{"host":"fixture"}',
        executionId: 'run-1',
        toolCallId: 'write-1',
        toolName: 'write',
        createdAt: '2026-10-15T00:00:00Z',
        status: 'traced'
      }
    ],
    review: { status: 'completed', findings: [], reviewer: 'independent-model' },
    ...overrides
  }
}
