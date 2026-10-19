import i18next from 'i18next'
import { useState } from 'react'
import { createRoot } from 'react-dom/client'
import { initReactI18next } from 'react-i18next'

import { AgentFormSubmitContext } from '../../../src/renderer/components/chat/form/agentFormSubmit'
import { FormPartView } from '../../../src/renderer/components/chat/form/FormPartView'
import { IntelligentUiSubmitContext } from '../../../src/renderer/components/chat/messages/markdown/intelligentUiHost'
import { OpenUiMessage } from '../../../src/renderer/components/chat/messages/markdown/OpenUiMessage'
import { splitOpenUiFences } from '../../../src/renderer/components/chat/messages/markdown/splitOpenUiFences'

await i18next.use(initReactI18next).init({
  lng: 'en',
  resources: {
    en: {
      translation: {
        'agent.form.invalid': 'Invalid UI',
        'agent.form.required': 'Required fields are missing',
        'agent.form.unavailable': 'Form unavailable',
        'agent.form.submitted': 'Sent',
        'agent.form.busy': 'Busy',
        'agent.form.sendFailed': 'Send failed',
        'common.loading': 'Loading'
      }
    }
  }
})

const budget = [
  'root = Card("旅行预算", [daily, total])',
  'daily = Input("daily", "每天预算", 300, "number", 0, 10000)',
  'total = Metric("合计", {daily * 5}, "元")'
].join('\n')

const formSource = [
  'root = Card("Details", [reason, agreed, count, submit])',
  'reason = Textarea("reason", "Reason", "", true)',
  'agreed = Checkbox("agreed", "Agree", false, true)',
  'count = Input("count", "Count", 3, "number")',
  'submit = Button("submit", "Continue")'
].join('\n')
const requestId = Array.from(crypto.getRandomValues(new Uint8Array(32)), (v) => v.toString(16).padStart(2, '0')).join(
  ''
)
const revision =
  'sha256:' +
  Array.from(new Uint8Array(await crypto.subtle.digest('SHA-256', new TextEncoder().encode(formSource))), (v) =>
    v.toString(16).padStart(2, '0')
  ).join('')
const binding = {
  protocolVersion: 1 as const,
  requestId,
  revision,
  owner: {
    sessionId: 'fixture',
    executionId: 'fixture',
    attemptId: 1,
    anchorMessageId: 'fixture',
    acpSessionId: 'fixture',
    promptId: 'fixture'
  }
}

function Fixture() {
  const [pendingForm, setPendingForm] = useState(false)
  const [source, setSource] = useState(budget + '\nnote =')
  const [streaming, setStreaming] = useState(true)
  const [answer, setAnswer] = useState('')
  return (
    <main>
      <h1>Intelligent UI message certification</h1>
      <p>
        Production Intelligent UI renderer and message/form hosts with controlled conversation and form submission. No
        AIX package is loaded by this fixture; native ownership is certified separately.
      </p>
      <nav>
        <button
          onClick={() =>
            setSource(
              [
                'root = Card("旅行预算", [daily, total, note])',
                'daily = Input("daily", "每天预算", 300, "number", 0, 10000)',
                'total = Metric("合计", {daily * 5}, "元")',
                'note = Text("已追加内容")',
                'extra ='
              ].join('\n')
            )
          }>
          Append content
        </button>
        <button
          onClick={() => {
            setSource(
              [
                'root = Card("旅行预算", [daily, total, note, submit])',
                'daily = Input("daily", "每天预算", 300, "number", 0, 10000)',
                'total = Metric("合计", {daily * 5}, "元")',
                'note = Text("已追加内容")',
                'submit = Button("submit", "使用这个预算")'
              ].join('\n')
            )
            setStreaming(false)
          }}>
          Finish response
        </button>
        <button
          onClick={() => {
            setPendingForm(false)
            setSource('root = Metric("Total", {alert(1)})')
            setStreaming(false)
          }}>
          Unsafe source
        </button>
        <button
          onClick={() => {
            setPendingForm(true)
            setAnswer('')
          }}>
          Pending form
        </button>
      </nav>
      <article aria-label="Assistant message">
        <IntelligentUiSubmitContext
          value={async (text) => {
            setAnswer(text)
            return 'sent'
          }}>
          {pendingForm ? (
            <AgentFormSubmitContext
              value={async (submission) => {
                setAnswer(JSON.stringify({ kind: 'same-turn', ...submission }))
                return 'sent'
              }}>
              <FormPartView
                part={{
                  data: {
                    id: requestId,
                    binding,
                    format: 'openui',
                    state: 'form-requested',
                    spec: formSource
                  }
                }}
              />
            </AgentFormSubmitContext>
          ) : (
            splitOpenUiFences(`\`\`\`openui\n${source}${streaming ? '' : '\n```'}`).map((segment) =>
              segment.kind === 'openui' ? (
                <OpenUiMessage
                  key={segment.start}
                  source={segment.source}
                  streaming={streaming || segment.incomplete === true}
                />
              ) : (
                <p key={segment.start}>{segment.source}</p>
              )
            )
          )}
        </IntelligentUiSubmitContext>
      </article>
      <pre aria-label="User continuation">{answer}</pre>
    </main>
  )
}

createRoot(document.getElementById('root')!).render(<Fixture />)
