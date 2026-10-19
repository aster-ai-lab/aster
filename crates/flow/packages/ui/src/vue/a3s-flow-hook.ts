import {
  type ComputedRef,
  computed,
  type MaybeRefOrGetter,
  type ShallowRef,
  shallowRef,
  toValue,
} from 'vue';
import type { JsonObject } from '@a3s-lab/form/core';
import type { A3sFlowWorkflowDagNode } from '../integrations/a3s-flow-dsl-types';
import {
  type A3sFlowDagNodeManifest,
  type A3sFlowDagNodeRegistry,
  a3sFlowDagNodeRegistry,
  createA3sFlowDagNode,
  mergeA3sFlowDagNodeConfiguration,
  selectA3sFlowDagNodeConfiguration,
} from '../integrations/a3s-flow-node-manifest';

export interface UseA3sFlowNodeOptions {
  id: MaybeRefOrGetter<string>;
  type: MaybeRefOrGetter<string>;
  configuration?: MaybeRefOrGetter<JsonObject | undefined>;
  presentation?: MaybeRefOrGetter<JsonObject | undefined>;
  registry?: MaybeRefOrGetter<A3sFlowDagNodeRegistry | undefined>;
  onChange?: (node: A3sFlowWorkflowDagNode) => void;
}

export interface UseA3sFlowNodeReturn {
  node: Readonly<ShallowRef<A3sFlowWorkflowDagNode>>;
  manifest: ComputedRef<A3sFlowDagNodeManifest>;
  configuration: ComputedRef<JsonObject>;
  setNode: (node: A3sFlowWorkflowDagNode) => void;
  setConfiguration: (configuration: JsonObject) => void;
  patchConfiguration: (patch: JsonObject) => void;
  setTitle: (title: string) => void;
  setDescription: (description: string) => void;
  reset: () => void;
}

/** Reactive Vue state for one typed A3S Flow DAG node. */
export function useA3sFlowNode(options: UseA3sFlowNodeOptions): UseA3sFlowNodeReturn {
  const registry = computed(() =>
    options.registry ? (toValue(options.registry) ?? a3sFlowDagNodeRegistry) : a3sFlowDagNodeRegistry,
  );
  const manifest = computed(() => registry.value.require(toValue(options.type)));
  const createInitialNode = () =>
    createA3sFlowDagNode(
      toValue(options.id),
      manifest.value,
      options.configuration ? (toValue(options.configuration) ?? {}) : {},
      options.presentation ? (toValue(options.presentation) ?? {}) : {},
    );
  const internalNode = shallowRef<A3sFlowWorkflowDagNode>(createInitialNode());
  const node: Readonly<ShallowRef<A3sFlowWorkflowDagNode>> = internalNode;
  const commit = (next: A3sFlowWorkflowDagNode) => {
    if (next.data.type !== manifest.value.type) {
      throw new TypeError(
        `A3S Flow DAG node type ${next.data.type} does not match manifest ${manifest.value.type}.`,
      );
    }
    internalNode.value = structuredClone(next);
    options.onChange?.(structuredClone(next));
  };
  const configuration = computed(() =>
    selectA3sFlowDagNodeConfiguration(internalNode.value, manifest.value),
  );
  const setConfiguration = (next: JsonObject) => {
    const normalized = selectA3sFlowDagNodeConfiguration(
      createA3sFlowDagNode(internalNode.value.id, manifest.value, next),
      manifest.value,
    );
    commit(mergeA3sFlowDagNodeConfiguration(internalNode.value, manifest.value, normalized));
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
