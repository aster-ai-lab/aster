import { useCallback, useMemo, useState } from 'react';
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
  id: string;
  type: string;
  configuration?: JsonObject;
  presentation?: JsonObject;
  registry?: AsterFlowDagNodeRegistry;
  onChange?: (node: AsterFlowWorkflowDagNode) => void;
}

export interface UseAsterFlowNodeReturn {
  node: AsterFlowWorkflowDagNode;
  manifest: AsterFlowDagNodeManifest;
  configuration: JsonObject;
  setNode: (node: AsterFlowWorkflowDagNode) => void;
  setConfiguration: (configuration: JsonObject) => void;
  patchConfiguration: (patch: JsonObject) => void;
  setTitle: (title: string) => void;
  setDescription: (description: string) => void;
  reset: () => void;
}

/** Controlled-ready React state for one typed Aster Flow DAG node. */
export function useAsterFlowNode(options: UseAsterFlowNodeOptions): UseAsterFlowNodeReturn {
  const registry = options.registry ?? asterFlowDagNodeRegistry;
  const manifest = useMemo(() => registry.require(options.type), [options.type, registry]);
  const [node, setInternalNode] = useState<AsterFlowWorkflowDagNode>(() =>
    createAsterFlowDagNode(
      options.id,
      manifest,
      options.configuration ?? {},
      options.presentation ?? {},
    ),
  );
  const commit = useCallback(
    (next: AsterFlowWorkflowDagNode) => {
      setInternalNode(next);
      options.onChange?.(next);
    },
    [options.onChange],
  );
  const configuration = useMemo(
    () => selectAsterFlowDagNodeConfiguration(node, manifest),
    [manifest, node],
  );
  const setConfiguration = useCallback(
    (next: JsonObject) => {
      const normalized = selectAsterFlowDagNodeConfiguration(
        createAsterFlowDagNode(node.id, manifest, next),
        manifest,
      );
      commit(mergeAsterFlowDagNodeConfiguration(node, manifest, normalized));
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
        createAsterFlowDagNode(
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
