import { fireEvent, render, screen } from '@testing-library/react';
import { useState } from 'react';
import { describe, expect, it } from 'vitest';
import {
  filterA3sFlowExpressionVariables,
  suggestionAnchor,
  VariableReferenceInput,
  VariableTemplateTextarea,
  type A3sFlowExpressionVariable,
} from '../src/react/a3s-flow-variable-picker';

const variables: readonly A3sFlowExpressionVariable[] = [
  {
    dataType: 'object',
    group: 'input',
    label: 'Order input',
    path: 'input.order',
  },
  {
    dataType: 'string',
    group: 'upstream',
    label: 'Customer name',
    nodeId: 'lookup-customer',
    path: 'steps.lookup-customer.customer_name',
  },
];

function ReferenceHarness() {
  const [path, setPath] = useState('');
  return (
    <VariableReferenceInput
      aria-label="Source variable"
      locale="en"
      onPathChange={setPath}
      path={path}
      variables={variables}
    />
  );
}

function TemplateHarness() {
  const [value, setValue] = useState('Hello ');
  return (
    <VariableTemplateTextarea
      aria-label="Message template"
      locale="en"
      onValueChange={setValue}
      value={value}
      variables={variables}
    />
  );
}

describe('workflow variable suggestions', () => {
  it('anchors a menu to the expression control, including its leading icon', () => {
    const source = document.createElement('div');
    source.className = 'a3s-form-flow-expression-source';
    const field = document.createElement('div');
    field.className = 'a3s-flow-variable-input';
    source.append(field);

    expect(suggestionAnchor(field)).toBe(source);
    expect(suggestionAnchor(source)).toBe(source);
  });

  it('filters by path, label, type, and node identity', () => {
    expect(filterA3sFlowExpressionVariables(variables, '$order')).toEqual([variables[0]]);
    expect(filterA3sFlowExpressionVariables(variables, 'customer string')).toEqual([variables[1]]);
    expect(filterA3sFlowExpressionVariables(variables, 'lookup-customer')).toEqual([variables[1]]);
  });

  it('portals the menu onto the document and keeps a pointer selection', () => {
    render(<ReferenceHarness />);
    const input = screen.getByRole('textbox', { name: 'Source variable' });

    fireEvent.change(input, {
      target: { selectionStart: 1, value: '$' },
    });
    const list = screen.getByRole('listbox', { name: 'Variable suggestions' });
    const option = screen.getByRole('option', { name: /\$input\.order/ });

    expect(list.parentElement).toBe(document.body);
    expect(input.closest('.a3s-flow-variable-input')?.contains(list)).toBe(false);
    expect(option.querySelector('em')?.textContent).toBe('object');
    expect(list.querySelector('header strong')?.textContent).toBe('Reference a variable');
    expect(list.querySelectorAll('header kbd')).toHaveLength(2);

    fireEvent.pointerDown(option);
    expect(screen.getByRole('listbox', { name: 'Variable suggestions' })).toBe(list);
    fireEvent.click(option);
    expect((input as HTMLInputElement).value).toBe('input.order');
  });

  it('opens on $, supports keyboard selection, and stores a clean path', () => {
    render(<ReferenceHarness />);
    const input = screen.getByRole('textbox', { name: 'Source variable' });

    fireEvent.change(input, {
      target: { selectionStart: 4, value: '$ord' },
    });
    expect(screen.getByRole('listbox', { name: 'Variable suggestions' })).toBeTruthy();
    expect(screen.getAllByRole('option')).toHaveLength(1);

    fireEvent.keyDown(input, { key: 'Enter' });
    expect((input as HTMLInputElement).value).toBe('input.order');
    expect(screen.queryByRole('listbox', { name: 'Variable suggestions' })).toBeNull();
  });

  it('keeps Escape local when suggestions were opened from the input', () => {
    render(<ReferenceHarness />);
    const input = screen.getByRole('textbox', { name: 'Source variable' });
    const parentKeyDown = vi.fn();
    window.addEventListener('keydown', parentKeyDown);

    fireEvent.change(input, {
      target: { selectionStart: 4, value: '$ord' },
    });
    expect(screen.getByRole('listbox', { name: 'Variable suggestions' })).toBeTruthy();

    fireEvent.keyDown(input, { key: 'Escape' });
    expect(screen.queryByRole('listbox', { name: 'Variable suggestions' })).toBeNull();
    expect(parentKeyDown).not.toHaveBeenCalled();

    window.removeEventListener('keydown', parentKeyDown);
  });

  it('closes toolbar suggestions before Escape reaches a parent panel', () => {
    render(<TemplateHarness />);
    const trigger = screen.getByRole('button', { name: 'Insert variable' });
    const parentKeyDown = vi.fn();
    window.addEventListener('keydown', parentKeyDown);

    fireEvent.click(trigger);
    expect(screen.getByRole('listbox', { name: 'Variable suggestions' })).toBeTruthy();

    fireEvent.keyDown(trigger, { key: 'Escape' });
    expect(screen.queryByRole('listbox', { name: 'Variable suggestions' })).toBeNull();
    expect(parentKeyDown).not.toHaveBeenCalled();

    window.removeEventListener('keydown', parentKeyDown);
  });

  it('inserts the selected variable into a template', () => {
    render(<TemplateHarness />);
    const textarea = screen.getByRole('textbox', { name: 'Message template' });

    fireEvent.change(textarea, {
      target: { selectionStart: 10, value: 'Hello $cus' },
    });
    expect(screen.getAllByRole('option')).toHaveLength(1);

    fireEvent.keyDown(textarea, { key: 'Enter' });
    expect((textarea as HTMLTextAreaElement).value).toBe(
      'Hello {{steps.lookup-customer.customer_name}}',
    );
  });

  it('inserts from the toolbar at the last remembered caret', async () => {
    render(<TemplateHarness />);
    const textarea = screen.getByRole('textbox', { name: 'Message template' });
    const trigger = screen.getByRole('button', { name: 'Insert variable' });

    textarea.focus();
    (textarea as HTMLTextAreaElement).setSelectionRange(3, 3);
    fireEvent.select(textarea);
    fireEvent.click(trigger);
    fireEvent.click(screen.getByRole('option', { name: /\$input\.order/ }));

    await new Promise((resolve) => requestAnimationFrame(resolve));
    expect((textarea as HTMLTextAreaElement).value).toBe('Hel{{input.order}}lo ');
  });

  it('defaults toolbar insertion to the end when the textarea has not been touched', async () => {
    render(<TemplateHarness />);
    fireEvent.click(screen.getByRole('button', { name: 'Insert variable' }));
    fireEvent.click(screen.getByRole('option', { name: /\$input\.order/ }));

    await new Promise((resolve) => requestAnimationFrame(resolve));
    expect(
      (
        screen.getByRole('textbox', {
          name: 'Message template',
        })
      ).value,
    ).toBe('Hello {{input.order}}');
  });
});
