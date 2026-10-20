/**
 * Agent session domain API Schema definitions.
 */

import * as z from 'zod'

import { TraceIdSchema } from '@shared/data/types/trace'
import { workbenchTurnPolicySchema } from '@shared/types/workbenchProof'

import type { CursorPaginationResponse } from '../types'
import type { OrderEndpoints } from './_endpointHelpers'
import {
  type AgentSessionWorkspaceSource,
  AgentSessionWorkspaceSourceSchema,
  AgentWorkspaceEntitySchema
} from './agentWorkspaces'

// ============================================================================
// Entity & DTOs (Rule C: derive DTOs via .pick())
// ============================================================================

/**
 * Session name validator. Empty is allowed for an untitled placeholder session,
 * and the length is capped at 255 — matching topic.name semantics
 * (`TopicNameEntitySchema`).
 */
export const SessionNameEntitySchema = z.string().max(255)

export const AgentSessionExecutionStatusSchema = z.enum([
  'pending',
  'streaming',
  'awaiting-approval',
  'awaiting-form',
  'done',
  'error',
  'aborted',
  'unknown'
])
export type AgentSessionExecutionStatus = z.infer<typeof AgentSessionExecutionStatusSchema>

export const AgentSessionExecutionSchema = z.strictObject({
  status: AgentSessionExecutionStatusSchema,
  executionId: z.string().min(1),
  anchorMessageId: z.string().min(1),
  observedAt: z.iso.datetime()
})

export const AgentSessionEntitySchema = z.strictObject({
  id: z.string(),
  agentId: z.string().nullable(),
  /** May be empty for an untitled placeholder session, matching topic.name semantics. */
  name: SessionNameEntitySchema,
  isNameManuallyEdited: z.boolean(),
  description: z.string().optional(),
  workspaceId: z.string(),
  workspace: AgentWorkspaceEntitySchema,
  /** Container-level OTel trace id — one trace tree per session. */
  traceId: TraceIdSchema.optional(),
  orderKey: z.string(),
  /** Last real conversation activity timestamp. */
  lastActivityAt: z.iso.datetime(),
  createdAt: z.string(),
  updatedAt: z.string(),
  /** Read-only soft-delete timestamp, present only for trashed sessions. */
  deletedAt: z.string().optional(),
  execution: AgentSessionExecutionSchema.optional()
})
export type AgentSessionEntity = z.infer<typeof AgentSessionEntitySchema>

// Create requires a real `agentId` — orphans only happen via cascade, never on insert.
export const CreateAgentSessionSchema = z.strictObject({
  agentId: z.string().min(1),
  name: SessionNameEntitySchema,
  description: z.string().optional(),
  workspace: AgentSessionWorkspaceSourceSchema
})
export type CreateAgentSessionDto = z.infer<typeof CreateAgentSessionSchema>

export const UpdateAgentSessionSchema = z.strictObject({
  name: SessionNameEntitySchema.optional(),
  isNameManuallyEdited: z.boolean().optional(),
  description: z.string().optional(),
  agentId: z.string().min(1).optional()
})

export type UpdateAgentSessionDto = z.infer<typeof UpdateAgentSessionSchema>

/**
 * Body for `PUT /agent-sessions/:sessionId/workspace`. Replacing a session's
 * workspace creates/deletes the backing system workspace row and is only
 * allowed before any message exists, so it lives on a dedicated sub-resource
 * rather than the generic PATCH (see api-design-guidelines: complex
 * side-effects / resource creation → dedicated endpoint).
 */
export const SetAgentSessionWorkspaceSchema = AgentSessionWorkspaceSourceSchema
export type SetAgentSessionWorkspaceDto = AgentSessionWorkspaceSource

