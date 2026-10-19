import * as z from 'zod'

export const AIX_TREE_PROTOCOL_VERSION = 1
export const AIX_TREE_NODE_LIMIT = 1024
export const AIX_TREE_DEPTH_LIMIT = 32
export const AIX_TREE_RESPONSE_BYTES = 512 * 1024
export const AIX_TREE_MESSAGE_LIMIT = 4096

const id = z.string().regex(/^[A-Za-z0-9_.-]{1,128}$/)
const sequence = z.number().int().positive().max(Number.MAX_SAFE_INTEGER)
const node = z
  .object({
    id,
    parent: id.nullable(),
    label: z
      .string()
      .max(128)
      .refine((value) => value.trim().length > 0),
    description: z.string().max(240).optional(),
    hasChildren: z.boolean(),
    command: id.optional()
  })
  .strict()

export const aixTreeProviderMessageSchema = z.discriminatedUnion('type', [
  z.object({ protocolVersion: z.literal(AIX_TREE_PROTOCOL_VERSION), type: z.literal('aix.tree.ready') }).strict(),
  z
    .object({
      protocolVersion: z.literal(AIX_TREE_PROTOCOL_VERSION),
      type: z.literal('aix.tree.changed'),
      revision: sequence
    })
    .strict(),
  z
    .object({
      protocolVersion: z.literal(AIX_TREE_PROTOCOL_VERSION),
      type: z.literal('aix.tree.data'),
      id: sequence,
      nodes: z.array(node).max(AIX_TREE_NODE_LIMIT)
    })
    .strict()
    .superRefine((value, ctx) => {
      const nodes = new Map(value.nodes.map((item) => [item.id, item]))
      let valid = nodes.size === value.nodes.length
      for (const item of value.nodes) {
        const seen = new Set([item.id])
        let parent = item.parent
        while (parent !== null) {
          const ancestor = nodes.get(parent)
          if (!ancestor || !ancestor.hasChildren || seen.has(parent) || seen.size >= AIX_TREE_DEPTH_LIMIT) {
            valid = false
            break
          }
          seen.add(parent)
          parent = ancestor.parent
        }
      }
      if (!valid || new TextEncoder().encode(JSON.stringify(value)).length > AIX_TREE_RESPONSE_BYTES)
        ctx.addIssue({ code: 'custom', message: 'Invalid or oversized AIX tree forest' })
    })
])

export type AixTreeNode = z.infer<typeof node>
export type AixTreeAction = 'refresh' | 'expand' | 'collapse' | 'select' | 'invoke'

export interface AixTreeRequest {
  readonly protocolVersion: typeof AIX_TREE_PROTOCOL_VERSION
  readonly type: 'aix.tree.request'
  readonly id: number
  readonly action: AixTreeAction
  readonly nodeId: string | null
  readonly command?: string
}

export interface AixTreeSnapshot {
  readonly status: 'loading' | 'ready' | 'failed' | 'closed'
  readonly busy: boolean
  readonly nodes: readonly AixTreeNode[]
}
