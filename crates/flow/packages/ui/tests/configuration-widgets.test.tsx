import { createElement, useState } from 'react';
import { cleanup, fireEvent, render, screen, waitFor, within } from '@testing-library/react';
import { resolveFormLocaleCatalog, type JsonObject, type JsonValue } from '@a3s-lab/form/core';
import type { FormWidgetProps } from '@a3s-lab/form/react';
import {
  WORKFLOW_CONFIGURATION_WIDGETS,
  WORKFLOW_SELECT_WIDGET_ALIASES,
  workflowNodeFieldControl,
} from '../src/integrations/workflow-node-form';
import {
  createWorkflowConfigurationWidgetRegistry,
  WorkflowFieldAccessory,
  WorkflowSelectWidget,
} from '../src/react/workflow-configuration-widgets';
import { WorkflowCodeEditor } from '../src/react/workflow-code-editor';
import {
  WorkflowCodeWidget,
  WorkflowFileWidget,
  WorkflowJsonWidget,
  WorkflowMcpControl,
  WorkflowNumberWidget,
  WorkflowPromptWidget,
  WorkflowSecretWidget,
  WorkflowSliderWidget,
} from '../src/react/workflow-configuration-editors';
import { SelectControl } from '../src/react/select-control';
import { FlowExpressionEditor } from '../src/react/a3s-flow-expression-widget';
import { A3sFlowDagNodeConfigurationPanel } from '../src/react/a3s-flow-dag-node';
import { A3sFlowSchemaWidget } from '../src/react/a3s-flow-schema-widget';
import { A3sFlowBatchWidget } from '../src/react/a3s-flow-batch-widget';
import {
  A3sFlowChildrenWidget,
  A3sFlowWorkflowSpecWidget,
} from '../src/react/a3s-flow-child-widgets';
import {
  a3sFlowDagNodeRegistry,
  createA3sFlowDagNode,
} from '../src/integrations/a3s-flow-node-manifest';

const conditionField = {
  id: 'condition-input',
  kind: 'field' as const,
  label: '参与判断的值',
  customProps: { inputTypes: ['FlowValue'] },
};

function widgetProps(overrides: Partial<FormWidgetProps> = {}): FormWidgetProps {
  return {
    id: 'workflow-control',
    node: conditionField,
    value: '',
    disabled: false,
    invalid: false,
    options: [],
    dataSource: {} as FormWidgetProps['dataSource'],
    messages: resolveFormLocaleCatalog('zh-CN').messages,
    locale: 'zh-CN',
    onChange: vi.fn(),
    ...overrides,
  };
}

