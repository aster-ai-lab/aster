import { canonicalize, sha256 } from '@aster-ai-lab/form/core';
import type { JsonValue } from '@aster-ai-lab/form/core';
import { compileAsterFlowWorkflowDag } from './aster-flow-dag';
import { parseAsterFlowStrictJson } from '../strict-json';
import {
  ASTER_FLOW_TESTED_WORKFLOW_DSL_VERSION,
  ASTER_FLOW_EXECUTION_DIGEST_VERSION,
  ASTER_FLOW_WORKFLOW_DSL_MAX_BYTES,
  type AsterFlowDslIssue,
  type AsterFlowWorkflowDsl,
  type AsterFlowWorkflowDslCompatibility,
  type AsterFlowWorkflowDslParseResult,
  type AsterFlowWorkflowDslValidation,
} from './aster-flow-dsl-types';

const DOCUMENT_EXECUTION_DIGEST_DOMAIN = `aster.flow.workflow_dsl.execution.${ASTER_FLOW_EXECUTION_DIGEST_VERSION}\0`;
const GRAPH_EXECUTION_DIGEST_DOMAIN = `aster.flow.workflow_dag.execution.${ASTER_FLOW_EXECUTION_DIGEST_VERSION}\0`;
const NODE_PRESENTATION_FIELDS = [
  'draggable',
  'height',
  'position',
  'positionAbsolute',
  'selected',
  'selectable',
  'sourcePosition',
  'targetPosition',
  'type',
  'width',
  'zIndex',
] as const;
const NODE_DATA_PRESENTATION_FIELDS = ['desc', 'height', 'selected', 'title', 'width'] as const;
const EDGE_PRESENTATION_FIELDS = [
  'animated',
  'hidden',
  'label',
  'selected',
  'style',
  'type',
  'zIndex',
] as const;
const utf8Encoder = new TextEncoder();

interface Semver {
  major: number;
  minor: number;
  patch: number;
  prerelease: string[];
}

function isRecord(value: unknown): value is Record<string, unknown> {
  return value !== null && typeof value === 'object' && !Array.isArray(value);
}

function parseSemver(value: string): Semver | undefined {
  const match =
    /^(0|[1-9]\d*)\.(0|[1-9]\d*)\.(0|[1-9]\d*)(?:-([0-9A-Za-z-]+(?:\.[0-9A-Za-z-]+)*))?(?:\+[0-9A-Za-z-]+(?:\.[0-9A-Za-z-]+)*)?$/.exec(
      value,
    );
  if (!match) return undefined;
  return {
    major: Number(match[1]),
    minor: Number(match[2]),
    patch: Number(match[3]),
    prerelease: match[4]?.split('.') ?? [],
  };
}

function compareIdentifiers(left: string, right: string): number {
  const leftNumeric = /^\d+$/.test(left);
  const rightNumeric = /^\d+$/.test(right);
  if (leftNumeric && rightNumeric) return Number(left) - Number(right);
  if (leftNumeric !== rightNumeric) return leftNumeric ? -1 : 1;
  return left < right ? -1 : left > right ? 1 : 0;
}

function compareUtf8(left: string, right: string): number {
  const leftBytes = utf8Encoder.encode(left);
  const rightBytes = utf8Encoder.encode(right);
  const length = Math.min(leftBytes.length, rightBytes.length);
  for (let index = 0; index < length; index += 1) {
    const difference = leftBytes[index] - rightBytes[index];
    if (difference !== 0) return difference;
  }
  return leftBytes.length - rightBytes.length;
}

function compareSemver(left: Semver, right: Semver): number {
  for (const key of ['major', 'minor', 'patch'] as const) {
    const difference = left[key] - right[key];
    if (difference !== 0) return difference;
  }
  if (left.prerelease.length === 0) return right.prerelease.length === 0 ? 0 : 1;
  if (right.prerelease.length === 0) return -1;
  const length = Math.max(left.prerelease.length, right.prerelease.length);
  for (let index = 0; index < length; index += 1) {
    const leftIdentifier = left.prerelease[index];
    const rightIdentifier = right.prerelease[index];
    if (leftIdentifier === undefined) return -1;
    if (rightIdentifier === undefined) return 1;
    const difference = compareIdentifiers(leftIdentifier, rightIdentifier);
    if (difference !== 0) return difference;
  }
  return 0;
}

