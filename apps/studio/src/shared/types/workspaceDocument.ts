/** One native model observation. This value does not itself grant resource access. */
export interface WorkspaceDocumentSnapshot {
  readonly documentId: string
  readonly uri: string
  readonly version: number
  readonly text: string
  readonly dirty: boolean
  readonly diskConflict: boolean
}

export interface WorkspaceDocumentLease {
  readonly lease: string
  readonly snapshot: WorkspaceDocumentSnapshot
}

export interface WorkspaceDocumentPermissions {
  readonly edit: boolean
  readonly save: boolean
}

/** Positions use zero-based lines and UTF-16 character offsets. */
export interface WorkspaceDocumentDiagnostic {
  readonly start: { readonly line: number; readonly character: number }
  readonly end: { readonly line: number; readonly character: number }
  readonly severity: 'error' | 'warning' | 'information' | 'hint'
  readonly message: string
}

export interface WorkspaceDiagnosticPublication {
  readonly publisherId: string
  readonly sequence: number
  readonly version: number
  readonly items: readonly WorkspaceDocumentDiagnostic[]
}

export interface WorkspaceDiagnosticObservation {
  readonly snapshot: WorkspaceDocumentSnapshot
  readonly diagnostics: readonly WorkspaceDiagnosticPublication[]
}

export interface WorkspaceDocumentReceipt {
  readonly version: number
  readonly dirty: boolean
  readonly diskConflict: boolean
  readonly retained: boolean
}

export interface WorkspaceDocumentRecovery {
  readonly id: string
  readonly revision: number
  readonly baseVersion: number
  readonly bytes: number
  readonly canResolve: boolean
}

export interface WorkspaceDocumentRecoveryReview {
  readonly snapshot: WorkspaceDocumentSnapshot
  readonly recovery: WorkspaceDocumentRecovery
  readonly text: string
}

export type WorkspaceDocumentRequest =
  | { readonly operation: 'open'; readonly root: string; readonly path: string }
  | { readonly operation: 'read'; readonly lease: string }
  | { readonly operation: 'delegate'; readonly lease: string; readonly permissions: WorkspaceDocumentPermissions }
  | { readonly operation: 'delegate_diagnostics'; readonly lease: string }
  | { readonly operation: 'diagnostics'; readonly lease: string }
  | {
      readonly operation: 'publish_diagnostics'
      readonly lease: string
      readonly sequence: number
      readonly version: number
      readonly items: readonly WorkspaceDocumentDiagnostic[]
    }
  | { readonly operation: 'close'; readonly lease: string }
  | { readonly operation: 'recoveries'; readonly lease: string }
  | { readonly operation: 'review_recovery'; readonly lease: string; readonly id: string }
  | {
      readonly operation: 'resolve_recovery'
      readonly lease: string
      readonly sequence: number
      readonly version: number
      readonly id: string
      readonly revision: number
      readonly restore: boolean
    }
  | { readonly operation: 'watch'; readonly lease: string; readonly version: number; readonly diskConflict: boolean }
  | {
      readonly operation: 'edit' | 'input'
      readonly lease: string
      readonly sequence: number
      readonly version: number
      readonly text: string
    }
  | {
      readonly operation: 'save' | 'discard'
      readonly lease: string
      readonly sequence: number
      readonly version: number
    }
