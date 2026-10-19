import type { WorkspaceDocumentDiagnostic, WorkspaceDocumentSnapshot } from './workspaceDocument'

export type VsxDocumentHostMessage =
  | {
      readonly protocol: 'a3s.vsx.document/1'
      readonly sessionId: string
      readonly type: 'document'
      readonly snapshot: WorkspaceDocumentSnapshot
    }
  | {
      readonly protocol: 'a3s.vsx.document/1'
      readonly sessionId: string
      readonly type: 'accepted'
      readonly sequence: number
      readonly version: number
    }
  | {
      readonly protocol: 'a3s.vsx.document/1'
      readonly sessionId: string
      readonly type: 'refused'
      readonly reason: 'invalid' | 'stale' | 'busy' | 'unavailable'
    }

export interface VsxDocumentPublication {
  readonly protocol: 'a3s.vsx.document/1'
  readonly sessionId: string
  readonly type: 'diagnostics'
  readonly documentId: string
  readonly sequence: number
  readonly version: number
  readonly items: readonly WorkspaceDocumentDiagnostic[]
}
