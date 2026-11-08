import {
  type ComputedRef,
  computed,
  type MaybeRefOrGetter,
  type ShallowRef,
  shallowRef,
  toValue,
} from 'vue';
import type { JsonObject } from '@aster-ai-lab/form/core';
import type { AsterFlowWorkflowDagNode } from '../integrations/aster-flow-dsl-types';
import {
  type AsterFlowDagNodeManifest,
  type AsterFlowDagNodeRegistry,
  asterFlowDagNodeRegistry,
  createAsterFlowDagNode,
  mergeAsterFlowDagNodeConfiguration,
  selectAsterFlowDagNodeConfiguration,
} from '../integrations/aster-flow-node-manifest';

export interface UseAsterFlowNodeOptions {
  id: MaybeRefOrGetter<string>;
  type: MaybeRefOrGetter<string>;
  configuration?: MaybeRefOrGetter<JsonObject | undefined>;
  presentation?: MaybeRefOrGetter<JsonObject | undefined>;
  registry?: MaybeRefOrGetter<AsterFlowDagNodeRegistry | undefined>;
  onChange?: (node: AsterFlowWorkflowDagNode) => void;
}

export interface UseAsterFlowNodeReturn {
  node: Readonly<ShallowRef<AsterFlowWorkflowDagNode>>;
  manifest: ComputedRef<AsterFlowDagNodeManifest>;
  configuration: ComputedRef<JsonObject>;
  setNode: (node: AsterFlowWorkflowDagNode) => void;
  setConfiguration: (configuration: JsonObject) => void;
  patchConfiguration: (patch: JsonObject) => void;
  setTitle: (title: string) => void;
  setDescription: (description: string) => void;
  reset: () => void;
}

/** Reactive Vue state for one typed Aster Flow DAG node. */
export function useAsterFlowNode(options: UseAsterFlowNodeOptions): UseAsterFlowNodeReturn {
  const registry = computed(() =>
    options.registry ? (toValue(options.registry) ?? asterFlowDagNodeRegistry) : asterFlowDagNodeRegistry,
  );
  const manifest = computed(() => registry.value.require(toValue(options.type)));
  const createInitialNode = () =>
    createAsterFlowDagNode(
      toValue(options.id),
      manifest.value,
      options.configuration ? (toValue(options.configuration) ?? {}) : {},
      options.presentation ? (toValue(options.presentation) ?? {}) : {},
    );
  const internalNode = shallowRef<AsterFlowWorkflowDagNode>(createInitialNode());
  const node: Readonly<ShallowRef<AsterFlowWorkflowDagNode>> = internalNode;
  const commit = (next: AsterFlowWorkflowDagNode) => {
    if (next.data.type !== manifest.value.type) {
      throw new TypeError(
        `Aster Flow DAG node type ${next.data.type} does not match manifest ${manifest.value.type}.`,
      );
    }
    internalNode.value = structuredClone(next);
    options.onChange?.(structuredClone(next));
  };
  const configuration = computed(() =>
    selectAsterFlowDagNodeConfiguration(internalNode.value, manifest.value),
  );
  const setConfiguration = (next: JsonObject) => {
    const normalized = selectAsterFlowDagNodeConfiguration(
      createAsterFlowDagNode(internalNode.value.id, manifest.value, next),
      manifest.value,
    );
    commit(mergeAsterFlowDagNodeConfiguration(internalNode.value, manifest.value, normalized));
  };
  const patchPresentation = (patch: JsonObject) =>
    commit({
      ...structuredClone(internalNode.value),
      data: {
        ...structuredClone(internalNode.value.data),
        ...structuredClone(patch),
        type: internalNode.value.data.type,
      },
    });

  return {
    node,
    manifest,
    configuration,
    setNode: commit,
    setConfiguration,
    patchConfiguration: (patch) =>
      setConfiguration({ ...configuration.value, ...structuredClone(patch) }),
    setTitle: (title) => patchPresentation({ title }),
    setDescription: (desc) => patchPresentation({ desc }),
    reset: () => commit(createInitialNode()),
  };
}
