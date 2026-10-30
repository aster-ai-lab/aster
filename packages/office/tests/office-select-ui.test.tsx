import { afterEach, describe, expect, it } from 'bun:test';
import { act, useState, type ReactNode } from 'react';
import { createRoot, type Root } from 'react-dom/client';
import { Dialog } from '../src/internal/design-system/primitives/dialog/dialog';
import { OfficeSelect, type OfficeSelectOption } from '../src/internal/features/work/editors/office-select';
import { OfficeSurface } from '../src/office-surface';

let root: Root | undefined;

afterEach(async () => {
  await act(async () => root?.unmount());
  root = undefined;
  document.body.replaceChildren();
});

async function mount(content: ReactNode) {
  const host = document.createElement('div');
  document.body.append(host);
  root = createRoot(host);
  await act(async () => root!.render(<OfficeSurface>{content}</OfficeSurface>));
  return document.querySelector<HTMLButtonElement>('[data-ui="office.select"] > button')!;
}

const choices: OfficeSelectOption[] = [
  { value: 'alpha', label: 'Alpha', group: 'Fonts', previewStyle: { fontFamily: 'serif' } },
  { value: 'beta', label: 'Beta', disabled: true },
  { value: 'gamma', label: 'Gamma', meta: 'Installed' },
];

function Picker({ options = choices }: { options?: OfficeSelectOption[] }) {
  const [value, setValue] = useState('alpha');
  return <OfficeSelect ariaLabel="Font" options={options} value={value} onValueChange={setValue} />;
}

describe('Office shared picker', () => {
  it('retains typeahead from a closed toolbar picker', async () => {
    const trigger = await mount(<Picker />);
    await act(async () => trigger.dispatchEvent(new KeyboardEvent('keydown', { key: 'g', bubbles: true })));
    expect(document.getElementById(trigger.getAttribute('aria-activedescendant')!)?.getAttribute('aria-label')).toBe('Gamma');
    await act(async () => trigger.dispatchEvent(new KeyboardEvent('keydown', { key: 'Enter', bubbles: true })));
    expect(trigger.getAttribute('data-selected-value')).toBe('gamma');
  });
  it('uses the shared picker and keeps the selected choice active on keyboard opening', async () => {
    const trigger = await mount(<Picker />);
    await act(async () => trigger.focus());
    await act(async () => trigger.dispatchEvent(new KeyboardEvent('keydown', { key: 'ArrowDown', bubbles: true })));
    expect(trigger.parentElement!.classList.contains('ui-search-select')).toBe(true);
    expect(document.activeElement === trigger).toBe(true);
    expect(document.getElementById(trigger.getAttribute('aria-activedescendant')!)?.getAttribute('aria-label')).toBe('Alpha');
    expect(document.querySelector('input[role="combobox"]') === null).toBe(true);
    await act(async () => trigger.dispatchEvent(new KeyboardEvent('keydown', { key: 'ArrowDown', bubbles: true })));
    await act(async () => trigger.dispatchEvent(new KeyboardEvent('keydown', { key: 'Enter', bubbles: true })));
    expect(trigger.getAttribute('data-selected-value')).toBe('gamma');
    expect(trigger.getAttribute('aria-expanded')).toBe('false');
    expect(document.activeElement === trigger).toBe(true);
  });

  it('filters long choices by localized keywords and preserves font previews and metadata', async () => {
    const options = [...choices, ...Array.from({ length: 7 }, (_, index) => ({ value: `extra-${index}`, label: `Extra ${index}` }))];
    options[2] = { ...options[2], searchText: '中文' };
    const trigger = await mount(<Picker options={options} />);
    await act(async () => trigger.click());
    const search = document.querySelector<HTMLInputElement>('input[role="combobox"]')!;
    expect(search !== null).toBe(true);
    expect(document.querySelector('[role="option"] .ui-search-select__option-label')?.getAttribute('style')).toContain('serif');
    expect(document.body.textContent).toContain('Fonts');
    await act(async () => {
      Object.getOwnPropertyDescriptor(window.HTMLInputElement.prototype, 'value')!.set!.call(search, '中文');
      search.dispatchEvent(new Event('input', { bubbles: true }));
    });
    expect(document.querySelectorAll('[role="option"]').length).toBe(1);
    expect(document.querySelector('[role="option"]')?.getAttribute('aria-label')).toBe('Gamma');
    expect(document.body.textContent).toContain('Installed');
    await act(async () => search.dispatchEvent(new KeyboardEvent('keydown', { key: 'Enter', bubbles: true })));
    expect(trigger.getAttribute('data-selected-value')).toBe('gamma');
  });

  it('dismisses a nested picker before its dialog and uses form sizing', async () => {
    let closed = 0;
    const trigger = await mount(<Dialog closeLabel="关闭" title="Format" onClose={() => { closed += 1; }}><Picker /></Dialog>);
    expect(trigger.parentElement!.getAttribute('data-size')).toBe('md');
    await act(async () => trigger.focus());
    await act(async () => trigger.click());
    expect(document.querySelector('[role="dialog"] [role="listbox"]') !== null).toBe(true);
    await act(async () => trigger.dispatchEvent(new KeyboardEvent('keydown', { key: 'Escape', bubbles: true, cancelable: true })));
    expect(closed).toBe(0);
    expect(trigger.getAttribute('aria-expanded')).toBe('false');
    await act(async () => trigger.dispatchEvent(new KeyboardEvent('keydown', { key: 'Escape', bubbles: true, cancelable: true })));
    expect(closed).toBe(1);
  });
});
