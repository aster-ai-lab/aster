import {
  WORKFLOW_CONFIGURATION_WIDGETS,
  WORKFLOW_SELECT_WIDGET_ALIASES,
} from '../integrations/workflow-node-form';
import { A3sFlowBatchWidget } from './a3s-flow-batch-widget';
import {
  A3sFlowChildrenWidget,
  A3sFlowWorkflowSpecWidget,
} from './a3s-flow-child-widgets';
import { A3sFlowExpressionWidget } from './a3s-flow-expression-widget';
import { A3sFlowSchemaWidget } from './a3s-flow-schema-widget';
import { WorkflowSelectWidget } from './workflow-configuration-widgets';
import type { FormWidgetRegistry } from '@a3s-lab/form/react';

export const a3sFlowWidgetRegistry: FormWidgetRegistry = {
  // Keep the standalone Flow registry safe for hosts that render a FormRenderer
  // directly instead of going through WorkflowNodeConfigurationPanel.
  ...Object.fromEntries(
    WORKFLOW_SELECT_WIDGET_ALIASES.map((alias) => [
      alias,
      WorkflowSelectWidget,
    ]),
  ),
  [WORKFLOW_CONFIGURATION_WIDGETS.flowBatch]: A3sFlowBatchWidget,
  [WORKFLOW_CONFIGURATION_WIDGETS.flowChildren]: A3sFlowChildrenWidget,
  [WORKFLOW_CONFIGURATION_WIDGETS.flowExpression]: A3sFlowExpressionWidget,
  [WORKFLOW_CONFIGURATION_WIDGETS.flowSchema]: A3sFlowSchemaWidget,
  [WORKFLOW_CONFIGURATION_WIDGETS.flowSpec]: A3sFlowWorkflowSpecWidget,
};
