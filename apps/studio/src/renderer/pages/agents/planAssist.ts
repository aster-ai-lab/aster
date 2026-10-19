import { fetchOrgModels } from '@renderer/data/orgModels'
import { readStudioRuntime } from '@shared/studioRuntime'

const SYSTEM = [
  'Revise the Markdown document.',
  'Return only the revised Markdown.',
  'Keep <!-- pagebreak --> comments and plan-image pictures.',
  'Do not add HTML or an explanation.'
].join(' ')

export type PlanDiffRow = { kind: 'same' | 'add' | 'del'; text: string }

/** Shows the changed lines, with two lines of context on each side. */
export function planLineDiff(before: string, after: string): PlanDiffRow[] {
  const left = before.split('\n')
  const right = after.split('\n')
  let start = 0
  while (start < left.length && start < right.length && left[start] === right[start]) start += 1
  let endLeft = left.length - 1
  let endRight = right.length - 1
  while (endLeft >= start && endRight >= start && left[endLeft] === right[endRight]) {
    endLeft -= 1
    endRight -= 1
  }
  const rows: PlanDiffRow[] = []
  for (let index = Math.max(0, start - 2); index < start; index += 1) rows.push({ kind: 'same', text: left[index] })
  for (let index = start; index <= endLeft; index += 1) rows.push({ kind: 'del', text: left[index] })
  for (let index = start; index <= endRight; index += 1) rows.push({ kind: 'add', text: right[index] })
  for (let index = endLeft + 1; index < Math.min(left.length, endLeft + 3); index += 1) {
    rows.push({ kind: 'same', text: left[index] })
  }
  return rows.slice(0, 80)
}

/** Drops a single outer markdown fence when the model wrapped the document. */
export function planRewriteText(content: string): string {
  const trimmed = content.trim()
  const fenced = /^```(?:markdown|md)?\n([\s\S]*?)\n```$/.exec(trimmed)
  return (fenced ? fenced[1] : trimmed).trim()
}

/**
 * Asks the signed-in organization gateway to revise the document.
 * The caller decides whether to keep the result.
 */
export async function requestPlanRewrite(input: {
  instruction: string
  markdown: string
  signal?: AbortSignal
}): Promise<string> {
  const runtime = readStudioRuntime()
  if (!runtime) throw new Error('studio runtime is unavailable')
  const credential = await gatewayCredential(runtime.apiBaseUrl, runtime.token, input.signal)
  const catalog = await fetchOrgModels(input.signal)
  const model =
    catalog.items.find((item) => item.available && item.id === catalog.defaultModel) ??
    catalog.items.find((item) => item.available)
  if (!model) throw new Error('no model is available')
  const response = await fetch(`${credential.origin}/v1/chat/completions`, {
    method: 'POST',
    signal: input.signal,
    headers: {
      authorization: `Bearer ${credential.token}`,
      'content-type': 'application/json'
    },
    body: JSON.stringify({
      model: model.id,
      temperature: 0.2,
      messages: [
        { role: 'system', content: SYSTEM },
        {
          role: 'user',
          content: '要求：\n' + input.instruction.slice(0, 2000) + '\n\n正文：\n' + input.markdown.slice(0, 12_000)
        }
      ]
    })
  })
  const body = (await response.json().catch(() => null)) as { choices?: { message?: { content?: unknown } }[] } | null
  const content = body?.choices?.[0]?.message?.content
  if (!response.ok || typeof content !== 'string' || !content.trim()) {
    throw new Error('the rewrite did not return a document')
  }
  return planRewriteText(content)
}

async function gatewayCredential(
  apiBaseUrl: string,
  token: string,
  signal?: AbortSignal
): Promise<{ origin: string; token: string }> {
  const response = await fetch(`${apiBaseUrl}/api/os-gateway/credential`, {
    headers: { authorization: `Bearer ${token}` },
    signal
  })
  const body = (await response.json().catch(() => null)) as { origin?: unknown; token?: unknown } | null
  const origin = typeof body?.origin === 'string' ? body.origin.trim().replace(/\/+$/, '') : ''
  const gatewayToken = typeof body?.token === 'string' ? body.token : ''
  if (!response.ok || !origin.startsWith('http') || !gatewayToken) {
    throw new Error('signed-in gateway is unavailable')
  }
  return { origin, token: gatewayToken }
}
