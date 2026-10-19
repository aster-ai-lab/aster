import type { PluginActivationContext, PluginComposerDraft } from '@renderer/types/workbenchPlugin'
import { AIX_DOCUMENT_ATTEMPT_LIMIT, aixDocumentReadySchema } from '@shared/types/aixViewBridge'
import type { AixWorkbenchPackage, AixWorkbenchView } from '@shared/types/aixWorkbench'

import type { AixViewSessions } from './aixWorkbench'

/** The provider document exists only for this invocation and receives no host services. */
export async function invokeAixComposer(
  sessions: AixViewSessions,
  item: AixWorkbenchPackage,
  view: AixWorkbenchView,
  context: PluginActivationContext,
  draft: PluginComposerDraft,
  signal?: AbortSignal
): Promise<string> {
  if (view.point !== 'studio.composer.actions' || !context.isCurrent() || signal?.aborted)
    throw new Error('AIX composer admission refused')
  const session = sessions.open(item, view)
  const frame = document.createElement('iframe')
  frame.hidden = true
  frame.tabIndex = -1
  frame.title = view.title || view.id
  frame.setAttribute('sandbox', 'allow-scripts')
  frame.setAttribute('referrerpolicy', 'no-referrer')
  frame.setAttribute(
    'allow',
    "camera 'none'; microphone 'none'; geolocation 'none'; clipboard-read 'none'; clipboard-write 'none'"
  )
  let withdrawn = false
  let attempts = 0
  const withdraw = () => {
    withdrawn = true
    session.withdraw()
    frame.remove()
  }
  const connectDocument = (event: MessageEvent) => {
    if (
      !frame.isConnected ||
      !frame.contentWindow ||
      event.source !== frame.contentWindow ||
      event.data?.type !== 'aix.document.ready'
    )
      return
    if (withdrawn || ++attempts > AIX_DOCUMENT_ATTEMPT_LIMIT) {
      for (const port of event.ports) port.close()
      withdraw()
      return
    }
    const parsed = aixDocumentReadySchema.safeParse(event.data)
    if (event.origin !== 'null' || event.ports.length !== 1 || !parsed.success) {
      for (const port of event.ports) port.close()
      return
    }
    try {
      session.connect(parsed.data.documentId, event.ports[0])
    } catch {
      withdraw()
    }
  }
  const loaded = () => {
    if (!session.documentLoaded()) withdraw()
  }
  window.addEventListener('message', connectDocument)
  frame.addEventListener('load', loaded)
  context.signal.addEventListener('abort', withdraw, { once: true })
  signal?.addEventListener('abort', withdraw, { once: true })
  try {
    const composer = session.composer
    if (!composer) throw new Error('AIX composer protocol unavailable')
    const result = composer.invoke(draft)
    void session.ready.then(
      ({ url }) => {
        if (withdrawn || !context.isCurrent() || signal?.aborted) return withdraw()
        try {
          frame.src = url
          document.body.append(frame)
        } catch {
          withdraw()
        }
      },
      () => withdraw()
    )
    if (!context.isCurrent() || context.signal.aborted || signal?.aborted) withdraw()
    const replacement = await result
    if (withdrawn || !context.isCurrent() || signal?.aborted) throw new Error('AIX composer owner closed')
    return replacement
  } finally {
    withdraw()
    window.removeEventListener('message', connectDocument)
    frame.removeEventListener('load', loaded)
    context.signal.removeEventListener('abort', withdraw)
    signal?.removeEventListener('abort', withdraw)
    await session.close()
  }
}
