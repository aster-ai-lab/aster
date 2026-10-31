import {
  asterFlowDagNodeRegistry,
  createAsterFlowDagNodeRegistry,
  defineAsterFlowDagNodeManifest,
  type AsterFlowDagNodeManifest,
  type AsterFlowDagNodeManifestInput,
  type AsterFlowDagNodeRegistry,
} from './aster-flow-node-manifest';

/** Exact host capability required to publish one custom node type. */
export interface AsterFlowDagNodeCapabilityBinding {
  nodeType: string;
  id: string;
  version: string;
  handler: string;
}

export interface AsterFlowDagNodeCapabilityBindingInput {
  nodeType?: string;
  id: string;
  version: string;
  handler: string;
}

export interface AsterFlowCustomDagNodeRegistration {
  manifest: AsterFlowDagNodeManifest;
  capability: AsterFlowDagNodeCapabilityBinding;
}

export interface AsterFlowDagNodeCapabilityRegistry {
  get(type: string): AsterFlowDagNodeCapabilityBinding | undefined;
  require(type: string): AsterFlowDagNodeCapabilityBinding;
  list(): readonly AsterFlowDagNodeCapabilityBinding[];
}

/** Immutable host catalog composed from the built-ins and explicit custom registrations. */
export interface AsterFlowDagNodeCatalog {
  registry: AsterFlowDagNodeRegistry;
  capabilities: AsterFlowDagNodeCapabilityRegistry;
  custom: readonly AsterFlowCustomDagNodeRegistration[];
}

export interface DefineAsterFlowCustomDagNodeInput {
  manifest: AsterFlowDagNodeManifestInput | AsterFlowDagNodeManifest;
  capability: AsterFlowDagNodeCapabilityBindingInput;
}

const EXACT_SEMVER =
  /^(?:0|[1-9]\d*)\.(?:0|[1-9]\d*)\.(?:0|[1-9]\d*)(?:-[0-9A-Za-z-]+(?:\.[0-9A-Za-z-]+)*)?(?:\+[0-9A-Za-z-]+(?:\.[0-9A-Za-z-]+)*)?$/u;
const CUSTOM_NODE_TYPE = /^[a-z][a-z0-9-]*(?:\.[a-z][a-z0-9-]*){2,}$/u;
const CAPABILITY_ID = /^[a-z][a-z0-9-]*(?:[/.][a-z][a-z0-9-]*)+$/u;
const RESERVED_CUSTOM_NODE_TYPES = new Set([
  'iteration',
  'iteration-start',
  'loop',
  'loop-start',
]);

function nonEmpty(value: string, label: string): void {
  if (!value.trim()) throw new TypeError(`${label} must not be empty.`);
}

function validateCustomNodeType(type: string): void {
  if (type.startsWith('flow.') || RESERVED_CUSTOM_NODE_TYPES.has(type)) {
    throw new TypeError(`Custom Aster Flow DAG node type ${type} is reserved.`);
  }
  if (!CUSTOM_NODE_TYPE.test(type)) {
    throw new TypeError(
      `Custom Aster Flow DAG node type ${type} must contain at least three lowercase namespace segments.`,
    );
  }
}

function normalizeCapabilityBinding(
  nodeType: string,
  input: AsterFlowDagNodeCapabilityBindingInput,
): AsterFlowDagNodeCapabilityBinding {
  if (input.nodeType !== undefined && input.nodeType !== nodeType) {
    throw new TypeError(
      `Capability binding node type ${input.nodeType} does not match custom node ${nodeType}.`,
    );
  }
  nonEmpty(input.id, `Custom Aster Flow DAG node ${nodeType} capability ID`);
  if (!CAPABILITY_ID.test(input.id)) {
    throw new TypeError(
      `Custom Aster Flow DAG node ${nodeType} capability ID must be namespaced.`,
    );
  }
  if (!EXACT_SEMVER.test(input.version)) {
    throw new TypeError(
      `Custom Aster Flow DAG node ${nodeType} capability version must be an exact semantic version.`,
    );
  }
  nonEmpty(input.handler, `Custom Aster Flow DAG node ${nodeType} handler`);
  return Object.freeze({
    nodeType,
    id: input.id,
    version: input.version,
    handler: input.handler,
  });
}

/** Checks a capability supplied by an external registry at the publication boundary. */
export function isAsterFlowDagNodeCapabilityBindingValid(
  binding: AsterFlowDagNodeCapabilityBinding,
  nodeType: string = binding.nodeType,
): boolean {
  return (
    binding.nodeType === nodeType &&
    CAPABILITY_ID.test(binding.id) &&
    EXACT_SEMVER.test(binding.version) &&
    binding.handler.trim().length > 0
  );
}

/** Defines a public host node together with the exact executor capability that authorizes it. */
export function defineAsterFlowCustomDagNode(
  input: DefineAsterFlowCustomDagNodeInput,
): AsterFlowCustomDagNodeRegistration {
  validateCustomNodeType(input.manifest.type);
  if (input.manifest.role !== 'host') {
    throw new TypeError(
      `Custom Aster Flow DAG node ${input.manifest.type} must use the host role.`,
    );
  }
  if (input.manifest.internal === true) {
    throw new TypeError(
      `Custom Aster Flow DAG node ${input.manifest.type} must be public.`,
    );
  }
  if (
    input.manifest.runtimeBinding !== undefined ||
    input.manifest.container !== undefined
  ) {
    throw new TypeError(
      `Custom Aster Flow DAG node ${input.manifest.type} cannot claim Flow runtime or container bindings.`,
    );
  }
  const manifest = defineAsterFlowDagNodeManifest({
    ...input.manifest,
    role: 'host',
    internal: false,
    official: input.manifest.official ?? false,
  });
  const capability = normalizeCapabilityBinding(
    manifest.type,
    input.capability,
  );
  return Object.freeze({ manifest, capability });
}

/**
 * Extends a node registry without changing the built-in singleton. Custom
 * manifests and their executor bindings are admitted as one atomic catalog.
 */
export function createAsterFlowDagNodeCatalog(
  registrations: readonly AsterFlowCustomDagNodeRegistration[],
  baseRegistry: AsterFlowDagNodeRegistry = asterFlowDagNodeRegistry,
): AsterFlowDagNodeCatalog {
  const customTypes = new Set<string>();
  const normalized: AsterFlowCustomDagNodeRegistration[] = [];
  for (const registration of registrations) {
    const admitted = defineAsterFlowCustomDagNode(registration);
    const type = admitted.manifest.type;
    if (customTypes.has(type)) {
      throw new TypeError(`Duplicate custom Aster Flow DAG node type: ${type}`);
    }
    if (baseRegistry.get(type)) {
      throw new TypeError(
        `Custom Aster Flow DAG node type ${type} is already registered.`,
      );
    }
    customTypes.add(type);
    normalized.push(admitted);
  }

  const orderedBindings = normalized.map(({ capability }) => capability);
  const bindings = new Map(
    orderedBindings.map((binding) => [binding.nodeType, binding]),
  );
  const capabilities: AsterFlowDagNodeCapabilityRegistry = Object.freeze({
    get: (type: string) => bindings.get(type),
    require: (type: string) => {
      const binding = bindings.get(type);
      if (!binding)
        throw new Error(`Unknown Aster Flow DAG node capability: ${type}`);
      return binding;
    },
    list: () => [...orderedBindings],
  });
  const custom = Object.freeze([...normalized]);
  return Object.freeze({
    registry: createAsterFlowDagNodeRegistry([
      ...baseRegistry.list(),
      ...custom.map(({ manifest }) => manifest),
    ]),
    capabilities,
    custom,
  });
}