/** Query for `GET /agent-sessions` (cursor pagination + optional agent filter). */
export const ListAgentSessionsQuerySchema = z.strictObject({
  ids: z.array(z.string().min(1)).min(1).max(200).optional(),
  agentId: z.string().optional(),
  workspaceId: z.string().min(1).optional(),
  cursor: z.string().optional(),
  limit: z.coerce.number().int().positive().max(200).optional(),
  /** `true` lists only trashed sessions; omitted/false lists active sessions. */
  inTrash: z.boolean().optional(),
  search: z
    .string()
    .trim()
    .refine((value) => Array.from(value).length <= 255, 'Search exceeds 255 characters')
    .optional(),
  status: AgentSessionExecutionStatusSchema.optional(),
  activityFrom: z.iso.datetime({ offset: true }).optional(),
  activityBefore: z.iso.datetime({ offset: true }).optional()
})
export type ListAgentSessionsQueryParams = z.input<typeof ListAgentSessionsQuerySchema>
export type ListAgentSessionsQuery = z.output<typeof ListAgentSessionsQuerySchema>

/** Optional owner scope for `GET /agent-sessions/latest`; omitted means global latest. */
export const LatestAgentSessionQuerySchema = z.strictObject({
  agentId: z.string().min(1).optional()
})
export type LatestAgentSessionQuery = z.infer<typeof LatestAgentSessionQuerySchema>

/** Exact creation target for atomically reusing or creating an empty session. */
export const ReuseOrCreateAgentSessionSchema = z.strictObject({
  agentId: z.string().min(1),
  workspace: AgentSessionWorkspaceSourceSchema,
  excludeSessionId: z.string().min(1).optional()
})
export type ReuseOrCreateAgentSessionDto = z.infer<typeof ReuseOrCreateAgentSessionSchema>

/** Name and personality for the ToBot process. They stay out of the stored user sentence. */
const ToBotHostContextSchema = z.strictObject({
  personaName: z.string().min(1).max(32),
  persona: z.string().max(400)
})

/** Body for `POST /agent-sessions/tobot/ensure`. An omitted model uses the gateway catalog. */
export const EnsureToBotAgentSchema = z.strictObject({
  model: z.string().min(1).optional(),
  hostContext: ToBotHostContextSchema.optional(),
  harnessComponents: z.array(z.string().min(1)).min(1).optional()
})
export type EnsureToBotAgentBody = z.infer<typeof EnsureToBotAgentSchema>

const ToBotTextPartSchema = z.strictObject({
  type: z.literal('text'),
  text: z.string().min(1)
})

/** A file the shell already wrote. The agent reads `url` as an absolute path. */
const ToBotFilePartSchema = z.strictObject({
  type: z.literal('file'),
  url: z.string().regex(/^file:\/\/\/\S+$/),
  filename: z.string().min(1).max(255),
  mediaType: z.string().min(1).max(127),
  sizeBytes: z
    .number()
    .int()
    .nonnegative()
    .max(20 * 1024 * 1024)
    .optional()
})

/** Per-turn host constraints. They prefix the model string and stay off the stored user sentence. */
const ToBotTurnContextSchema = z.strictObject({
  workbench: workbenchTurnPolicySchema.optional(),
  directives: z.array(z.string().min(1).max(500)).max(8),
  research: z.boolean().optional(),
  agentId: z
    .string()
    .regex(/^[A-Za-z0-9][A-Za-z0-9_.-]{0,63}$/)
    .optional()
})

/** Body for `POST /agent-sessions/tobot/turns`. Text and files are one user turn. */
export const OpenToBotTurnSchema = z.strictObject({
  userMessageParts: z.array(z.union([ToBotTextPartSchema, ToBotFilePartSchema])).min(1),
  hostContext: ToBotHostContextSchema.optional(),
  turnContext: ToBotTurnContextSchema.optional(),
  harnessComponents: z.array(z.string().min(1)).min(1).optional(),
  orgId: z.string().uuid().optional()
})
export type OpenToBotTurnBody = z.infer<typeof OpenToBotTurnSchema>

export interface OpenToBotTurnResponse {
  mode: string
}

export interface EnsureToBotAgentResponse {
  agentId: string
  sessionId: string
  assetId: string
  releaseId: string
  providerKind: string
  model: string
  warm: boolean
}

export interface DeleteAgentSessionsResult {
  deletedIds: string[]
}

