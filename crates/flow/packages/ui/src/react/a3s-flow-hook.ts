import { useCallback, useMemo, useState } from 'react';
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
  id: string;
  type: string;
  configuration?: JsonObject;
  presentation?: JsonObject;
  registry?: A3sFlowDagNodeRegistry;
  onChange?: (node: A3sFlowWorkflowDagNode) => void;
}

export interface UseA3sFlowNodeReturn {
  node: A3sFlowWorkflowDagNode;
  manifest: A3sFlowDagNodeManifest;
  configuration: JsonObject;
  setNode: (node: A3sFlowWorkflowDagNode) => void;
  setConfiguration: (configuration: JsonObject) => void;
  patchConfiguration: (patch: JsonObject) => void;
  setTitle: (title: string) => void;
  setDescription: (description: string) => void;
  reset: () => void;
}

/** Controlled-ready React state for one typed A3S Flow DAG node. */
export function useA3sFlowNode(options: UseA3sFlowNodeOptions): UseA3sFlowNodeReturn {
  const registry = options.registry ?? a3sFlowDagNodeRegistry;
  const manifest = useMemo(() => registry.require(options.type), [options.type, registry]);
  const [node, setInternalNode] = useState<A3sFlowWorkflowDagNode>(() =>
    createA3sFlowDagNode(
      options.id,
      manifest,
      options.configuration ?? {},
      options.presentation ?? {},
    ),
  );
  const commit = useCallback(
    (next: A3sFlowWorkflowDagNode) => {
      setInternalNode(next);
      options.onChange?.(next);
    },
    [options.onChange],
  );
  const configuration = useMemo(
    () => selectA3sFlowDagNodeConfiguration(node, manifest),
    [manifest, node],
  );
  const setConfiguration = useCallback(
    (next: JsonObject) => {
      const normalized = selectA3sFlowDagNodeConfiguration(
        createA3sFlowDagNode(node.id, manifest, next),
        manifest,
      );
      commit(mergeA3sFlowDagNodeConfiguration(node, manifest, normalized));
    },
    [commit, manifest, node],
  );
  const patchConfiguration = useCallback(
    (patch: JsonObject) => setConfiguration({ ...configuration, ...structuredClone(patch) }),
    [configuration, setConfiguration],
  );
  const patchPresentation = useCallback(
    (patch: JsonObject) =>
      commit({
        ...structuredClone(node),
        data: { ...structuredClone(node.data), ...structuredClone(patch), type: node.data.type },
      }),
    [commit, node],
  );
  const reset = useCallback(
    () =>
      commit(
        createA3sFlowDagNode(
          options.id,
          manifest,
          options.configuration ?? {},
          options.presentation ?? {},
        ),
      ),
    [commit, manifest, options.configuration, options.id, options.presentation],
  );
  const setTitle = useCallback(
    (title: string) => patchPresentation({ title }),
    [patchPresentation],
  );
  const setDescription = useCallback(
    (desc: string) => patchPresentation({ desc }),
    [patchPresentation],
  );
  return {
    node,
    manifest,
    configuration,
    setNode: commit,
    setConfiguration,
    patchConfiguration,
    setTitle,
    setDescription,
    reset,
  };
}
