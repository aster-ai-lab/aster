import { webcrypto } from 'node:crypto'
import { readFileSync } from 'node:fs'
import process from 'node:process'

import { cleanup, render, screen, waitFor } from '@testing-library/react'
import userEvent from '@testing-library/user-event'
import { readUIMessageStream, type UIMessageChunk } from 'ai'
import { afterEach, expect, it, vi } from 'vitest'

import { submitAgentForm, resetStudioAgentPlaneForTests } from '@renderer/data/studioAgentPlane'
import { TaskConversationTurn } from '@renderer/pages/agents/taskConversation/TaskConversationMessage'
import { applyTobotChunk, type TobotLive } from '@renderer/services/tobotStream'
import type { AgentFormChunkPayload } from '@shared/ai/transport'
import { agentFormPartSchema, agentFormReceiptSchema, agentFormSubmissionSchema } from '@shared/types/agentForm'

import { AgentFormSubmitContext, type AgentFormSubmit } from '../agentFormSubmit'

vi.unmock('@a3s-lab/ui')
const artifactPath = process.env.A3S_CLOUD_TEST_AGENT_FORM_INPUT

afterEach(() => {
  cleanup()
  resetStudioAgentPlaneForTests()
  vi.unstubAllGlobals()
  vi.restoreAllMocks()
})

it.skipIf(!artifactPath)(
  'consumes actual native form chunks through the SDK, submits their binding and retains the readonly answer',
  async () => {
    const artifact = JSON.parse(readFileSync(artifactPath!, 'utf8')) as {
      requested: UIMessageChunk[]
      answered: UIMessageChunk[]
      requestEnvelope: AgentFormChunkPayload
      answerEnvelope: AgentFormChunkPayload
      submission: unknown
      receipt: unknown
    }
    const submission = agentFormSubmissionSchema.parse(artifact.submission)
    const receipt = agentFormReceiptSchema.parse(artifact.receipt)
    const sdk = async (chunks: typeof artifact.requested) => {
      const stream = new globalThis.ReadableStream({
        start(controller) {
          for (const chunk of chunks) controller.enqueue(chunk)
          controller.close()
        }
      })
      let data: unknown
      for await (const message of readUIMessageStream({ stream }))
        data = message.parts.find((part) => part.type === 'data-form')
      return agentFormPartSchema.parse((data as { data: unknown }).data)
    }
    const initial = await sdk(artifact.requested)
    expect(initial.binding).toEqual(submission.binding)
    const topic = `agent-session:${initial.binding.owner.sessionId}`
    const live = applyTobotChunk(null, artifact.requestEnvelope, topic) as TobotLive
    expect(live.parts[0].data).toEqual(initial)
    vi.stubGlobal('crypto', webcrypto)
    vi.stubGlobal('__A3S_STUDIO_RUNTIME__', { apiBaseUrl: 'http://127.0.0.1:11', token: 'a'.repeat(64) })
    const posted: unknown[] = []
    vi.stubGlobal('fetch', async (input: string, init?: globalThis.RequestInit) => {
      if (input.endsWith('/api/agent-plane'))
        return new globalThis.Response(JSON.stringify({ baseUrl: 'http://127.0.0.1:12' }))
      expect(input).toBe(`http://127.0.0.1:12/api/agent-forms/${initial.id}`)
      expect(init?.method).toBe('POST')
      posted.push(JSON.parse(String(init?.body)))
      return new globalThis.Response(JSON.stringify(receipt))
    })
    const submit: AgentFormSubmit = async (body, signal) => ((await submitAgentForm(body, signal)) ? 'sent' : 'failed')
    const ui = (data: typeof initial) => (
      <AgentFormSubmitContext value={submit}>
        <TaskConversationTurn parts={[{ type: 'data-form', id: data.id, data }]} streaming={false} />
      </AgentFormSubmitContext>
    )
    const rendered = render(ui(initial))
    expect(screen.getByRole('spinbutton', { name: 'Count' })).toHaveValue(0)
    expect(screen.getByRole('checkbox', { name: 'Ready' })).not.toBeChecked()
    await userEvent.setup().click(screen.getByRole('button', { name: 'Continue' }))
    await waitFor(() => expect(posted).toEqual([submission]))
    const answered = await sdk([...artifact.requested, ...artifact.answered])
    expect(answered.state).toBe('answered')
    expect(applyTobotChunk(live, artifact.answerEnvelope, topic)?.parts[0].data).toEqual(answered)
    rendered.rerender(ui(answered))
    expect(screen.queryByRole('button')).not.toBeInTheDocument()
    for (const value of ['0', 'false', 'Work']) expect(screen.getByText(value)).toBeInTheDocument()
  }
)