/** Response for `GET /agent-sessions/latest` — the most-recently-active session in the requested scope, or `null`. */
export interface LatestAgentSessionResponse {
  session: AgentSessionEntity | null
}

/** The reusable empty session selected or created for the exact target. */
export interface ReusableAgentSessionPlaceholdersResponse {
  session: AgentSessionEntity
  created: boolean
  deletedDuplicateSessionIds: string[]
}

// ============================================================================
// API Schema definitions
// ============================================================================

export type AgentSessionSchemas = {
  '/agent-sessions': {
    GET: {
      query?: ListAgentSessionsQueryParams
      response: CursorPaginationResponse<AgentSessionEntity> & { total?: number; archiveEpoch?: string }
    }
    POST: {
      body: CreateAgentSessionDto
      response: AgentSessionEntity
    }
  }

  /**
   * Most-recently-active session, globally or within one owner scope.
   *
   * First-entry restore reads this to resume the last-touched session. Declared
   * before `/agent-sessions/:sessionId` and matched exactly by the server router,
   * so `latest` is never mistaken for a session id. Proves global latest via
   * `lastActivityAt DESC LIMIT 1`, unlike the `orderKey`-paged `/agent-sessions` first
   * page. `agentId=unlinked` covers sessions without a live agent.
   */
  /**
   * Warm the platform a3s-code process bound to the messages ToBot thread.
   * Declared as its own path so `tobot` is not a caller-chosen session id.
   */
  '/agent-sessions/tobot/ensure': {
    POST: {
      body: EnsureToBotAgentBody
      response: EnsureToBotAgentResponse
    }
  }

  /** Clear the ToBot transcript and stop the live turn. The session id stays. */
  '/agent-sessions/tobot/reset': {
    POST: {
      body: Record<string, never>
      response: { ok: boolean }
    }
  }

  /** One user turn on the long-lived ToBot a3s-code session. */
  '/agent-sessions/tobot/turns': {
    POST: {
      body: OpenToBotTurnBody
      response: OpenToBotTurnResponse
    }
  }

  /** `attached` while ToBot is still in a turn, otherwise `not-found`. */
  '/agent-sessions/tobot/turn': {
    GET: {
      response: { status: string }
    }
  }

  /** Warm the platform a3s-code process bound to the messages ToBoss thread. */
  '/agent-sessions/toboss/ensure': {
    POST: {
      body: EnsureToBotAgentBody
      response: EnsureToBotAgentResponse
    }
  }

  /** Clear the ToBoss transcript and stop the live turn. The session id stays. */
  '/agent-sessions/toboss/reset': {
    POST: {
      body: Record<string, never>
      response: { ok: boolean }
    }
  }

  /** One user turn on the long-lived ToBoss a3s-code session. */
  '/agent-sessions/toboss/turns': {
    POST: {
      body: OpenToBotTurnBody
      response: OpenToBotTurnResponse
    }
  }

  /** `attached` while ToBoss is still in a turn, otherwise `not-found`. */
  '/agent-sessions/toboss/turn': {
    GET: {
      response: { status: string }
    }
  }

  '/agent-sessions/latest': {
    GET: {
      query?: LatestAgentSessionQuery
      response: LatestAgentSessionResponse
    }
  }

  '/agent-sessions/:sessionId': {
    GET: {
      params: { sessionId: string }
      response: AgentSessionEntity
    }
    PATCH: {
      params: { sessionId: string }
      body: UpdateAgentSessionDto
      response: AgentSessionEntity
    }
  }

  '/agent-sessions/:sessionId/workspace': {
    /**
     * Replace the session's workspace. Only permitted while the session has no
     * messages — once a conversation has started the binding is permanent
     * (NOT_FOUND if the session is missing, INVALID_OPERATION if it already has
     * messages).
     *
     * Side effects: switching away from a system workspace deletes that backing
     * row; switching to `{ type: 'system' }` creates a fresh system workspace.
     */
    PUT: {
      params: { sessionId: string }
      body: SetAgentSessionWorkspaceDto
      response: AgentSessionEntity
    }
  }
} & OrderEndpoints<'/agent-sessions'>
