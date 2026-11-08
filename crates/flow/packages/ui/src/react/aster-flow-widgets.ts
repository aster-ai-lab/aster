import {
  WORKFLOW_CONFIGURATION_WIDGETS,
  WORKFLOW_SELECT_WIDGET_ALIASES,
} from '../integrations/workflow-node-form';
import { AsterFlowBatchWidget } from './aster-flow-batch-widget';
import {
  AsterFlowChildrenWidget,
  AsterFlowWorkflowSpecWidget,
} from './aster-flow-child-widgets';
import { AsterFlowExpressionWidget } from './aster-flow-expression-widget';
import { AsterFlowSchemaWidget } from './aster-flow-schema-widget';
import { WorkflowSelectWidget } from './workflow-configuration-widgets';
import type { FormWidgetRegistry } from '@aster-ai-lab/form/react';

export const asterFlowWidgetRegistry: FormWidgetRegistry = {
  // Keep the standalone Flow registry safe for hosts that render a FormRenderer
  // directly instead of going through WorkflowNodeConfigurationPanel.
  ...Object.fromEntries(
    WORKFLOW_SELECT_WIDGET_ALIASES.map((alias) => [
      alias,
      WorkflowSelectWidget,
    ]),
  ),
  [WORKFLOW_CONFIGURATION_WIDGETS.flowBatch]: AsterFlowBatchWidget,
  [WORKFLOW_CONFIGURATION_WIDGETS.flowChildren]: AsterFlowChildrenWidget,
  [WORKFLOW_CONFIGURATION_WIDGETS.flowExpression]: AsterFlowExpressionWidget,
  [WORKFLOW_CONFIGURATION_WIDGETS.flowSchema]: AsterFlowSchemaWidget,
  [WORKFLOW_CONFIGURATION_WIDGETS.flowSpec]: AsterFlowWorkflowSpecWidget,
};