export function classifyAsterFlowWorkflowDslVersion(
  importedVersion: string,
): AsterFlowWorkflowDslCompatibility | 'invalid' {
  const current = parseSemver(ASTER_FLOW_TESTED_WORKFLOW_DSL_VERSION);
  const imported = parseSemver(importedVersion);
  if (!current || !imported) return 'invalid';
  if (compareSemver(imported, current) > 0 || imported.major < current.major) {
    return 'requires_confirmation';
  }
  if (imported.minor < current.minor) return 'compatible_with_warnings';
  return 'compatible';
}

function invalid(code: string, path: string, message: string): AsterFlowWorkflowDslValidation {
  return { ok: false, issues: [{ code, path, message }] };
}

export function validateAsterFlowWorkflowDsl(value: unknown): AsterFlowWorkflowDslValidation {
  if (!isRecord(value))
    return invalid('flow.dsl.invalid_shape', '', 'Workflow DSL must be an object.');
  if (typeof value.version !== 'string' || !value.version.trim()) {
    return invalid('flow.dsl.version', 'version', 'Workflow DSL version is empty.');
  }
  const compatibility = classifyAsterFlowWorkflowDslVersion(value.version);
  if (compatibility === 'invalid') {
    return invalid('flow.dsl.version', 'version', 'Workflow DSL version is not valid SemVer.');
  }
  if (value.kind !== 'app') {
    return invalid('flow.dsl.kind', 'kind', 'Workflow DSL kind must be app.');
  }
  if (!isRecord(value.app)) {
    return invalid('flow.dsl.app', 'app', 'Workflow DSL app metadata must be an object.');
  }
  if (typeof value.app.name !== 'string' || !value.app.name.trim()) {
    return invalid('flow.dsl.app_name', 'app.name', 'Workflow DSL app name is empty.');
  }
  if (value.app.mode !== 'workflow') {
    return invalid(
      'flow.dsl.app_mode',
      'app.mode',
      'Workflow DSL app mode must be workflow.',
    );
  }
  if (!isRecord(value.workflow) || !isRecord(value.workflow.graph)) {
    return invalid('flow.dsl.graph', 'workflow.graph', 'Workflow DSL graph is missing.');
  }
  return { ok: true, compatibility };
}

export function parseAsterFlowWorkflowDslJson(source: string): AsterFlowWorkflowDslParseResult {
  const actualBytes = new TextEncoder().encode(source).byteLength;
  if (actualBytes > ASTER_FLOW_WORKFLOW_DSL_MAX_BYTES) {
    return {
      ok: false,
      issues: [
        {
          code: 'flow.dsl.document_too_large',
          path: '',
          message: `Workflow DSL document is ${actualBytes} bytes; maximum is ${ASTER_FLOW_WORKFLOW_DSL_MAX_BYTES} bytes.`,
        },
      ],
    };
  }
  let value: unknown;
  try {
    value = parseAsterFlowStrictJson(source);
  } catch (error) {
    return {
      ok: false,
      issues: [
        {
          code: 'flow.dsl.invalid_json',
          path: '',
          message: error instanceof Error ? error.message : 'Workflow DSL JSON is invalid.',
        },
      ],
    };
  }
  const validation = validateAsterFlowWorkflowDsl(value);
  if (!validation.ok) return validation;
  return {
    ok: true,
    document: value as AsterFlowWorkflowDsl,
    compatibility: validation.compatibility,
  };
}

function cloneRecord(value: unknown, label: string): Record<string, unknown> {
  if (!isRecord(value)) throw new TypeError(`${label} must be an object.`);
  return structuredClone(value);
}

function compareId(left: Record<string, unknown>, right: Record<string, unknown>): number {
  return compareUtf8(String(left.id), String(right.id));
}