describe('workflow configuration widgets', () => {
  it('opens a select when clicked before the runtime initialization settles', async () => {
    const onChange = vi.fn();
    const view = render(
      <SelectControl aria-label="Immediate mode" onChange={onChange} value="durable">
        <option value="durable">Durable</option>
        <option value="local">Local</option>
      </SelectControl>,
    );
    const root = view.container.querySelector<HTMLElement>('.a3s-flow-select-control');
    const trigger = screen.getByRole('combobox', { name: 'Immediate mode' });

    fireEvent.click(trigger);
    expect(trigger.getAttribute('aria-expanded')).toBe('true');
    await waitFor(() => expect(root?.getAttribute('data-select-initialized')).toBe('true'));
    expect(trigger.getAttribute('aria-expanded')).toBe('true');

    fireEvent.click(screen.getByRole('option', { name: 'Local' }));
    expect(onChange).toHaveBeenLastCalledWith(expect.objectContaining({ target: { value: 'local' } }));
    view.unmount();
  });

  it('keeps every value-source mode and expression envelope in sync', async () => {
    function Harness() {
      const [value, setValue] = useState<JsonObject>({
        apiVersion: 'a3s.dev/flow-expression/v1',
        expression: {
          op: 'concat',
          values: [
            { op: 'literal', value: 'Failed: ' },
            { op: 'field', path: 'input.reason' },
          ],
        },
      });
      return (
        <>
          <FlowExpressionEditor
            id="expression-debug"
            value={value}
            onChange={(next) => setValue(next as JsonObject)}
            locale="en-US"
            purpose="error"
          />
          <output data-testid="expression-op">
            {(value.expression as JsonObject).op as string}
          </output>
        </>
      );
    }

    const view = render(<Harness />);
    const root = view.container.querySelector<HTMLElement>('.a3s-flow-select-control');
    await waitFor(() => expect(root?.getAttribute('data-select-initialized')).toBe('true'));
    const trigger = screen.getByRole('combobox', { name: 'Value source' });

    const choose = async (label: string, mode: string, op: string) => {
      fireEvent.click(trigger);
      fireEvent.click(screen.getByRole('option', { name: label }));
      await waitFor(() => {
        expect(trigger.textContent).toContain(label);
        expect(
          view.container.querySelector('.a3s-form-flow-expression')?.getAttribute('data-mode'),
          ).toBe(mode);
      });
      expect(screen.getByTestId('expression-op').textContent).toBe(op);
    };

    expect(trigger.textContent).toContain('Text template');
    await choose('Workflow field', 'source', 'field');
    await choose('Fixed value', 'value', 'literal');
    await choose('Advanced expression', 'advanced', 'coalesce');
    await choose('Text template', 'template', 'concat');
    view.unmount();
  });

  it('keeps value-source changes working through the DAG configuration panel', async () => {
    const manifest = a3sFlowDagNodeRegistry.require('flow.fail');
    const initial = createA3sFlowDagNode('fail_1', manifest);
    function Harness() {
      const [node, setNode] = useState(initial);
      return (
        <A3sFlowDagNodeConfigurationPanel
          dagNode={node}
          locale="en"
          onChange={setNode}
          showDocumentation={false}
        />
      );
    }
    const view = render(<Harness />);
    const trigger = await screen.findByRole('combobox', { name: 'Value source' });
    expect(trigger.textContent).toContain('Text template');
    fireEvent.click(trigger);
    fireEvent.click(screen.getByRole('option', { name: 'Fixed value' }));
    await waitFor(() => expect(trigger.textContent).toContain('Fixed value'));
    expect(view.container.querySelector('[data-mode="value"]')).not.toBeNull();
    view.unmount();
  });

  it('resets panel-local scroll state when the DAG node instance changes', () => {
    const manifest = a3sFlowDagNodeRegistry.require('flow.fail');
    const first = createA3sFlowDagNode('fail_first', manifest);
    const second = createA3sFlowDagNode('fail_second', manifest);
    const onChange = vi.fn();
    const view = render(
      <A3sFlowDagNodeConfigurationPanel
        dagNode={first}
        locale="en"
        onChange={onChange}
        showDocumentation={false}
      />,
    );
    const firstSettings = view.container.querySelector<HTMLElement>(
      '.a3s-form-workflow-node-settings',
    );
    expect(firstSettings).not.toBeNull();
    if (!firstSettings) return;
    firstSettings.scrollTop = 180;

    view.rerender(
      <A3sFlowDagNodeConfigurationPanel
        dagNode={second}
        locale="en"
        onChange={onChange}
        showDocumentation={false}
      />,
    );

    const secondSettings = view.container.querySelector<HTMLElement>(
      '.a3s-form-workflow-node-settings',
    );
    expect(secondSettings).not.toBe(firstSettings);
    expect(secondSettings?.scrollTop).toBe(0);
    view.unmount();
  });

  it('updates each batch member instead of comparing member objects to indexes', async () => {
    const onChange = vi.fn();
    const initial = [
      {
        step_key: 'first',
        step_name: 'task.first',
        input_mapping: {
          apiVersion: 'a3s.dev/flow-expression/v1',
          expression: { op: 'field', path: 'input.first' },
        },
        max_attempts: 3,
        retry_delay_ms: 0,
        on_exhausted: 'fail_run',
      },
      {
        step_key: 'second',
        step_name: 'task.second',
        input_mapping: {
          apiVersion: 'a3s.dev/flow-expression/v1',
          expression: { op: 'field', path: 'input.second' },
        },
        max_attempts: 4,
        retry_delay_ms: 100,
        on_exhausted: 'fail_run',
      },
    ] satisfies JsonObject[];
    const view = render(
      <A3sFlowBatchWidget
        {...widgetProps({
          id: 'batch-members',
          node: { ...conditionField, label: '批量任务' },
          value: initial,
          valuePath: 'members',
          onChange,
        })}
      />,
    );

    const taskIds = screen.getAllByRole('textbox', { name: /任务 ID/ });
    fireEvent.change(taskIds[0], { target: { value: 'first-renamed' } });
    expect(onChange).toHaveBeenLastCalledWith([
      expect.objectContaining({ step_key: 'first-renamed' }),
      expect.objectContaining({ step_key: 'second' }),
    ]);

    const taskNames = screen.getAllByRole('textbox', { name: /任务名称/ });
    fireEvent.change(taskNames[0], { target: { value: 'task.first.v2' } });
    expect(onChange).toHaveBeenLastCalledWith([
      expect.objectContaining({ step_name: 'task.first.v2' }),
      expect.objectContaining({ step_name: 'task.second' }),
    ]);

    fireEvent.change(screen.getAllByRole('spinbutton', { name: '最多尝试次数' })[0], {
      target: { value: '7' },
    });
    expect(onChange).toHaveBeenLastCalledWith([
      expect.objectContaining({ max_attempts: 7 }),
      expect.objectContaining({ max_attempts: 4 }),
    ]);

    const sourceSelect = screen.getAllByRole('combobox', { name: '取值方式' })[0];
    await waitFor(() =>
      expect(sourceSelect.closest('.a3s-flow-select-control')).not.toBeNull(),
    );
    fireEvent.click(sourceSelect);
    fireEvent.click(screen.getByRole('option', { name: '固定值' }));
    expect(onChange).toHaveBeenLastCalledWith([
      expect.objectContaining({
        input_mapping: expect.objectContaining({
          expression: expect.objectContaining({ op: 'literal' }),
        }),
      }),
      expect.objectContaining({ step_key: 'second' }),
    ]);

    const secondSummary = screen
      .getByText('second', { selector: 'small' })
      .closest('summary');
    expect(secondSummary).not.toBeNull();
    fireEvent.click(secondSummary as HTMLElement);
    await waitFor(() =>
      expect(
        (screen.getByRole('textbox', { name: /任务 ID/ }))
          .value,
      ).toBe('second'),
    );
    const secondTaskId = screen.getByRole('textbox', { name: /任务 ID/ });
    fireEvent.change(secondTaskId, { target: { value: 'second-renamed' } });
    expect(onChange).toHaveBeenLastCalledWith([
      expect.objectContaining({ step_key: 'first' }),
      expect.objectContaining({ step_key: 'second-renamed' }),
    ]);

    const retrySelect = screen.getByRole('combobox', {
      name: '尝试次数用完后',
    });
    fireEvent.click(retrySelect);
    fireEvent.click(
      within(retrySelect.closest('.a3s-flow-select-control') as HTMLElement).getByRole(
        'option',
        { name: '进入失败分支' },
      ),
    );
    expect(onChange).toHaveBeenLastCalledWith([
      expect.objectContaining({ step_key: 'first' }),
      expect.objectContaining({ on_exhausted: 'continue_workflow' }),
    ]);
    view.unmount();
  });

  it('updates every child workflow member, including nested editors and policies', async () => {
    const onChange = vi.fn();
    const initial = [
      {
        child_id: 'child-1',
        spec: {
          name: 'workflow.one',
          version: '1.0.0',
          runtime: {
            kind: 'native_ts',
            entrypoint: 'workflows/one.ts',
            export_name: 'main',
          },
        },
        input: { order: 1 },
        cancellation_policy: 'request_cancellation',
      },
      {
        child_id: 'child-2',
        spec: {
          name: 'workflow.two',
          version: '2.0.0',
          runtime: {
            kind: 'native_ts',
            entrypoint: 'workflows/two.ts',
            export_name: 'main',
          },
        },
        input: { order: 2 },
        cancellation_policy: 'request_cancellation',
      },
    ] satisfies JsonObject[];
    const view = render(
      <A3sFlowChildrenWidget
        {...widgetProps({
          id: 'child-members',
          node: { ...conditionField, label: '子工作流' },
          value: initial,
          valuePath: 'children',
          onChange,
        })}
      />,
    );

    const childIds = screen.getAllByRole('textbox', { name: '子工作流 ID' });
    fireEvent.change(childIds[0], { target: { value: 'child-one-renamed' } });
    expect(onChange).toHaveBeenLastCalledWith([
      expect.objectContaining({ child_id: 'child-one-renamed' }),
      expect.objectContaining({ child_id: 'child-2' }),
    ]);

    const workflowNames = screen.getAllByRole('textbox', { name: '工作流名称' });
    fireEvent.change(workflowNames[0], { target: { value: 'workflow.one.v2' } });
    expect(onChange).toHaveBeenLastCalledWith([
      expect.objectContaining({
        spec: expect.objectContaining({ name: 'workflow.one.v2' }),
      }),
      expect.objectContaining({ child_id: 'child-2' }),
    ]);

    const childInput = screen.getAllByRole('textbox', {
      name: '子工作流输入 JSON',
    })[0] as HTMLTextAreaElement;
    fireEvent.change(childInput, { target: { value: '{"order":42}' } });
    fireEvent.blur(childInput);
    expect(onChange).toHaveBeenLastCalledWith([
      expect.objectContaining({ input: { order: 42 } }),
      expect.objectContaining({ child_id: 'child-2' }),
    ]);

    const policySelect = screen.getAllByRole('combobox', { name: '取消策略' })[0];
    fireEvent.click(policySelect);
    fireEvent.click(screen.getByRole('option', { name: '保留运行' }));
    expect(onChange).toHaveBeenLastCalledWith([
      expect.objectContaining({ cancellation_policy: 'abandon' }),
      expect.objectContaining({ child_id: 'child-2' }),
    ]);

    fireEvent.click(screen.getByText('child-2', { selector: 'strong' }));
    const secondChildIds = screen.getAllByRole('textbox', { name: '子工作流 ID' });
    fireEvent.change(secondChildIds[1], { target: { value: 'child-two-renamed' } });
    expect(onChange).toHaveBeenLastCalledWith([
      expect.objectContaining({ child_id: 'child-1' }),
      expect.objectContaining({ child_id: 'child-two-renamed' }),
    ]);

    const secondPolicy = screen.getAllByRole('combobox', { name: '取消策略' })[1];
    fireEvent.click(secondPolicy);
    fireEvent.click(
      within(secondPolicy.closest('.a3s-flow-select-control') as HTMLElement).getByRole(
        'option',
        { name: '保留运行' },
      ),
    );
    expect(onChange).toHaveBeenLastCalledWith([
      expect.objectContaining({ child_id: 'child-1' }),
      expect.objectContaining({ cancellation_policy: 'abandon' }),
    ]);
    view.unmount();
  });
  it('uses the A3S UI Select runtime for composite controls', async () => {
    const onChange = vi.fn();
    const view = render(
      <SelectControl aria-label="Run mode" onChange={onChange} value="durable">
        <option value="durable">Durable</option>
        <option value="local">Local</option>
      </SelectControl>,
    );
    const root = view.container.querySelector('.a3s-flow-select-control');
    const trigger = screen.getByRole('combobox', { name: 'Run mode' });

    await waitFor(() => expect(root?.getAttribute('data-select-initialized')).toBe('true'));
    expect(root?.getAttribute('data-a3s-components')).toContain('select');
    expect(root?.querySelector('select')).toBeNull();
    fireEvent.click(trigger);
    expect(trigger.getAttribute('aria-expanded')).toBe('true');

    fireEvent.click(screen.getByRole('option', { name: 'Local' }));
    expect(onChange).toHaveBeenCalledWith(expect.objectContaining({ target: { value: 'local' } }));
    expect(trigger.getAttribute('aria-expanded')).toBe('false');
    view.unmount();
  });

  it('keeps pointer selection alive while the trigger remains focused', async () => {
    const onChange = vi.fn();
    const view = render(
      <SelectControl aria-label="Pointer mode" onChange={onChange} value="durable">
        <option value="durable">Durable</option>
        <option value="local">Local</option>
      </SelectControl>,
    );
    const root = view.container.querySelector<HTMLElement>('.a3s-flow-select-control');
    await waitFor(() => expect(root?.getAttribute('data-select-initialized')).toBe('true'));

    const trigger = screen.getByRole('combobox', { name: 'Pointer mode' });
    fireEvent.click(trigger);
    const option = screen.getByRole('option', { name: 'Local' });
    const pointerDown = new Event('pointerdown', { bubbles: true, cancelable: true });
    option.dispatchEvent(pointerDown);

    expect(pointerDown.defaultPrevented).toBe(true);
    fireEvent.pointerUp(option);
    fireEvent.click(option);
    expect(onChange).toHaveBeenLastCalledWith(
      expect.objectContaining({ target: { value: 'local' } }),
    );
    view.unmount();
  });

  it('preserves select semantics for nested labels, optgroups, and metadata', async () => {
    const view = render(
      <SelectControl
        aria-label="Execution mode"
        data-a3s-form-path="settings.mode"
        data-testid="execution-select"
        name="settings.mode"
        onChange={vi.fn()}
        value="safe"
      >
        <option value="">Choose a mode…</option>
        <optgroup disabled label="Unavailable">
          <option value="legacy">Legacy</option>
        </optgroup>
        <optgroup label="Available">
          <option value="safe">
            <>
              Safe <span>mode</span>
            </>
          </option>
          <option label="Fallback label" value="fallback" />
        </optgroup>
      </SelectControl>,
    );
    const root = view.container.querySelector<HTMLElement>("[data-testid='execution-select']");
    await waitFor(() => expect(root?.getAttribute('data-select-initialized')).toBe('true'));

    expect(root?.querySelector('select')).toBeNull();
    expect(root?.getAttribute('data-a3s-form-path')).toBe('settings.mode');
    expect(root?.querySelector("[data-value='safe']")?.textContent).toBe('Safe mode');
    expect(root?.querySelector("[data-value='legacy']")?.getAttribute('aria-disabled')).toBe(
      'true',
    );
    expect(root?.querySelector("[data-value='fallback']")?.textContent).toBe('Fallback label');
    expect((root?.querySelector("input[type='hidden']") as HTMLInputElement)?.name).toBe(
      'settings.mode',
    );
    view.unmount();
  });

  it('supports keyboard navigation and closes when focus leaves the trigger', async () => {
    const onChange = vi.fn();
    const view = render(
      <div>
        <SelectControl aria-label="Mode" onChange={onChange} value="one">
          <option value="one">One</option>
          <option value="two">Two</option>
          <option value="three">Three</option>
        </SelectControl>
        <button type="button">Outside</button>
      </div>,
    );
    const root = view.container.querySelector<HTMLElement>('.a3s-flow-select-control');
    const trigger = screen.getByRole('combobox', { name: 'Mode' });
    await waitFor(() => expect(root?.getAttribute('data-select-initialized')).toBe('true'));

    fireEvent.keyDown(trigger, { key: 'ArrowDown' });
    expect(trigger.getAttribute('aria-expanded')).toBe('true');
    fireEvent.keyDown(trigger, { key: 'ArrowDown' });
    expect(trigger.getAttribute('aria-activedescendant')).toContain('option-2');
    fireEvent.keyDown(trigger, { key: 'End' });
    expect(trigger.getAttribute('aria-activedescendant')).toContain('option-3');
    fireEvent.keyDown(trigger, { key: 'Enter' });
    expect(onChange).toHaveBeenLastCalledWith(
      expect.objectContaining({ target: { value: 'three' } }),
    );
    expect(trigger.getAttribute('aria-expanded')).toBe('false');

    fireEvent.click(trigger);
    expect(trigger.getAttribute('aria-expanded')).toBe('true');
    fireEvent.focusOut(trigger, {
      relatedTarget: screen.getByRole('button', { name: 'Outside' }),
    });
    expect(trigger.getAttribute('aria-expanded')).toBe('false');
    view.unmount();
  });

  it('falls back deterministically for stale and disabled values', async () => {
    const view = render(
      <SelectControl aria-label="Policy" value="removed">
        <option disabled value="blocked">
          Blocked
        </option>
        <option value="safe">Safe</option>
        <option value="fast">Fast</option>
      </SelectControl>,
    );
    const root = view.container.querySelector<HTMLElement>('.a3s-flow-select-control');
    await waitFor(() => expect(root?.getAttribute('data-select-initialized')).toBe('true'));
    expect(screen.getByRole('combobox', { name: 'Policy' }).textContent).toContain('Safe');
    expect((root?.querySelector("input[type='hidden']") as HTMLInputElement)?.value).toBe('safe');
    view.unmount();
  });

  it('keeps placeholder styling separate from a real empty-string option', async () => {
    const placeholderView = render(
      <SelectControl aria-label="Placeholder mode" value="safe">
        <option value="">Choose a mode…</option>
        <option value="safe">Safe</option>
      </SelectControl>,
    );
    const placeholderRoot = placeholderView.container.querySelector<HTMLElement>(
      '.a3s-flow-select-control',
    );
    await waitFor(() =>
      expect(placeholderRoot?.getAttribute('data-select-initialized')).toBe('true'),
    );
    expect(placeholderRoot?.dataset.valueEmpty).toBe('false');
    expect(placeholderRoot?.hasAttribute('data-placeholder')).toBe(true);
    placeholderView.unmount();

    const emptyValueView = render(
      <SelectControl aria-label="Clear mode" value="">
        <option value="safe">Safe</option>
        <option value="">Clear selection</option>
      </SelectControl>,
    );
    const emptyValueRoot = emptyValueView.container.querySelector<HTMLElement>(
      '.a3s-flow-select-control',
    );
    await waitFor(() =>
      expect(emptyValueRoot?.getAttribute('data-select-initialized')).toBe('true'),
    );
    expect(emptyValueRoot?.hasAttribute('data-placeholder')).toBe(false);
    expect(
      within(emptyValueView.container).getByRole('combobox', {
        name: 'Clear mode',
      }).textContent,
    ).toContain('Clear selection');
    expect(emptyValueRoot?.querySelector("[data-value='']")?.getAttribute('aria-selected')).toBe(
      'true',
    );
    emptyValueView.unmount();
  });

  it('keeps form labels and hidden names stable when the trigger shares the field id', async () => {
    const view = render(
      <label htmlFor="field-mode">
        Mode
        <SelectControl aria-label="Mode" id="field-mode" triggerId="field-mode" value="safe">
          <option value="safe">Safe</option>
          <option value="fast">Fast</option>
        </SelectControl>
      </label>,
    );
    const root = view.container.querySelector<HTMLElement>('.a3s-flow-select-control');
    const trigger = screen.getByRole('combobox', { name: 'Mode' });
    await waitFor(() => expect(root?.getAttribute('data-select-initialized')).toBe('true'));
    expect(root?.id).toBe('field-mode-root');
    expect(trigger.id).toBe('field-mode');
    expect(view.container.querySelectorAll('#field-mode')).toHaveLength(1);
    expect((root?.querySelector("input[type='hidden']") as HTMLInputElement)?.name).toBe(
      'field-mode-root-value',
    );
    trigger.focus();
    expect(document.activeElement).toBe(trigger);
    view.unmount();
  });

  it('exposes disabled state and skips disabled options during keyboard selection', async () => {
    const onChange = vi.fn();
    const view = render(
      <div>
        <SelectControl aria-label="Disabled mode" disabled value="safe">
          <option value="safe">Safe</option>
        </SelectControl>
        <SelectControl aria-label="Keyboard mode" onChange={onChange} value="safe">
          <option disabled value="blocked">
            Blocked
          </option>
          <option value="safe">Safe</option>
          <option value="fast">Fast</option>
        </SelectControl>
      </div>,
    );
    const disabledTrigger = screen.getByRole('combobox', {
      name: 'Disabled mode',
    });
    expect((disabledTrigger as HTMLButtonElement).disabled).toBe(true);
    expect(disabledTrigger.getAttribute('aria-disabled')).toBe('true');

    const keyboardRoot = view.container.querySelectorAll<HTMLElement>(
      '.a3s-flow-select-control',
    )[1];
    const keyboardTrigger = screen.getByRole('combobox', {
      name: 'Keyboard mode',
    });
    await waitFor(() => expect(keyboardRoot?.getAttribute('data-select-initialized')).toBe('true'));
    fireEvent.keyDown(keyboardTrigger, { key: 'ArrowDown' });
    fireEvent.keyDown(keyboardTrigger, { key: 'Home' });
    expect(keyboardTrigger.getAttribute('aria-activedescendant')).toContain('option-2');
    fireEvent.keyDown(keyboardTrigger, { key: 'ArrowUp' });
    expect(keyboardTrigger.getAttribute('aria-activedescendant')).toContain('option-2');
    fireEvent.keyDown(keyboardTrigger, { key: 'Escape' });
    expect(keyboardTrigger.getAttribute('aria-expanded')).toBe('false');
    expect(onChange).not.toHaveBeenCalled();
    view.unmount();
  });

  it('derives an ordinary Flow select from a schema enum when static options are omitted', async () => {
    const onChange = vi.fn();
    const registry = createWorkflowConfigurationWidgetRegistry();
    const view = render(
      createElement(
        registry[WORKFLOW_CONFIGURATION_WIDGETS.parameter],
        widgetProps({
          node: {
            ...conditionField,
            label: 'Schema mode',
            customProps: {},
          },
          schema: { type: 'string', enum: ['safe', 'fast'] },
          options: [],
          value: 'safe',
          onChange,
        }),
      ),
    );
    const root = view.container.querySelector<HTMLElement>('.a3s-flow-select-control');
    await waitFor(() => expect(root?.getAttribute('data-select-initialized')).toBe('true'));
    expect(
      within(view.container).getByRole('combobox', { name: 'Schema mode' }).textContent,
    ).toContain('safe');
    expect(view.container.querySelector('select')).toBeNull();
    const trigger = within(view.container).getByRole('combobox', {
      name: 'Schema mode',
    });
    fireEvent.click(trigger);
    fireEvent.click(within(view.container).getByRole('option', { name: 'fast' }));
    expect(onChange).toHaveBeenLastCalledWith('fast');
    view.unmount();
  });

  it('does not steal a legitimate empty enum value for the placeholder', async () => {
    const onChange = vi.fn();
    const registry = createWorkflowConfigurationWidgetRegistry();
    const view = render(
      createElement(
        registry[WORKFLOW_CONFIGURATION_WIDGETS.parameter],
        widgetProps({
          node: {
            ...conditionField,
            label: 'Empty enum',
            placeholder: 'Choose a value…',
            customProps: {},
          },
          schema: { type: 'string', enum: ['', 'named'] },
          options: [
            { label: 'Empty value', value: '' },
            { label: 'Named value', value: 'named' },
          ],
          value: '',
          onChange,
        }),
      ),
    );
    const root = view.container.querySelector<HTMLElement>('.a3s-flow-select-control');
    await waitFor(() => expect(root?.getAttribute('data-select-initialized')).toBe('true'));
    expect(root?.hasAttribute('data-placeholder')).toBe(false);
    expect(
      within(view.container).getByRole('combobox', { name: 'Empty enum' }).textContent,
    ).toContain('Empty value');
    fireEvent.click(within(view.container).getByRole('combobox', { name: 'Empty enum' }));
    fireEvent.click(within(view.container).getByRole('option', { name: 'Named value' }));
    expect(onChange).toHaveBeenLastCalledWith('named');
    view.unmount();
  });

  it('normalizes select aliases at both manifest and registry boundaries', async () => {
    expect(
      workflowNodeFieldControl({
        name: 'mode',
        widget: ' nativeSelect ',
        options: ['safe', 'fast'],
      }),
    ).toBe('select');

    const registry = createWorkflowConfigurationWidgetRegistry();
    for (const alias of WORKFLOW_SELECT_WIDGET_ALIASES) {
      expect(registry[alias]).toBe(registry.select);
    }

    const view = render(
      createElement(
        registry[WORKFLOW_CONFIGURATION_WIDGETS.parameter],
        widgetProps({
          node: {
            ...conditionField,
            customProps: { control_widget: ' native-select ' },
          },
          options: [
            { label: 'Safe', value: 'safe' },
            { label: 'Fast', value: 'fast' },
          ],
          value: 'safe',
        }),
      ),
    );
    await waitFor(() =>
      expect(view.container.querySelector('.a3s-flow-select-control')).not.toBeNull(),
    );
    expect(view.container.querySelector('select')).toBeNull();
    view.unmount();
  });

  it('keeps the select popover closed across a controlled value update', async () => {
    function StatefulSelect() {
      const [value, setValue] = useState('durable');
      return (
        <SelectControl
          aria-label="Run mode"
          onChange={(event) => setValue(event.target.value)}
          value={value}
        >
          <option value="durable">Durable</option>
          <option value="local">Local</option>
        </SelectControl>
      );
    }

    const view = render(<StatefulSelect />);
    const root = view.container.querySelector('.a3s-flow-select-control');
    await waitFor(() => expect(root?.getAttribute('data-select-initialized')).toBe('true'));
    const trigger = screen.getByRole('combobox', { name: 'Run mode' });
    fireEvent.click(trigger);
    fireEvent.click(screen.getByRole('option', { name: 'Local' }));

    await waitFor(() => {
      expect(trigger.getAttribute('aria-expanded')).toBe('false');
      expect(trigger.textContent).toContain('Local');
    });
    view.unmount();
  });

  it('uses the A3S UI Select runtime for ordinary enum fields', async () => {
    const onChange = vi.fn();
    const registry = createWorkflowConfigurationWidgetRegistry();
    const view = render(
      createElement(
        registry.select,
        widgetProps({
          node: {
            ...conditionField,
            label: '评分策略',
            placeholder: '请选择',
            customProps: { controlWidget: 'select' },
          },
          options: [
            { label: '均衡策略 v2', value: 'balanced-v2' },
            { label: '高风险拦截', value: 'strict-v1' },
          ],
          value: 'strict-v1',
          onChange,
        }),
      ),
    );

    expect(view.container.querySelector('select')).toBeNull();
    const root = view.container.querySelector('.a3s-flow-select-control');
    expect(root).not.toBeNull();
    await waitFor(() => expect(root?.getAttribute('data-select-initialized')).toBe('true'));

    const trigger = within(view.container).getByRole('combobox', {
      name: '评分策略',
    });
    fireEvent.click(trigger);
    fireEvent.click(within(view.container).getByRole('option', { name: '均衡策略 v2' }));
    expect(onChange).toHaveBeenLastCalledWith('balanced-v2');
    view.unmount();
  });

  it('localizes canvas connection actions for the Chinese panel', () => {
    render(
      <WorkflowFieldAccessory
        node={conditionField}
        value={{}}
        disabled={false}
        locale="zh-CN"
        callbacks={{ onRequestConnection: vi.fn() }}
      />,
    );

    expect(screen.getByText('工作流输入')).toBeTruthy();
    expect(screen.getByRole('button', { name: '连接参与判断的值' }).textContent).toContain('连接');
  });

  it('localizes editor disclosure controls for the Chinese panel', () => {
    const registry = createWorkflowConfigurationWidgetRegistry();
    const Widget = registry[WORKFLOW_CONFIGURATION_WIDGETS.json];
    const props: FormWidgetProps = {
      id: 'condition-input-editor',
      node: conditionField,
      value: {},
      disabled: false,
      invalid: false,
      options: [],
      dataSource: {} as FormWidgetProps['dataSource'],
      messages: {} as FormWidgetProps['messages'],
      locale: 'zh-CN',
      onChange: vi.fn(),
    };

    render(createElement(Widget, props));

    expect(screen.getByRole('button', { name: '展开参与判断的值编辑器' })).toBeTruthy();
  });

  it('initializes the shared code editor with Chinese runtime labels', async () => {
    const view = render(
      <WorkflowCodeEditor
        ariaLabel="运行结果"
        fileName="result.json"
        id="localized-code-editor"
        language="json"
        locale="zh-CN"
        onChange={vi.fn()}
        status="JSON"
        value={'{\n  "ok": true\n}'}
      />,
    );
    const editor = view.container.querySelector<HTMLElement>('.code-editor');

    await waitFor(() => expect(editor?.dataset.codeEditorInitialized).toBe('true'));
    expect(editor?.getAttribute('role')).toBe('group');
    expect(editor?.getAttribute('data-a3s-components')).toContain('code-editor');
    expect(editor?.dataset.indentSize).toBe('2');
    expect(editor?.dataset.validation).toBe('json');
    expect(editor?.querySelector('textarea')?.id).toBe('localized-code-editor');
    expect(editor?.id).not.toBe('localized-code-editor');
    expect(editor?.dataset.labelSaved).toBe('已保存');
    expect(editor?.dataset.labelReadonly).toBe('只读');
    expect(editor?.querySelector('[data-code-editor-state]')?.textContent).toBe('已保存');
    expect(editor?.querySelector('[data-code-editor-lines]')?.textContent).toBe('3 行');
    expect(editor?.querySelector('[data-code-editor-characters]')?.textContent).toBe('14 个字符');
  });

  it('keeps every source line available to the shared editor viewport', async () => {
    const value = Array.from(
      { length: 17 },
      (_, index) => `line ${index + 1}`,
    ).join('\n');
    const view = render(
      <WorkflowCodeEditor
        ariaLabel="多行表达式"
        fileName="expression.json"
        id="multiline-code-editor"
        language="json"
        locale="zh-CN"
        onChange={vi.fn()}
        value={value}
      />,
    );
    const editor = view.container.querySelector<HTMLElement>('.code-editor');

    await waitFor(() =>
      expect(editor?.dataset.codeEditorInitialized).toBe('true'),
    );
    expect(editor?.querySelector('textarea')?.value).toBe(value);
    expect(
      editor?.querySelectorAll('[data-code-editor-gutter] > span'),
    ).toHaveLength(17);
    expect(
      editor?.querySelector('[data-code-editor-lines]')?.textContent,
    ).toBe('17 行');
  });

  it('keeps invalid schema drafts local and preserves valid editor formatting', async () => {
    const initialSchema = {
      type: 'object',
      properties: { name: { type: 'string' } },
      required: ['name'],
    };

    function Harness() {
      const [value, setValue] = useState<JsonValue>(initialSchema);
      const [resetCount, setResetCount] = useState(0);
      return (
        <>
          <A3sFlowSchemaWidget
            {...widgetProps({
              id: 'schema-editor',
              node: { ...conditionField, label: 'Input schema' },
              value,
              locale: 'en-US',
              onChange: (next) => setValue(next),
            })}
          />
          <output data-testid="schema-value">{JSON.stringify(value)}</output>
          <output data-testid="schema-reset-count">{resetCount}</output>
          <button
            type="button"
            onClick={() => {
              setValue({
                type: 'object',
                properties: { orderId: { type: 'string' } },
              });
              setResetCount((count) => count + 1);
            }}
          >
            Reset schema
          </button>
        </>
      );
    }

    const view = render(<Harness />);
    fireEvent.click(screen.getByText('Advanced JSON Schema'));
    const editor = screen.getByRole('textbox', {
      name: 'Input schema JSON',
    }) as HTMLTextAreaElement;
    const invalid = '{"type":"object",';
    fireEvent.change(editor, { target: { value: invalid } });

    expect(editor.value).toBe(invalid);
    expect(screen.getByText('The JSON is invalid. Fix it before saving changes.')).toBeTruthy();
    expect(screen.getByTestId('schema-value').textContent).toBe(JSON.stringify(initialSchema));

    const valid = '{ "type": "object", "properties": { "age": { "type": "number" } } }';
    fireEvent.change(editor, { target: { value: valid } });
    await waitFor(() =>
      expect(screen.getByTestId('schema-value').textContent).toContain('age'),
    );
    // The parent echo must not replace the user's intentional one-line
    // formatting with JSON.stringify(schema, null, 2).
    expect(editor.value).toBe(valid);
    expect(screen.queryByText('The JSON is invalid. Fix it before saving changes.')).toBeNull();

    fireEvent.change(editor, { target: { value: '{"broken":' } });
    expect(editor.value).toBe('{"broken":');
    fireEvent.click(screen.getByRole('button', { name: 'Reset schema' }));
    await waitFor(() =>
      expect(editor.value).toBe(
        JSON.stringify(
          { type: 'object', properties: { orderId: { type: 'string' } } },
          null,
          2,
        ),
      ),
    );
    expect(screen.getByTestId('schema-reset-count').textContent).toBe('1');
    expect(screen.queryByText('The JSON is invalid. Fix it before saving changes.')).toBeNull();
    view.unmount();
  });

  it('keeps invalid schema field names out of the stored value', async () => {
    const onChange = vi.fn();
    const schema = {
      type: 'object',
      properties: {
        email: { type: 'string' },
        phone: { type: 'string' },
      },
      required: ['email'],
    };
    const view = render(
      <A3sFlowSchemaWidget
        {...widgetProps({
          id: 'schema-fields',
          node: { ...conditionField, label: 'Input schema' },
          value: schema,
          locale: 'en-US',
          onChange,
        })}
      />,
    );
    const nameInput = screen.getAllByRole('textbox', { name: 'Field name' })[0] as HTMLInputElement;
    fireEvent.change(nameInput, { target: { value: 'phone' } });
    fireEvent.blur(nameInput);
    expect(onChange).not.toHaveBeenCalled();
    expect(screen.getByText('Field names must be non-empty and unique.')).toBeTruthy();

    fireEvent.change(nameInput, { target: { value: 'customerEmail' } });
    fireEvent.blur(nameInput);
    expect(onChange).toHaveBeenLastCalledWith(
      expect.objectContaining({
        properties: expect.objectContaining({ customerEmail: { type: 'string' } }),
      }),
    );
    expect(onChange.mock.calls.at(-1)?.[0]).not.toEqual(
      expect.arrayContaining([expect.objectContaining({ __a3s_form_invalid_schema_draft__: expect.anything() })]),
    );
    view.unmount();
  });

  it('inserts an externally supplied upstream variable from the prompt keyboard menu', async () => {
    function PromptHarness() {
      const [value, setValue] = useState('Notify ');
      return (
        <WorkflowPromptWidget
          {...widgetProps({
            node: { ...conditionField, label: '通知内容' },
            onChange: (nextValue) => {
              if (typeof nextValue === 'string') setValue(nextValue);
            },
            value,
          })}
          variables={[
            {
              dataType: 'string',
              group: 'upstream',
              label: 'Order ID',
              nodeId: 'load-order',
              path: 'load-order.order_id',
            },
          ]}
        />
      );
    }

    render(<PromptHarness />);
    const textarea = screen.getByRole('textbox', { name: '通知内容' });
    fireEvent.change(textarea, {
      target: {
        selectionEnd: 13,
        selectionStart: 13,
        value: 'Notify $order',
      },
    });

    expect(await screen.findByRole('listbox', { name: '变量智能感知' })).toBeTruthy();
    expect(screen.getByText('$load-order.order_id')).toBeTruthy();
    fireEvent.keyDown(textarea, { key: 'Enter' });

    await waitFor(() =>
      expect((textarea as HTMLTextAreaElement).value).toBe('Notify {{load-order.order_id}}'),
    );
    expect(screen.queryByRole('listbox')).toBeNull();
  });

  it('localizes composite controls without changing their stored values', () => {
    const registry = createWorkflowConfigurationWidgetRegistry();
    cleanup();

    const sortable = render(
      createElement(
        registry[WORKFLOW_CONFIGURATION_WIDGETS.sortableList],
        widgetProps({
          node: {
            ...conditionField,
            label: '收件人来源',
            customProps: {
              sourceOptions: ['input.customer.email', 'input.customer.phone'],
            },
          },
          value: ['input.customer.email'],
        }),
      ),
    );
    expect(
      within(sortable.container).getByRole('combobox', {
        name: '添加收件人来源',
      }).textContent,
    ).toContain('添加一项…');
    expect(
      within(sortable.container).getByRole('button', {
        name: '移除input.customer.email',
      }),
    ).toBeTruthy();
    sortable.unmount();

    const duration = render(
      createElement(
        registry[WORKFLOW_CONFIGURATION_WIDGETS.duration],
        widgetProps({
          node: {
            ...conditionField,
            label: '保留时长',
            customProps: { sourceOptions: ['Minutes', 'Hours'] },
          },
          value: { value: 30, unit: 'Minutes' },
        }),
      ),
    );
    expect(
      (
        within(duration.container).getByRole('spinbutton', {
          name: '保留时长数值',
        })
      ).valueAsNumber,
    ).toBe(30);
    expect(
      within(duration.container).getByRole('combobox', {
        name: '保留时长单位',
      }).textContent,
    ).toContain('分钟');
    expect(
      (duration.container.querySelector('input[type="hidden"]') as HTMLInputElement).value,
    ).toBe('Minutes');
    duration.unmount();

    const slider = render(
      createElement(
        registry[WORKFLOW_CONFIGURATION_WIDGETS.parameter],
        widgetProps({
          node: {
            ...conditionField,
            customProps: { controlWidget: 'slider' },
          },
          schema: { type: 'number', minimum: 0, maximum: 1, multipleOf: 0.05 },
          value: 0.7,
        }),
      ),
    );
    expect(within(slider.container).getByText('最小值 0')).toBeTruthy();
    expect(within(slider.container).getByText('最大值 1')).toBeTruthy();
    expect(within(slider.container).getByText('步长 0.05')).toBeTruthy();
    slider.unmount();
  });

  it('localizes editor empty states in the default Chinese locale', () => {
    const registry = createWorkflowConfigurationWidgetRegistry();
    cleanup();

    const file = render(
      createElement(
        registry[WORKFLOW_CONFIGURATION_WIDGETS.file],
        widgetProps({ value: '', schema: { type: 'string' } }),
      ),
    );
    expect(within(file.container).getByText('选择文件')).toBeTruthy();
    expect(within(file.container).getByText('尚未选择文件')).toBeTruthy();
    file.unmount();

    const mcp = render(
      createElement(registry[WORKFLOW_CONFIGURATION_WIDGETS.mcp], widgetProps({ value: null })),
    );
    expect(within(mcp.container).getByText('MCP 服务')).toBeTruthy();
    expect(within(mcp.container).getByText('尚未配置')).toBeTruthy();
    mcp.unmount();

    const data = render(
      createElement(
        registry[WORKFLOW_CONFIGURATION_WIDGETS.dataDisplay],
        widgetProps({ value: undefined }),
      ),
    );
    expect((within(data.container).getByRole('textbox')).value).toBe(
      '暂无数据。',
    );
    data.unmount();
  });

  it('validates file extensions, preserves valid selections, and exposes removal controls', () => {
    const onChange = vi.fn();
    const node = {
      ...conditionField,
      label: '申报材料',
      customProps: { fileTypes: ['PDF', '.png'] },
    };
    const view = render(
      <WorkflowFileWidget
        {...widgetProps({
          node,
          schema: { type: 'array' },
          value: ['existing.pdf'],
          onChange,
        })}
      />,
    );
    const input = view.container.querySelector('input[type="file"]') as HTMLInputElement;
    expect(input.accept).toBe('.pdf,.png');

    const invalid = new File(['toml'], 'Cargo.toml', { type: 'text/plain' });
    fireEvent.change(input, { target: { files: [invalid] } });
    expect(onChange).not.toHaveBeenCalled();
    expect(view.container.querySelector('[role="alert"]')?.textContent).toContain('Cargo.toml');

    const valid = new File(['pdf'], 'new.PDF', { type: 'application/pdf' });
    fireEvent.change(input, { target: { files: [valid] } });
    expect(onChange).toHaveBeenLastCalledWith(['new.PDF']);

    view.rerender(
      <WorkflowFileWidget
        {...widgetProps({
          node,
          schema: { type: 'array' },
          value: ['new.PDF'],
          onChange,
        })}
      />,
    );
    fireEvent.click(view.getByRole('button', { name: '移除文件new.PDF' }));
    expect(onChange).toHaveBeenLastCalledWith([]);
    view.unmount();
  });

  it('normalizes slider values before exposing or storing them', () => {
    const onChange = vi.fn();
    const view = render(
      <WorkflowSliderWidget
        {...widgetProps({
          node: { ...conditionField, label: '人工复核阈值', customProps: {} },
          schema: { type: 'number', minimum: 0, maximum: 1, multipleOf: 0.01 },
          value: 0.7799999713897705,
          onChange,
        })}
      />,
    );
    const slider = view.getByRole('slider', {
      name: '人工复核阈值',
    }) as HTMLInputElement;
    expect(slider.value).toBe('0.78');
    expect(slider.getAttribute('aria-valuetext')).toContain('0.78');
    fireEvent.change(slider, { target: { value: '0.7799999713897705' } });
    expect(onChange).toHaveBeenLastCalledWith(0.78);
    view.unmount();
  });

  it('reports action-picker duplicates and enforces a declared limit', () => {
    const onChange = vi.fn();
    const registry = createWorkflowConfigurationWidgetRegistry();
    const Widget = registry[WORKFLOW_CONFIGURATION_WIDGETS.actionPicker];
    const node = {
      ...conditionField,
      label: '审核结论',
      customProps: { maxItems: 2 },
    };
    const view = render(createElement(Widget, widgetProps({ node, value: ['clear'], onChange })));
    const input = view.container.querySelector('input') as HTMLInputElement;
    fireEvent.change(input, { target: { value: 'clear' } });
    fireEvent.keyDown(input, { key: 'Enter' });
    expect(
      view.getAllByRole('status').some((status) => status.textContent?.includes('该决策已添加')),
    ).toBe(true);

    fireEvent.change(input, { target: { value: 'manual_review' } });
    fireEvent.click(view.getByRole('button', { name: '添加' }));
    expect(onChange).toHaveBeenLastCalledWith(['clear', 'manual_review']);

    view.rerender(
      createElement(Widget, widgetProps({ node, value: ['clear', 'manual_review'], onChange })),
    );
    expect((view.container.querySelector('input') as HTMLInputElement).disabled).toBe(true);
    expect(view.container.querySelector('[role="status"]')?.textContent).toContain('2/2');
    view.unmount();
  });

  it('keeps sortable lists stable with duplicate values and empty choices', () => {
    const registry = createWorkflowConfigurationWidgetRegistry();
    const Widget = registry[WORKFLOW_CONFIGURATION_WIDGETS.sortableList];
    const view = render(
      createElement(
        Widget,
        widgetProps({
          node: {
            ...conditionField,
            label: '收件人来源',
            customProps: { sourceOptions: [] },
          },
          value: [],
        }),
      ),
    );
    expect(view.container.querySelectorAll('.a3s-form-workflow-empty-control')).toHaveLength(2);
    view.rerender(
      createElement(
        Widget,
        widgetProps({
          node: {
            ...conditionField,
            label: '收件人来源',
            customProps: {
              sourceOptions: [{ name: 'email' }, { name: 'email' }],
            },
          },
          value: [{ name: 'email' }, { name: 'email' }],
        }),
      ),
    );
    expect(view.container.querySelectorAll('ol > li')).toHaveLength(2);
    expect(view.container.querySelectorAll('ol > li button')).toHaveLength(6);
    view.unmount();
  });

  it('exposes duration constraints and treats an empty MCP object as unconfigured', () => {
    const registry = createWorkflowConfigurationWidgetRegistry();
    const duration = render(
      createElement(
        registry[WORKFLOW_CONFIGURATION_WIDGETS.duration],
        widgetProps({
          node: {
            ...conditionField,
            label: '保留时长',
            customProps: { sourceOptions: ['Minutes', 'Hours'] },
          },
          schema: {
            type: 'object',
            properties: {
              value: { type: 'number', minimum: 5, maximum: 30, multipleOf: 5 },
            },
          },
          value: { value: 10, unit: 'Minutes' },
        }),
      ),
    );
    const amount = duration.getByRole('spinbutton', {
      name: '保留时长数值',
    }) as HTMLInputElement;
    expect(amount.min).toBe('5');
    expect(amount.max).toBe('30');
    expect(amount.step).toBe('5');
    duration.unmount();

    const mcp = render(
      createElement(registry[WORKFLOW_CONFIGURATION_WIDGETS.mcp], widgetProps({ value: {} })),
    );
    expect(mcp.getByText('尚未配置')).toBeTruthy();
    mcp.unmount();
  });

  it('can unmount code editors before their asynchronous runtime settles', async () => {
    const consoleError = vi.spyOn(console, 'error').mockImplementation(() => {});
    const unhandled = vi.fn();
    window.addEventListener('unhandledrejection', unhandled);

    const view = render(
      <WorkflowCodeEditor
        ariaLabel="临时编辑器"
        fileName="temporary.json"
        id="temporary-code-editor"
        language="json"
        locale="zh-CN"
        onChange={vi.fn()}
        value="{}"
      />,
    );
    view.unmount();
    await new Promise((resolve) => setTimeout(resolve, 0));

    expect(unhandled).not.toHaveBeenCalled();
    expect(consoleError).not.toHaveBeenCalled();
    window.removeEventListener('unhandledrejection', unhandled);
    consoleError.mockRestore();
  });

  it('renders each planned inspector control state', async () => {
    const registry = createWorkflowConfigurationWidgetRegistry();
    const label = (zh: string, en: string) => `${zh} / ${en}`;

    const expression = render(
      <FlowExpressionEditor
        id="run-id-expression"
        locale="zh-CN"
        value={{ op: 'not-an-op' }}
        onChange={vi.fn()}
      />,
    );
    expect(expression.getAllByRole('alert').map((item) => item.textContent).join(' ')).toContain(
      '表达式无法识别',
    );
    expression.unmount();

    const focused = render(
      <FlowExpressionEditor
        id="focused-expression"
        locale="zh-CN"
        value={{
          apiVersion: 'a3s.dev/flow-expression/v1',
          expression: { op: 'field', path: 'input.configured' },
        }}
        onChange={vi.fn()}
      />,
    );
    const source = focused.container.querySelector(
      '.a3s-form-flow-expression-source input',
    ) as HTMLInputElement;
    source.focus();
    expect(document.activeElement?.closest('.a3s-form-flow-expression-source')).not.toBeNull();
    expect(focused.container.querySelector('.a3s-form-flow-expression')?.getAttribute('data-mode')).toBe(
      'source',
    );
    focused.unmount();

    const json = render(
      <WorkflowJsonWidget
        {...widgetProps({
          node: { ...conditionField, label: label('附加数据', 'Additional metadata') },
          value: {},
        })}
      />,
    );
    expect(json.container.querySelector('textarea')?.value).toContain('{');
    expect(json.queryByRole('alert')).toBeNull();
    json.unmount();
    const badJson = render(
      <WorkflowJsonWidget
        {...widgetProps({
          node: { ...conditionField, label: label('附加数据', 'Metadata') },
          value: 'not-an-object',
        })}
      />,
    );
    expect(badJson.getByRole('alert').textContent).toContain('需要一个对象');
    badJson.unmount();

    const dropdown = render(
      <WorkflowSelectWidget
        {...widgetProps({
          node: { ...conditionField, label: label('尝试次数用完后', 'If all attempts fail') },
          options: [
            { label: 'End run as failed', value: 'fail_run' },
            { label: 'Continue from failure branch', value: 'continue_workflow' },
          ],
          value: 'fail_run',
        })}
      />,
    );
    await waitFor(() =>
      expect(dropdown.container.querySelector('.a3s-flow-select-control')?.getAttribute('data-select-initialized')).toBe(
        'true',
      ),
    );
    expect(dropdown.getByRole('combobox').textContent).toContain('End run as failed');
    dropdown.unmount();
    const badDropdown = render(
      <WorkflowSelectWidget
        {...widgetProps({
          node: { ...conditionField, label: label('评分策略', 'Scoring policy') },
          options: [
            { label: 'Balanced policy v2', value: 'balanced-v2' },
            { label: 'High-risk interception', value: 'strict-v1' },
          ],
          value: '__not_an_option__',
        })}
      />,
    );
    expect(badDropdown.getByRole('alert').textContent).toContain('值不在声明选项内');
    badDropdown.unmount();

    const Tabs = registry[WORKFLOW_CONFIGURATION_WIDGETS.tabs];
    const tabs = render(
      createElement(
        Tabs,
        widgetProps({
          node: { ...conditionField, label: label('运行方式', 'Execution runtime') },
          options: [
            { label: 'Native TypeScript', value: 'native_ts' },
            { label: 'Rust embedded', value: 'rust_embedded' },
          ],
          value: 'native_ts',
        }),
      ),
    );
    const selected = tabs.getByRole('radio', { name: 'Native TypeScript' }) as HTMLInputElement;
    expect(selected.checked).toBe(true);
    selected.focus();
    expect(document.activeElement?.closest('.a3s-form-workflow-segments')).not.toBeNull();
    tabs.unmount();
    const badTabs = render(
      createElement(
        Tabs,
        widgetProps({
          node: { ...conditionField, label: label('等待方式', 'Callback kind') },
          options: [
            { label: 'Human approval', value: 'human_approval' },
            { label: 'Webhook', value: 'webhook' },
          ],
          value: 'nope',
        }),
      ),
    );
    expect(badTabs.getByRole('alert').textContent).toContain('值不在声明选项内');
    badTabs.unmount();

    const slider = render(
      <WorkflowSliderWidget
        {...widgetProps({
          node: { ...conditionField, label: label('人工复核阈值', 'Review threshold') },
          schema: { type: 'number', minimum: 0, maximum: 1, multipleOf: 0.01 },
          value: 1.5,
        })}
      />,
    );
    slider.getByRole('slider', { name: label('人工复核阈值', 'Review threshold') }).focus();
    expect(document.activeElement?.closest('.a3s-form-workflow-slider')).not.toBeNull();
    expect(slider.getByRole('alert').textContent).toContain('取值超出范围');
    slider.unmount();

    const toggle = render(
      createElement(
        registry.switch,
        widgetProps({
          node: { ...conditionField, label: label('严格校验', 'Strict validation') },
          value: true,
        }),
      ),
    );
    const switchControl = toggle.getByRole('switch', { name: label('严格校验', 'Strict validation') });
    expect(switchControl.getAttribute('aria-checked')).toBe('true');
    switchControl.focus();
    expect(document.activeElement).toBe(switchControl);
    toggle.unmount();

    const secret = render(
      <WorkflowSecretWidget
        {...widgetProps({
          node: { ...conditionField, label: label('凭据引用', 'Credential reference') },
          value: '',
        })}
      />,
    );
    const masked = secret.getByLabelText(label('凭据引用', 'Credential reference')) as HTMLInputElement;
    expect(masked.type).toBe('password');
    expect(masked.value).toBe('');
    masked.focus();
    expect(masked.type).toBe('password');
    secret.unmount();

    const preview = render(
      createElement(
        registry[WORKFLOW_CONFIGURATION_WIDGETS.dataDisplay],
        widgetProps({
          node: { ...conditionField, label: label('抽取结果预览', 'Extraction result preview') },
          value: undefined,
        }),
      ),
    );
    const previewBox = preview.getByRole('textbox', {
      name: label('抽取结果预览', 'Extraction result preview'),
    }) as HTMLTextAreaElement;
    expect(previewBox.readOnly).toBe(true);
    expect(previewBox.value).toContain('暂无数据');
    preview.unmount();

    const files = render(
      <WorkflowFileWidget
        {...widgetProps({
          node: { ...conditionField, label: label('报关附件', 'Customs attachments') },
          required: true,
          schema: { type: 'array' },
          value: [],
        })}
      />,
    );
    const picker = files.getByRole('button', { name: '选择文件' });
    picker.focus();
    expect(document.activeElement).toBe(picker);
    expect(files.getByRole('alert').textContent).toContain('请选择文件');
    files.unmount();

    const choices = render(
      createElement(
        registry['multi-select'],
        widgetProps({
          node: { ...conditionField, label: label('必需材料类型', 'Required document types') },
          required: true,
          options: [
            { label: 'Commercial invoice', value: 'commercial_invoice' },
            { label: 'Packing list', value: 'packing_list' },
          ],
          value: [],
        }),
      ),
    );
    const chips = choices.container.querySelector('.a3s-form-workflow-multiselect-chips') as HTMLElement;
    chips.focus();
    expect(document.activeElement).toBe(chips);
    expect(choices.getByRole('alert').textContent).toContain('此项为必填');
    choices.unmount();

    const order = render(
      createElement(
        registry[WORKFLOW_CONFIGURATION_WIDGETS.sortableList],
        widgetProps({
          node: {
            ...conditionField,
            label: label('收件人来源', 'Recipient sources'),
            customProps: {
              sourceOptions: [{ name: 'input.customer.email' }, { name: 'input.customer.phone' }],
            },
          },
          value: [],
        }),
      ),
    );
    expect(order.getByText('尚未选择任何项目。')).toBeTruthy();
    order.unmount();
    function OrderedRecipients() {
      const [value, setValue] = useState([
        { name: 'input.customer.email' },
        { name: 'input.customer.phone' },
      ]);
      return createElement(
        registry[WORKFLOW_CONFIGURATION_WIDGETS.sortableList],
        widgetProps({
          node: {
            ...conditionField,
            label: label('收件人来源', 'Recipient sources'),
            customProps: {
              sourceOptions: [{ name: 'input.customer.email' }, { name: 'input.customer.phone' }],
            },
          },
          value,
          onChange: (next) => setValue(next as typeof value),
        }),
      );
    }
    const ordered = render(<OrderedRecipients />);
    fireEvent.click(ordered.getByRole('button', { name: '下移input.customer.email' }));
    expect(
      [...ordered.container.querySelectorAll('ol li strong')].map((item) => item.textContent),
    ).toEqual(['input.customer.phone', 'input.customer.email']);
    expect(document.activeElement).toBe(
      ordered.container.querySelector('li[data-sort-name="input.customer.email"]'),
    );
    ordered.unmount();

    const decisions = render(
      createElement(
        registry[WORKFLOW_CONFIGURATION_WIDGETS.actionPicker],
        widgetProps({
          node: { ...conditionField, label: label('允许的审阅结论', 'Allowed review decisions') },
          required: true,
          value: [],
        }),
      ),
    );
    const decisionList = decisions.getByRole('list', {
      name: label('允许的审阅结论', 'Allowed review decisions'),
    });
    decisionList.focus();
    expect(document.activeElement).toBe(decisionList);
    expect(decisions.getByRole('alert').textContent).toContain('尚未选择任何结论');
    decisions.unmount();

    const code = render(
      <WorkflowCodeWidget
        {...widgetProps({
          node: { ...conditionField, label: label('预处理代码', 'Preprocessing code') },
          required: true,
          value: '',
        })}
      />,
    );
    expect(code.getByRole('alert').textContent).toContain('请填写代码');
    code.getByRole('textbox', { name: label('预处理代码', 'Preprocessing code') }).focus();
    expect(document.activeElement?.closest('.a3s-form-workflow-source-editor.is-code')).not.toBeNull();
    code.unmount();

    const prompt = render(
      <WorkflowPromptWidget
        {...widgetProps({
          node: { ...conditionField, label: label('抽取指令', 'Extraction instructions') },
          required: true,
          value: '',
        })}
      />,
    );
    expect(prompt.getByRole('alert').textContent).toContain('请填写指令');
    prompt.unmount();

    const mcp = render(
      <WorkflowMcpControl
        {...widgetProps({
          node: { ...conditionField, label: label('海关数据连接器', 'Customs data connector') },
          value: {},
        })}
      />,
    );
    expect(mcp.getByText('尚未配置')).toBeTruthy();
    expect(mcp.getByLabelText('server')).toBeTruthy();
    expect(mcp.getByLabelText('tool')).toBeTruthy();
    mcp.unmount();
    const badMcp = render(
      <WorkflowMcpControl
        {...widgetProps({
          node: { ...conditionField, label: label('海关数据连接器', 'Customs data connector') },
          value: 'not-an-object',
        })}
      />,
    );
    expect(badMcp.getByRole('alert').textContent).toContain('需要一个对象');
    expect(badMcp.getByLabelText('server')).toBeTruthy();
    badMcp.unmount();

    const batch = render(
      <A3sFlowBatchWidget
        {...widgetProps({
          node: { ...conditionField, label: label('任务列表', 'Steps to run') },
          required: true,
          value: [],
        })}
      />,
    );
    expect(batch.getByRole('alert').textContent).toContain('还没有任务');
    batch.unmount();
    const filledBatch = render(
      <A3sFlowBatchWidget
        {...widgetProps({
          node: { ...conditionField, label: label('任务列表', 'Steps to run') },
          value: [
            {
              step_key: 'review',
              step_name: 'task.review',
              input_mapping: {
                apiVersion: 'a3s.dev/flow-expression/v1',
                expression: { op: 'field', path: 'input' },
              },
              max_attempts: 1,
              retry_delay_ms: 0,
              on_exhausted: 'fail_run',
            },
          ],
        })}
      />,
    );
    const batchList = filledBatch.container.querySelector('.a3s-form-flow-batch-list');
    expect(batchList?.textContent).toContain('task.review');
    filledBatch.unmount();

    const children = render(
      <A3sFlowChildrenWidget
        {...widgetProps({
          node: { ...conditionField, label: label('子工作流列表', 'Child workflows') },
          required: true,
          value: [],
        })}
      />,
    );
    expect(children.getByRole('alert').textContent).toContain('还没有子工作流');
    children.unmount();
    const filledChildren = render(
      <A3sFlowChildrenWidget
        {...widgetProps({
          node: { ...conditionField, label: label('子工作流列表', 'Child workflows') },
          value: [{ child_id: 'child.review' }],
        })}
      />,
    );
    const childList = filledChildren.container.querySelector('.a3s-form-flow-children-list');
    expect(childList?.textContent).toContain('child.review');
    filledChildren.unmount();

    const spec = render(
      <A3sFlowWorkflowSpecWidget
        {...widgetProps({
          node: { ...conditionField, label: label('子工作流定义', 'Workflow spec') },
          required: true,
          value: 'not-an-object',
        })}
      />,
    );
    expect(spec.getByRole('alert').textContent).toContain('需要一个对象');
    const name = spec.container.querySelector('.a3s-form-flow-spec input') as HTMLInputElement;
    name.focus();
    expect(document.activeElement?.closest('.a3s-form-flow-spec')).not.toBeNull();
    spec.unmount();

    const handle = render(
      createElement(
        registry[WORKFLOW_CONFIGURATION_WIDGETS.connection],
        widgetProps({
          node: { ...conditionField, label: label('订单上下文', 'Order context') },
          required: true,
          value: 'not-an-object',
        }),
      ),
    );
    expect(handle.getByRole('alert').textContent).toContain('需要一个对象');
    handle.unmount();

    const duration = render(
      createElement(
        registry[WORKFLOW_CONFIGURATION_WIDGETS.duration],
        widgetProps({
          node: {
            ...conditionField,
            label: label('保留时长', 'Reservation window'),
            customProps: { sourceOptions: ['Minutes', 'Hours'] },
          },
          value: { value: 2, unit: 'Centuries' },
        }),
      ),
    );
    expect(duration.container.querySelector('.a3s-form-workflow-duration')).not.toBeNull();
    expect(duration.getByRole('alert').textContent).toContain('单位不在声明列表内');
    duration.unmount();

    const quantity = render(
      <WorkflowNumberWidget
        {...widgetProps({
          node: { ...conditionField, label: label('预留数量', 'Quantity') },
          required: true,
          schema: { type: 'integer', minimum: 1, maximum: 10000 },
          value: 10001,
        })}
      />,
    );
    expect(quantity.getByRole('alert').textContent).toContain('取值超出范围');
    quantity.unmount();
    const missingQuantity = render(
      <WorkflowNumberWidget
        {...widgetProps({
          node: { ...conditionField, label: label('最多尝试次数', 'Maximum attempts') },
          required: true,
          schema: { type: 'integer', minimum: 1, maximum: 100 },
          value: null,
        })}
      />,
    );
    expect(missingQuantity.container.querySelector('.a3s-form-workflow-required-marker')?.textContent).toBe('*');
    expect(missingQuantity.getByRole('alert')).toBeTruthy();
    missingQuantity.unmount();

    const model = render(
      createElement(
        registry[WORKFLOW_CONFIGURATION_WIDGETS.model],
        widgetProps({
          node: { ...conditionField, label: label('材料识别模型', 'Document extraction model') },
          options: [
            { label: 'Document Vision v3', value: 'document-vision-v3' },
            { label: 'Trade Document Extractor v2', value: 'trade-extractor-v2' },
          ],
          value: 'document-vision-v3',
        }),
      ),
    );
    await waitFor(() =>
      expect(model.getByRole('combobox').textContent).toContain('Document Vision v3'),
    );
    model.unmount();
    const badModel = render(
      createElement(
        registry[WORKFLOW_CONFIGURATION_WIDGETS.model],
        widgetProps({
          node: { ...conditionField, label: label('材料识别模型', 'Document extraction model') },
          options: [{ label: 'Document Vision v3', value: 'document-vision-v3' }],
          value: 'missing-model',
        }),
      ),
    );
    expect(badModel.getByRole('alert').textContent).toContain('值不在声明选项内');
    badModel.unmount();
  });
});

