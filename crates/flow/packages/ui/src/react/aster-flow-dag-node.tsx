import { createElement, useMemo } from 'react';
import type { FormDocument, JsonObject } from '@aster-ai-lab/form/core';
import { createAsterFlowNodeBuildConfig, getAsterFlowCoreNode } from '../integrations/aster-flow-core';
import {
  ASTER_FLOW_V1_COMPATIBILITY,
  type AsterFlowWorkflowDagNode,
} from '../integrations/aster-flow-dsl-types';
import {
  type AsterFlowDagNodeManifest,
  type AsterFlowDagNodeRegistry,
  asterFlowDagNodeRegistry,
  mergeAsterFlowDagNodeConfiguration,
  selectAsterFlowDagNodeConfiguration,
} from '../integrations/aster-flow-node-manifest';
import {
  isWorkflowNodeFieldVisible,
  resolveWorkflowNodeFields,
} from '../integrations/workflow-node-form';
import { isAsterFlowCorePortAvailable } from '../integrations/aster-flow-validation';
import { asterFlowDagNodePreviewSummary } from './aster-flow-node-summary';
import {
  createAsterFlowPanelHostAdapter,
  localizeAsterFlowDagNodeManifest,
} from './aster-flow-panel-support';
import {
  AsterFlowExpressionVariablesProvider,
  type AsterFlowExpressionVariable,
} from './aster-flow-variable-picker';
import { asterFlowWidgetRegistry } from './aster-flow-widgets';
import {
  WorkflowNodeConfigurationPanel,
  type WorkflowNodeConfigurationPanelProps,
} from './workflow-node-panel';
import { WorkflowNodePreview, type WorkflowNodePreviewProps } from './workflow-node-preview';

export interface AsterFlowDagNodeConfigurationPanelProps
  extends Omit<
    WorkflowNodeConfigurationPanelProps,
    'compatibility' | 'node' | 'onApply' | 'onChange' | 'onReset' | 'value'
  > {
  dagNode: AsterFlowWorkflowDagNode;
  manifest?: AsterFlowDagNodeManifest;
  registry?: AsterFlowDagNodeRegistry;
  connectedOutputPortIds?: readonly string[];
  /** Keep the supplied manifest labels and controls. Do not restore help prose. */
  preserveFieldPresentation?: boolean;
  expressionVariables?: readonly AsterFlowExpressionVariable[];
  onChange: (node: AsterFlowWorkflowDagNode) => void;
  onApply?: (node: AsterFlowWorkflowDagNode, document: FormDocument) => void | Promise<void>;
  onReset?: (node: AsterFlowWorkflowDagNode) => void;
}

export interface AsterFlowDagNodePreviewProps
  extends Omit<WorkflowNodePreviewProps, 'node' | 'ports'> {
  dagNode: AsterFlowWorkflowDagNode;
  manifest?: AsterFlowDagNodeManifest;
  registry?: AsterFlowDagNodeRegistry;
}

function missingManifestAlert(
  type: string,
  locale: string | undefined,
  className: string | undefined,
  surfaceClass: string,
) {
  const chinese = locale?.toLocaleLowerCase().startsWith('zh') === true;
  const copy = chinese
    ? {
        title: `找不到节点定义 ${type}`,
        detail: '请先在节点注册表中添加这个类型。当前节点数据没有被修改。',
      }
    : {
        title: `Unregistered node ${type}`,
        detail: 'Register this type in the node registry. The current node data was not changed.',
      };
  return createElement(
    'section',
    {
      className: [surfaceClass, 'aster-form-flow-dag-node-missing', className]
        .filter(Boolean)
        .join(' '),
      role: 'alert',
    },
    createElement('strong', null, copy.title),
    createElement('p', null, copy.detail),
  );
}

/** Typed graph preview for one host-owned Flow v1 DAG node. */
export function AsterFlowDagNodePreview({
  className,
  dagNode,
  manifest: suppliedManifest,
  registry = asterFlowDagNodeRegistry,
  technical = false,
  ...props
}: AsterFlowDagNodePreviewProps) {
  const manifest = suppliedManifest ?? registry.get(dagNode.data.type);
  const coreDefinition = useMemo(
    () => (manifest ? getAsterFlowCoreNode(manifest.type) : undefined),
    [manifest],
  );
  const localizedManifest = useMemo(() => {
    if (!manifest) return undefined;
    const localized = localizeAsterFlowDagNodeManifest(manifest, coreDefinition, props.locale);
    return {
      ...localized,
      display_name:
        typeof dagNode.data.title === 'string' ? dagNode.data.title : localized.display_name,
      description:
        typeof dagNode.data.desc === 'string' ? dagNode.data.desc : localized.description,
    };
  }, [coreDefinition, dagNode.data.desc, dagNode.data.title, manifest, props.locale]);
  const previewPorts = useMemo(() => {
    if (!localizedManifest) return undefined;
    if (!coreDefinition) return localizedManifest.ports;
    return {
      inputs: localizedManifest.ports.inputs,
      outputs: localizedManifest.ports.outputs.filter((port) => {
        const corePort = coreDefinition.ports.outputs.find((candidate) => candidate.id === port.id);
        return !corePort || isAsterFlowCorePortAvailable(corePort, dagNode.data);
      }),
    };
  }, [coreDefinition, dagNode.data, localizedManifest]);
  const previewSummary = useMemo(
    () => asterFlowDagNodePreviewSummary(dagNode, props.locale),
    [dagNode, props.locale],
  );
  if (!localizedManifest) {
    return missingManifestAlert(
      dagNode.data.type,
      props.locale,
      className,
      'aster-form-workflow-node-preview',
    );
  }

  return createElement(WorkflowNodePreview, {
    ...props,
    className: ['aster-form-flow-node-preview', className].filter(Boolean).join(' '),
    node: localizedManifest,
    ports: previewPorts,
    summary: props.summary ?? previewSummary,
    technical,
  });
}

