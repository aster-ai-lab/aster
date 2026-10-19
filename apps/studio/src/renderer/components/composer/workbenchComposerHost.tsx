import { createContext } from 'react'

import type { TopicStreamStatus } from '@shared/ai/transport'

/** DOM node on the workbench home where the live composer is placed. */
export const WorkbenchComposerHostContext = createContext<HTMLElement | null>(null)

/**
 * Live work state of the open task. The latest assistant face reads this.
 * Undefined when the session has no active stream.
 */
export const WorkbenchTurnStreamContext = createContext<TopicStreamStatus | undefined>(undefined)

/**
 * The open task conversation. The composer keeps the home controls, and the
 * project shown under it cannot be changed.
 */
export const WorkbenchProjectLockedContext = createContext(false)

/** The session under the home composer already has messages, so a send must start a new task. */
export const WorkbenchSessionHasMessagesContext = createContext(false)

export type WorkbenchFreshSession = { id: string }

export type WorkbenchHomeTaskBridge = {
  /** An untitled session for the next home send. Does not leave the home. */
  create: () => Promise<WorkbenchFreshSession | null>
  /** Show that session while the home stays up, so the composer rebinds before the send. */
  adopt: (id: string) => void
}

export const WorkbenchHomeTaskContext = createContext<WorkbenchHomeTaskBridge | null>(null)

let pendingHomeSubmitSessionId: string | null = null

/** The fresh task should send the draft that was just moved onto it. */
export function markPendingHomeSubmit(sessionId: string) {
  pendingHomeSubmitSessionId = sessionId
}

export function peekPendingHomeSubmit(sessionId: string) {
  return pendingHomeSubmitSessionId === sessionId
}

export function consumePendingHomeSubmit(sessionId: string) {
  if (pendingHomeSubmitSessionId === sessionId) pendingHomeSubmitSessionId = null
}

const homeDraftHandoff = new Set<string>()

/**
 * The fresh task is sending the moved draft. A remount while that send is in
 * flight must not restore the text, and the unmount snapshot must not write it back.
 */
export function beginHomeDraftHandoff(sessionId: string) {
  homeDraftHandoff.add(sessionId)
}

export function homeDraftHandoffActive(sessionId: string) {
  return homeDraftHandoff.has(sessionId)
}

export function finishHomeDraftHandoff(sessionId: string) {
  homeDraftHandoff.delete(sessionId)
}

/**
 * The home overlay is up. Distinct from the host node: the node is published
 * after the home commits, and the composer has to exist before that so it can
 * move into the slot. A null host node still means the home is showing.
 */
export const WorkbenchHomeActiveContext = createContext(false)

/** Home card resting height, about four lines. Chat composers keep the shared two-line minimum. */
export const WORKBENCH_COMPOSER_EDITOR_MIN_HEIGHT = 88

/**
 * The live composer node is parked inside an empty anchor. The project tray slot
 * is the anchor's sibling, so it is found from the frame that holds both.
 */
export function findWorkbenchComposerScope(host: HTMLElement): HTMLElement | null {
  return host.parentElement?.parentElement?.querySelector<HTMLElement>("[data-ui~='workbench.composer.scope']") ?? null
}
