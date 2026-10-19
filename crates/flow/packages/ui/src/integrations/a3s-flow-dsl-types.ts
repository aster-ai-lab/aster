import type { JsonObject, JsonValue } from '@a3s-lab/form/core';

export const A3S_FLOW_ENGINE_VERSION = '1.0.0' as const;
export const A3S_FLOW_TESTED_WORKFLOW_DSL_VERSION = '0.7.0' as const;
/** Canonical execution digest format shared with the Rust engine. */
export const A3S_FLOW_EXECUTION_DIGEST_VERSION = 'v2' as const;
export const A3S_FLOW_WORKFLOW_DSL_MAX_BYTES = 10 * 1024 * 1024;
export const A3S_FLOW_WORKFLOW_DAG_MAX_NODES = 10_000;
export const A3S_FLOW_WORKFLOW_DAG_MAX_EDGES = 100_000;

export const A3S_FLOW_CONTRACT_PROVENANCE = Object.freeze({
  repository: 'https://github.com/A3S-Lab/Flow',
  revision: '006e988b1f63e92a381f138d10af4084b96625a8',
  engineVersion: A3S_FLOW_ENGINE_VERSION,
  executionDigestVersion: A3S_FLOW_EXECUTION_DIGEST_VERSION,
  workflowDslVersion: A3S_FLOW_TESTED_WORKFLOW_DSL_VERSION,
  maximumDocumentBytes: A3S_FLOW_WORKFLOW_DSL_MAX_BYTES,
  maximumNodes: A3S_FLOW_WORKFLOW_DAG_MAX_NODES,
  maximumEdges: A3S_FLOW_WORKFLOW_DAG_MAX_EDGES,
} as const);

export const A3S_FLOW_V1_COMPATIBILITY = Object.freeze([
  `a3s-flow/${A3S_FLOW_ENGINE_VERSION}`,
  `a3s-flow-workflow-dsl/${A3S_FLOW_TESTED_WORKFLOW_DSL_VERSION}`,
] as const);

export type A3sFlowWorkflowDslCompatibility =
  | 'compatible'
  | 'compatible_with_warnings'
  | 'requires_confirmation';

export interface A3sFlowWorkflowDagNodeData extends JsonObject {
  type: string;
}

/**
 * Lossless Flow DAG node. Unknown top-level fields are authoring/presentation
 * extensions unless the Flow semantic digest explicitly retains them.
 */
export interface A3sFlowWorkflowDagNode {
  id: string;
  data: A3sFlowWorkflowDagNodeData;
  parentId?: string;
  [field: string]: unknown;
}

/** Lossless directed Flow DAG edge. */
export interface A3sFlowWorkflowDagEdge {
  id: string;
  source: string;
  target: string;
  sourceHandle?: string;
  targetHandle?: string;
  data?: JsonValue;
  [field: string]: unknown;
}

export interface A3sFlowWorkflowDag {
  nodes: A3sFlowWorkflowDagNode[];
  edges: A3sFlowWorkflowDagEdge[];
  viewport?: JsonValue;
  [field: string]: unknown;
}

export interface A3sFlowWorkflowDslApp {
  name: string;
  mode: string;
  [field: string]: unknown;
}

export interface A3sFlowWorkflowDslBody {
  graph: A3sFlowWorkflowDag;
  [field: string]: unknown;
}

/** Lossless app document accepted by A3S Flow's v1 workflow DSL importer. */
export interface A3sFlowWorkflowDsl {
  version: string;
  kind: string;
  app: A3sFlowWorkflowDslApp;
  dependencies: JsonValue[];
  workflow: A3sFlowWorkflowDslBody;
  [field: string]: unknown;
}

export interface A3sFlowDslIssue {
  code: string;
  path: string;
  message: string;
}

export interface A3sFlowWorkflowDagPlan {
  topLevel: string[];
  scopes: Record<string, string[]>;
}

export type A3sFlowWorkflowDagCompilation =
  | { ok: true; plan: A3sFlowWorkflowDagPlan }
  | { ok: false; issues: A3sFlowDslIssue[] };

export type A3sFlowWorkflowDslValidation =
  | { ok: true; compatibility: A3sFlowWorkflowDslCompatibility }
  | { ok: false; issues: A3sFlowDslIssue[] };

export type A3sFlowWorkflowDslParseResult =
  | {
      ok: true;
      document: A3sFlowWorkflowDsl;
      compatibility: A3sFlowWorkflowDslCompatibility;
    }
  | { ok: false; issues: A3sFlowDslIssue[] };
