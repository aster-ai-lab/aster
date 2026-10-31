import type {
  AsterFlowDslIssue,
  AsterFlowWorkflowDag,
  AsterFlowWorkflowDagCompilation,
  AsterFlowWorkflowDagEdge,
  AsterFlowWorkflowDagNode,
  AsterFlowWorkflowDsl,
} from "./aster-flow-dsl-types";

/** A serializable annotation supplied by a host designer. */
export interface AsterFlowDesignerAnnotation {
  id: string;
  kind?: string;
  text?: string;
  [field: string]: unknown;
}

export type AsterFlowDesignerSelection =
  | { kind: "canvas" }
  | { kind: "node"; id: string }
  | { kind: "edge"; id: string }
  | { kind: "annotation"; id: string };

export type AsterFlowDesignerSelectionInput =
  AsterFlowDesignerSelection | undefined | null;

export interface AsterFlowDesignerSelectionDetails {
  readonly kind: AsterFlowDesignerSelection["kind"];
  readonly id?: string;
  readonly node?: AsterFlowWorkflowDagNode;
  readonly edge?: AsterFlowWorkflowDagEdge;
  /** Endpoint projection for an edge selection. */
  readonly sourceNode?: AsterFlowWorkflowDagNode;
  readonly targetNode?: AsterFlowWorkflowDagNode;
  readonly annotation?: AsterFlowDesignerAnnotation;
  readonly incomingEdges: readonly AsterFlowWorkflowDagEdge[];
  readonly outgoingEdges: readonly AsterFlowWorkflowDagEdge[];
  readonly relatedNodes: readonly AsterFlowWorkflowDagNode[];
  readonly scopeNode?: AsterFlowWorkflowDagNode;
  readonly ancestorNodes: readonly AsterFlowWorkflowDagNode[];
}

/**
 * Read-only, serializable context passed to CLI, Skill, and Copilot adapters.
 *
 * The context deliberately contains both the complete DSL and a focused
 * selection projection. Adapters can inspect the graph without reaching into
 * React state, and hosts can safely serialize it across a process boundary.
 */
export interface AsterFlowDesignerContext {
  readonly dsl: AsterFlowWorkflowDsl;
  /** Alias for integrations that call the document `document`. */
  readonly document: AsterFlowWorkflowDsl;
  readonly documentJson: string;
  readonly graph: AsterFlowWorkflowDag;
  readonly nodes: readonly AsterFlowWorkflowDagNode[];
  readonly edges: readonly AsterFlowWorkflowDagEdge[];
  readonly annotations: readonly AsterFlowDesignerAnnotation[];
  readonly selection: AsterFlowDesignerSelectionDetails;
  readonly selectedNode?: AsterFlowWorkflowDagNode;
  readonly selectedEdge?: AsterFlowWorkflowDagEdge;
  readonly selectedAnnotation?: AsterFlowDesignerAnnotation;
  readonly compilation?: AsterFlowWorkflowDagCompilation;
  readonly issues: readonly AsterFlowDslIssue[];
  readonly metadata: Readonly<Record<string, unknown>>;
}

export interface CreateAsterFlowDesignerContextOptions {
  selection?: AsterFlowDesignerSelectionInput;
  annotations?: readonly AsterFlowDesignerAnnotation[];
  compilation?: AsterFlowWorkflowDagCompilation;
  issues?: readonly AsterFlowDslIssue[];
  metadata?: Readonly<Record<string, unknown>>;
  /** JSON indentation used by documentJson and defaults to two spaces. */
  space?: number | string;
}

function isObject(value: unknown): value is Record<string, unknown> {
  return value !== null && typeof value === "object";
}

function clone<T>(value: T): T {
  if (typeof structuredClone === "function") return structuredClone(value);
  return JSON.parse(JSON.stringify(value)) as T;
}

function freezeDeep<T>(value: T, seen = new WeakSet<object>()): T {
  if (!isObject(value) || seen.has(value)) return value;
  seen.add(value);
  for (const child of Object.values(value)) freezeDeep(child, seen);
  return Object.freeze(value);
}

function normalizeSelection(
  selection: AsterFlowDesignerSelectionInput,
): AsterFlowDesignerSelection {
  if (!selection) return { kind: "canvas" };
  if (selection.kind === "canvas") return { kind: "canvas" };
  if (
    selection.kind !== "node" &&
    selection.kind !== "edge" &&
    selection.kind !== "annotation"
  ) {
    return { kind: "canvas" };
  }
  if (typeof selection.id !== "string" || selection.id.length === 0) {
    return { kind: "canvas" };
  }
  return { kind: selection.kind, id: selection.id };
}

