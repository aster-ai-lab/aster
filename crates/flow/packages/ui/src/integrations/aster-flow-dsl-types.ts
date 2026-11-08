import type { JsonObject, JsonValue } from '@aster-ai-lab/form/core';

export const ASTER_FLOW_ENGINE_VERSION = '1.0.0' as const;
export const ASTER_FLOW_TESTED_WORKFLOW_DSL_VERSION = '0.7.0' as const;
/** Canonical execution digest format shared with the Rust engine. */
export const ASTER_FLOW_EXECUTION_DIGEST_VERSION = 'v2' as const;
export const ASTER_FLOW_WORKFLOW_DSL_MAX_BYTES = 10 * 1024 * 1024;
export const ASTER_FLOW_WORKFLOW_DAG_MAX_NODES = 10_000;
export const ASTER_FLOW_WORKFLOW_DAG_MAX_EDGES = 100_000;

export const ASTER_FLOW_CONTRACT_PROVENANCE = Object.freeze({
  repository: 'https://github.com/aster-ai-lab/Flow',
  revision: '006e988b1f63e92a381f138d10af4084b96625a8',
  engineVersion: ASTER_FLOW_ENGINE_VERSION,
  executionDigestVersion: ASTER_FLOW_EXECUTION_DIGEST_VERSION,
  workflowDslVersion: ASTER_FLOW_TESTED_WORKFLOW_DSL_VERSION,
  maximumDocumentBytes: ASTER_FLOW_WORKFLOW_DSL_MAX_BYTES,
  maximumNodes: ASTER_FLOW_WORKFLOW_DAG_MAX_NODES,
  maximumEdges: ASTER_FLOW_WORKFLOW_DAG_MAX_EDGES,
} as const);

export const ASTER_FLOW_V1_COMPATIBILITY = Object.freeze([
  `aster-flow/${ASTER_FLOW_ENGINE_VERSION}`,
  `aster-flow-workflow-dsl/${ASTER_FLOW_TESTED_WORKFLOW_DSL_VERSION}`,
] as const);

export type AsterFlowWorkflowDslCompatibility =
  | 'compatible'
  | 'compatible_with_warnings'
  | 'requires_confirmation';

export interface AsterFlowWorkflowDagNodeData extends JsonObject {
  type: string;
}

/**
 * Lossless Flow DAG node. Unknown top-level fields are authoring/presentation
 * extensions unless the Flow semantic digest explicitly retains them.
 */
export interface AsterFlowWorkflowDagNode {
  id: string;
  data: AsterFlowWorkflowDagNodeData;
  parentId?: string;
  [field: string]: unknown;
}

/** Lossless directed Flow DAG edge. */
export interface AsterFlowWorkflowDagEdge {
  id: string;
  source: string;
  target: string;
  sourceHandle?: string;
  targetHandle?: string;
  data?: JsonValue;
  [field: string]: unknown;
}

export interface AsterFlowWorkflowDag {
  nodes: AsterFlowWorkflowDagNode[];
  edges: AsterFlowWorkflowDagEdge[];
  viewport?: JsonValue;
  [field: string]: unknown;
}

export interface AsterFlowWorkflowDslApp {
  name: string;
  mode: string;
  [field: string]: unknown;
}

export interface AsterFlowWorkflowDslBody {
  graph: AsterFlowWorkflowDag;
  [field: string]: unknown;
}

/** Lossless app document accepted by Aster Flow's v1 workflow DSL importer. */
export interface AsterFlowWorkflowDsl {
  version: string;
  kind: string;
  app: AsterFlowWorkflowDslApp;
  dependencies: JsonValue[];
  workflow: AsterFlowWorkflowDslBody;
  [field: string]: unknown;
}

export interface AsterFlowDslIssue {
  code: string;
  path: string;
  message: string;
}

export interface AsterFlowWorkflowDagPlan {
  topLevel: string[];
  scopes: Record<string, string[]>;
}

export type AsterFlowWorkflowDagCompilation =
  | { ok: true; plan: AsterFlowWorkflowDagPlan }
  | { ok: false; issues: AsterFlowDslIssue[] };

export type AsterFlowWorkflowDslValidation =
  | { ok: true; compatibility: AsterFlowWorkflowDslCompatibility }
  | { ok: false; issues: AsterFlowDslIssue[] };

export type AsterFlowWorkflowDslParseResult =
  | {
      ok: true;
      document: AsterFlowWorkflowDsl;
      compatibility: AsterFlowWorkflowDslCompatibility;
    }
  | { ok: false; issues: AsterFlowDslIssue[] };