/** Lossless configuration surface for one host-owned Flow v1 DAG node. */
export function AsterFlowDagNodeConfigurationPanel({
  buildConfig,
  className,
  connectedOutputPortIds,
  dagNode,
  expressionVariables,
  fieldVisibility,
  hostAdapter,
  locale,
  manifest: suppliedManifest,
  preserveFieldPresentation = false,
  onApply,
  onChange,
  onReset,
  onRequestConnection,
  registry = asterFlowDagNodeRegistry,
  widgetRegistry,
  ...props
}: AsterFlowDagNodeConfigurationPanelProps) {
  const manifest = suppliedManifest ?? registry.get(dagNode.data.type);
  const coreDefinition = useMemo(
    () => (manifest ? getAsterFlowCoreNode(manifest.type) : undefined),
    [manifest],
  );
  const localizedManifest = useMemo(
    () => {
      if (!manifest) return undefined;
      if (preserveFieldPresentation) {
        return {
          ...manifest,
          display_name:
            typeof dagNode.data.title === 'string' && dagNode.data.title.trim()
              ? dagNode.data.title
              : manifest.display_name,
          description: typeof dagNode.data.desc === 'string' ? dagNode.data.desc : '',
        };
      }
      const localized = localizeAsterFlowDagNodeManifest(manifest, coreDefinition, locale);
      return {
        ...localized,
        display_name:
          typeof dagNode.data.title === 'string' ? dagNode.data.title : localized.display_name,
        description:
          typeof dagNode.data.desc === 'string' ? dagNode.data.desc : localized.description,
      };
    },
    [coreDefinition, dagNode.data.desc, dagNode.data.title, locale, manifest, preserveFieldPresentation],
  );
  const value = useMemo(
    () => (manifest ? selectAsterFlowDagNodeConfiguration(dagNode, manifest) : undefined),
    [dagNode, manifest],
  );
  const resolvedFieldVisibility = useMemo(() => {
    if (!manifest || !value) return fieldVisibility;
    const configuredFields = resolveWorkflowNodeFields(localizedManifest ?? manifest, {
      buildConfig,
      fieldVisibility,
    }, value);
    return Object.fromEntries(
      configuredFields.map((field) => {
        return [
          field.name,
          isWorkflowNodeFieldVisible(field, value, fieldVisibility),
        ];
      }),
    );
  }, [buildConfig, fieldVisibility, localizedManifest, manifest, value]);
  const resolvedBuildConfig = useMemo(
    () =>
      localizedManifest && buildConfig
        ? createAsterFlowNodeBuildConfig(localizedManifest, buildConfig)
        : buildConfig,
    [buildConfig, localizedManifest],
  );
  const resolvedWidgetRegistry = useMemo(
    () => ({
      ...asterFlowWidgetRegistry,
      ...widgetRegistry,
    }),
    [widgetRegistry],
  );
  const validatingHostAdapter = useMemo(
    () =>
      createAsterFlowPanelHostAdapter({
        connectedOutputPortIds,
        definition: coreDefinition,
        hostAdapter,
        locale,
      }),
    [connectedOutputPortIds, coreDefinition, hostAdapter, locale],
  );

  if (!manifest || !localizedManifest || !value) {
    return missingManifestAlert(
      dagNode.data.type,
      locale,
      className,
      'aster-form-workflow-node-panel',
    );
  }

  const nextNode = (configuration: JsonObject) =>
    mergeAsterFlowDagNodeConfiguration(dagNode, manifest, configuration);
  const nextPresentation = (patch: JsonObject): AsterFlowWorkflowDagNode => ({
    ...structuredClone(dagNode),
    data: { ...structuredClone(dagNode.data), ...structuredClone(patch), type: dagNode.data.type },
  });

  return createElement(
    AsterFlowExpressionVariablesProvider,
    { variables: expressionVariables },
    createElement(WorkflowNodeConfigurationPanel, {
      ...props,
      key: dagNode.id,
      buildConfig: resolvedBuildConfig,
      className: ['aster-form-flow-node-panel', className].filter(Boolean).join(' '),
      compatibility: ASTER_FLOW_V1_COMPATIBILITY,
      fieldVisibility: resolvedFieldVisibility,
      hostAdapter: validatingHostAdapter,
      locale,
      node: localizedManifest,
      value,
      onChange: (configuration) => onChange(nextNode(configuration)),
      onApply: async (configuration, document) => onApply?.(nextNode(configuration), document),
      onReset: (configuration) => onReset?.(nextNode(configuration)),
      title: localizedManifest.display_name,
      description: localizedManifest.description,
      onTitleChange: (title) => onChange(nextPresentation({ title })),
      onDescriptionChange: (desc) => onChange(nextPresentation({ desc })),
      onRequestConnection,
      presentation: 'task',
      widgetRegistry: resolvedWidgetRegistry,
    }),
  );
}