function normalizeGraph(graphValue: unknown): Record<string, unknown> {
  const graph = cloneRecord(graphValue, 'Workflow DAG');
  delete graph.viewport;
  const rawNodes = Array.isArray(graph.nodes) ? graph.nodes : [];
  graph.nodes = rawNodes
    .map((value) => {
      const node = cloneRecord(value, 'Workflow DAG node');
      for (const field of NODE_PRESENTATION_FIELDS) delete node[field];
      const data = cloneRecord(node.data, 'Workflow DAG node.data');
      for (const field of NODE_DATA_PRESENTATION_FIELDS) delete data[field];
      node.data = data;
      return node;
    })
    .sort(compareId);
  const rawEdges = Array.isArray(graph.edges) ? graph.edges : [];
  graph.edges = rawEdges
    .map((value) => {
      const edge = cloneRecord(value, 'Workflow DAG edge');
      for (const field of EDGE_PRESENTATION_FIELDS) delete edge[field];
      if (!('data' in edge)) edge.data = null;
      return edge;
    })
    .sort(compareId);
  return graph;
}

function assertCanonicalDigestNumbers(
  value: unknown,
  path = '$',
  ancestors = new Set<object>(),
): void {
  if (typeof value === 'number') {
    if (!Number.isFinite(value)) {
      throw new TypeError(`Canonical execution digest does not support non-finite number at ${path}.`);
    }
    if (Number.isInteger(value) && !Number.isSafeInteger(value)) {
      throw new TypeError(
        `Canonical execution digest does not support unsafe integer at ${path}; use a string instead.`,
      );
    }
    return;
  }
  if (Array.isArray(value)) {
    if (ancestors.has(value)) {
      throw new TypeError(`Canonical execution digest does not support cyclic values at ${path}.`);
    }
    ancestors.add(value);
    try {
      value.forEach((item, index) =>
        assertCanonicalDigestNumbers(item, `${path}[${index}]`, ancestors),
      );
    } finally {
      ancestors.delete(value);
    }
    return;
  }
  if (isRecord(value)) {
    if (ancestors.has(value)) {
      throw new TypeError(`Canonical execution digest does not support cyclic values at ${path}.`);
    }
    ancestors.add(value);
    try {
      Object.entries(value).forEach(([key, child]) => {
        if (child !== undefined) {
          assertCanonicalDigestNumbers(child, `${path}.${key}`, ancestors);
        }
      });
    } finally {
      ancestors.delete(value);
    }
  }
}

/**
 * Encode JSON using the cross-language Flow canonical form.
 *
 * Besides deterministic key ordering and number formatting, this helper
 * rejects values that JavaScript cannot represent safely before they become a
 * digest or an authoring operation. It is intentionally exported so hosts can
 * canonicalize operation bytes before deriving idempotency keys.
 */
export function canonicalizeAsterFlowJson(value: JsonValue): string {
  assertCanonicalDigestNumbers(value);
  return canonicalize(value);
}

function normalizedDocument(document: AsterFlowWorkflowDsl): JsonValue {
  const normalized = cloneRecord(document, 'Workflow DSL document');
  normalized.dependencies = Array.isArray(document.dependencies)
    ? structuredClone(document.dependencies)
    : [];
  normalized.app = { mode: document.app.mode };
  const workflow = cloneRecord(document.workflow, 'Workflow DSL body');
  workflow.graph = normalizeGraph(document.workflow.graph);
  normalized.workflow = workflow;
  return normalized as JsonValue;
}

function ensureExecutable(document: AsterFlowWorkflowDsl): void {
  const validation = validateAsterFlowWorkflowDsl(document);
  if (!validation.ok) throw new TypeError(validation.issues[0]?.message);
  const compilation = compileAsterFlowWorkflowDag(document.workflow.graph);
  if (!compilation.ok) throw new TypeError(compilation.issues[0]?.message);
}

export function digestAsterFlowWorkflowDsl(document: AsterFlowWorkflowDsl): string {
  ensureExecutable(document);
  const normalized = normalizedDocument(document);
  return sha256(`${DOCUMENT_EXECUTION_DIGEST_DOMAIN}${canonicalizeAsterFlowJson(normalized)}`);
}

export function digestAsterFlowWorkflowDag(graph: AsterFlowWorkflowDsl['workflow']['graph']): string {
  const compilation = compileAsterFlowWorkflowDag(graph);
  if (!compilation.ok) throw new TypeError(compilation.issues[0]?.message);
  const normalized = normalizeGraph(graph) as JsonValue;
  return sha256(
    `${GRAPH_EXECUTION_DIGEST_DOMAIN}${canonicalizeAsterFlowJson(normalized)}`,
  );
}

export * from './aster-flow-dag';
export * from './aster-flow-dsl-types';
export type { AsterFlowDslIssue };
