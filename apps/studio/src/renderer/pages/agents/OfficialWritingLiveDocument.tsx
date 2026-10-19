import { useEffect, useLayoutEffect, useRef, useState, useSyncExternalStore } from 'react'

import {
  commitOfficialWritingDocument,
  finishOfficialWritingOpening,
  planOfficialWritingFollowUp,
  planOfficialWritingTyping,
  readOfficialWritingSession,
  revealOfficialWritingFrame,
  setOfficialWritingView,
  subscribeOfficialWritingSession,
  type OfficialWritingSession,
  type OfficialWritingTyping
} from './officialWritingAgentEdit'

const FRAME_MS = 24

function runOfficialWritingFrames(
  baseline: string,
  steps: readonly OfficialWritingTyping[],
  paint: (shown: string, caret: number, frame: string) => void,
  done: (shown: string) => void
): () => void {
  let doc = baseline
  let stepIndex = 0
  let frameIndex = 0
  let timer = 0
  let stopped = false
  const step = () => {
    if (stopped) return
    const typing = steps[stepIndex]
    const nextFrame = typing?.frames[frameIndex]
    if (!typing || nextFrame == null) return
    const shown = revealOfficialWritingFrame(doc, typing, nextFrame)
    paint(shown, typing.caret, nextFrame)
    frameIndex += 1
    if (frameIndex < typing.frames.length) {
      timer = window.setTimeout(step, FRAME_MS)
      return
    }
    doc = shown
    stepIndex += 1
    frameIndex = 0
    if (stepIndex < steps.length) {
      timer = window.setTimeout(step, FRAME_MS)
      return
    }
    // Hold the last glyph for one frame before the caret leaves.
    timer = window.setTimeout(() => {
      if (stopped) return
      done(doc)
    }, FRAME_MS)
  }
  timer = window.setTimeout(step, FRAME_MS)
  return () => {
    stopped = true
    window.clearTimeout(timer)
  }
}

export function useOfficialWritingSession(): OfficialWritingSession | null {
  return useSyncExternalStore(subscribeOfficialWritingSession, readOfficialWritingSession, readOfficialWritingSession)
}

/** Live caret for the official-writing task that is on screen. */
export function OfficialWritingSessionPaper({ sessionId }: { sessionId: string }) {
  const writing = useOfficialWritingSession()
  const visible = writing?.sessionId === sessionId
  useEffect(() => {
    if (!visible) return undefined
    setOfficialWritingView(sessionId)
    return () => setOfficialWritingView(null)
  }, [sessionId, visible])
  if (!writing || !visible) return null
  return <OfficialWritingLiveDocument document={writing.document} update={writing.update} opening={writing.opening} />
}

/**
 * Task-conversation document. The opening draft and each later edit reveal one
 * character per frame on one caret. The host node stays mounted.
 */
export function OfficialWritingLiveDocument({
  document,
  update,
  opening = false
}: {
  document: string
  update: string | null
  opening?: boolean
}) {
  const [shown, setShown] = useState(opening ? '' : document)
  const [seen, setSeen] = useState(opening ? '' : document)
  const [caret, setCaret] = useState<number | null>(null)
  const [frame, setFrame] = useState('')
  const baselineRef = useRef(opening ? '' : document)
  const caretNode = useRef<HTMLSpanElement>(null)
  if (!opening && update == null && document !== seen) {
    baselineRef.current = document
    setSeen(document)
    setShown(document)
    setCaret(null)
    setFrame('')
  }

  const paint = (next: string, nextCaret: number, nextFrame: string) => {
    setShown(next)
    setCaret(nextCaret)
    setFrame(nextFrame)
  }

  useEffect(() => {
    if (!opening || document.length === 0) return undefined
    const typing = planOfficialWritingTyping('', { at: 0, text: document })
    return runOfficialWritingFrames('', [typing], paint, (next) => {
      baselineRef.current = document
      setShown(next)
      setCaret(null)
      setFrame('')
      finishOfficialWritingOpening()
    })
  }, [opening, document])

  useEffect(() => {
    // A follow-up that arrives while the opening caret is moving waits here.
    if (opening || update == null || update === baselineRef.current) return undefined
    const base = baselineRef.current || document
    const plan = planOfficialWritingFollowUp(base, update)
    if (!plan.ok || plan.steps.length === 0) return undefined
    baselineRef.current = base
    return runOfficialWritingFrames(base, plan.steps, paint, (next) => {
      baselineRef.current = update
      setShown(next)
      setCaret(null)
      setFrame('')
      commitOfficialWritingDocument(update)
    })
  }, [opening, document, update])

  useLayoutEffect(() => {
    const node = caretNode.current
    if (!node) return
    node.scrollIntoView({ block: 'nearest', inline: 'nearest' })
  }, [caret, frame])

  const caretAt = caret == null ? null : caret + frame.length
  const before = caret == null ? shown : shown.slice(0, caretAt ?? 0)
  const after = caret == null ? '' : shown.slice(caretAt ?? 0)
  return (
    <pre data-ui="workbench.writing.document" data-writing-editor="live" className="writing-agent-document">
      {before}
      {caret == null ? null : (
        <span
          ref={caretNode}
          className="writing-agent-caret"
          data-writing-caret={String(caret)}
          data-writing-frame={frame}
        />
      )}
      {after}
    </pre>
  )
}