function ancestorsFor(
  node: AsterFlowWorkflowDagNode | undefined,
  nodesById: ReadonlyMap<string, AsterFlowWorkflowDagNode>,
): AsterFlowWorkflowDagNode[] {
  const ancestors: AsterFlowWorkflowDagNode[] = [];
  const visited = new Set<string>();
  let parentId = node?.parentId;
  while (parentId && !visited.has(parentId)) {
    visited.add(parentId);
    const parent = nodesById.get(parentId);
    if (!parent) break;
    ancestors.push(parent);
    parentId = parent.parentId;
  }
  return ancestors;
}

/**
 * Creates a frozen designer context from a complete workflow DSL.
 *
 * Inputs are cloned before freezing, so a renderer cannot accidentally mutate
 * the editor's live graph through a reference retained in the context.
 */
export function createAsterFlowDesignerContext(
  document: AsterFlowWorkflowDsl,
  options: CreateAsterFlowDesignerContextOptions = {},
): AsterFlowDesignerContext {
  const dsl = clone(document);
  const graph = clone(dsl.workflow.graph);
  const nodes = graph.nodes.map((node) => clone(node));
  const edges = graph.edges.map((edge) => clone(edge));
  const annotations = (options.annotations ?? []).map((annotation) =>
    clone(annotation),
  );
  const nodesById = new Map(nodes.map((node) => [node.id, node]));
  const edgesById = new Map(edges.map((edge) => [edge.id, edge]));
  const selection = normalizeSelection(options.selection);
  const selectedNode =
    selection.kind === "node" ? nodesById.get(selection.id) : undefined;
  const selectedEdge =
    selection.kind === "edge" ? edgesById.get(selection.id) : undefined;
  const sourceNode = selectedEdge
    ? nodesById.get(selectedEdge.source)
    : undefined;
  const targetNode = selectedEdge
    ? nodesById.get(selectedEdge.target)
    : undefined;
  const selectedAnnotation =
    selection.kind === "annotation"
      ? annotations.find(({ id }) => id === selection.id)
      : undefined;
  const focusedNode = selectedNode;
  const incomingEdges = focusedNode
    ? edges.filter(({ target }) => target === focusedNode.id)
    : selectedEdge
      ? edges.filter(({ id }) => id === selectedEdge.id)
      : [];
  const outgoingEdges = focusedNode
    ? edges.filter(({ source }) => source === focusedNode.id)
    : selectedEdge
      ? edges.filter(({ id }) => id === selectedEdge.id)
      : [];
  const relatedIds = new Set<string>();
  for (const edge of [...incomingEdges, ...outgoingEdges]) {
    relatedIds.add(edge.source);
    relatedIds.add(edge.target);
  }
  if (selectedEdge) {
    relatedIds.add(selectedEdge.source);
    relatedIds.add(selectedEdge.target);
  }
  const relatedNodes = nodes.filter((node) => relatedIds.has(node.id));
  const scopeNode = focusedNode?.parentId
    ? nodesById.get(focusedNode.parentId)
    : selectedEdge
      ? nodesById.get(nodesById.get(selectedEdge.source)?.parentId ?? "")
      : undefined;
  const ancestorNodes = ancestorsFor(focusedNode ?? sourceNode, nodesById);
  const frozenGraph = freezeDeep({
    ...graph,
    nodes,
    edges,
  });
  const frozenNodes = frozenGraph.nodes;
  const frozenEdges = frozenGraph.edges;
  const frozenAnnotations = freezeDeep(annotations);
  const frozenSelection = freezeDeep({
    ...selection,
    id: "id" in selection ? selection.id : undefined,
    node: selectedNode,
    edge: selectedEdge,
    sourceNode,
    targetNode,
    annotation: selectedAnnotation,
    incomingEdges,
    outgoingEdges,
    relatedNodes,
    scopeNode,
    ancestorNodes,
  });
  const frozenDsl = freezeDeep({
    ...dsl,
    workflow: { ...dsl.workflow, graph: frozenGraph },
  });
  const documentJson = JSON.stringify(frozenDsl, null, options.space ?? 2);
  const context = {
    dsl: frozenDsl,
    document: frozenDsl,
    documentJson,
    graph: frozenGraph,
    nodes: frozenNodes,
    edges: frozenEdges,
    annotations: frozenAnnotations,
    selection: frozenSelection,
    selectedNode,
    selectedEdge,
    selectedAnnotation,
    compilation: options.compilation
      ? freezeDeep(clone(options.compilation))
      : undefined,
    issues: freezeDeep(clone(options.issues ?? [])),
    metadata: freezeDeep(clone(options.metadata ?? {})),
  } satisfies AsterFlowDesignerContext;
  return freezeDeep(context);
}

/** Serializes only the safe, process-boundary representation of a context. */
export function serializeAsterFlowDesignerContext(
  context: AsterFlowDesignerContext,
  space: number | string = 2,
): string {
  return JSON.stringify(
    {
      dsl: context.dsl,
      selection: context.selection,
      annotations: context.annotations,
      compilation: context.compilation,
      issues: context.issues,
      metadata: context.metadata,
    },
    null,
    space,
  );
}