describe('schema editor empty state', () => {
  it('makes the empty state itself the add-field action', () => {
    const onChange = vi.fn();
    const view = render(
      <A3sFlowSchemaWidget
        {...widgetProps({
          id: 'schema-empty-action',
          value: {},
          locale: 'en-US',
          onChange,
        })}
      />,
    );
    expect(screen.getByText('No input fields defined')).toBeTruthy();
    const action = screen.getByRole('button', { name: 'Add input field' });
    fireEvent.click(action);
    expect(onChange).toHaveBeenCalledWith(
      expect.objectContaining({
        type: 'object',
        properties: { field_1: { type: 'string', title: '' } },
      }),
    );
    view.unmount();
  });

  it('offers exactly one add affordance while the list is empty', () => {
    const view = render(
      <A3sFlowSchemaWidget
        {...widgetProps({
          id: 'schema-empty-single',
          value: {},
          locale: 'en-US',
        })}
      />,
    );
    expect(
      screen.getAllByRole('button', { name: 'Add input field' }),
    ).toHaveLength(1);
    view.unmount();
  });

  it('moves the add action to the footer once a field exists', () => {
    const view = render(
      <A3sFlowSchemaWidget
        {...widgetProps({
          id: 'schema-filled-footer',
          value: {
            type: 'object',
            properties: { name: { type: 'string' } },
            required: [],
          },
          locale: 'en-US',
        })}
      />,
    );
    expect(screen.queryByText('No input fields defined')).toBeNull();
    expect(
      screen.getAllByRole('button', { name: 'Add input field' }),
    ).toHaveLength(1);
    view.unmount();
  });

  it('disables the empty-state action while the form is disabled', () => {
    const view = render(
      <A3sFlowSchemaWidget
        {...widgetProps({
          id: 'schema-empty-disabled',
          value: {},
          disabled: true,
        })}
      />,
    );
    expect(
      (screen.getByRole('button', { name: '添加输入字段' }))
        .disabled,
    ).toBe(true);
    view.unmount();
  });
});
