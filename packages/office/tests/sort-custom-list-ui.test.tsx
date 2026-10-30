import { afterEach, describe, expect, it } from 'bun:test';
import { act } from 'react';
import { createRoot, type Root } from 'react-dom/client';
import { SpreadsheetSortCustomListManagerDialog } from '../src/internal/features/work/editors/spreadsheet-sort-custom-list-manager';

let root: Root | undefined;

afterEach(async () => {
  await act(async () => root?.unmount());
  root = undefined;
  document.body.replaceChildren();
});

async function mount() {
  const invoker = document.createElement('button');
  invoker.textContent = 'Sort';
  const host = document.createElement('div');
  document.body.append(invoker, host);
  invoker.focus();
  root = createRoot(host);
  await act(async () => {
    root!.render(
      <SpreadsheetSortCustomListManagerDialog
        customLists={[
          { source: 'built-in', label: 'Priority', entries: ['High', 'Low'] },
          { source: 'stored', label: 'Team', entries: ['Design', 'Engineering'] },
        ]}
        restoreFocusTarget={() => invoker}
        onApply={() => {}}
        onClose={() => {}}
      />,
    );
  });
  return document.querySelector<HTMLElement>('[role="listbox"]');
}

describe('custom sort list selection', () => {
  it('draws the choices and exposes the current selection without an OS control', async () => {
    const list = await mount();
    expect(document.querySelector('select, datalist') === null).toBe(true);
    expect(list !== null).toBe(true);
    expect(document.activeElement === list).toBe(true);
    const selected = list!.querySelector('[aria-selected="true"]');
    expect(selected?.textContent).toBe('Priority');
    expect(list!.getAttribute('aria-activedescendant')).toBe(selected?.id);
  });

  it('changes the displayed entries with keyboard navigation and pointer selection', async () => {
    const list = await mount();
    expect(list !== null).toBe(true);
    await act(async () => {
      list!.dispatchEvent(new KeyboardEvent('keydown', { key: 'ArrowDown', bubbles: true }));
    });
    expect(document.querySelector('textarea')?.value).toBe('Design\nEngineering');
    expect(list!.querySelector('[aria-selected="true"]')?.textContent).toBe('Design → Engineering');
    await act(async () => {
      list!.querySelector<HTMLElement>('[role="option"]')!.click();
    });
    expect(document.querySelector('textarea')?.value).toBe('High\nLow');
  });

  it('keeps an invalid edited list and its selection when another choice is requested', async () => {
    const list = await mount();
    await act(async () => {
      list!.dispatchEvent(new KeyboardEvent('keydown', { key: 'End', bubbles: true }));
    });
    const field = document.querySelector('textarea')!;
    await act(async () => {
      Object.getOwnPropertyDescriptor(window.HTMLTextAreaElement.prototype, 'value')!
        .set!.call(field, 'Only one');
      field.dispatchEvent(new Event('input', { bubbles: true }));
    });
    await act(async () => {
      list!.dispatchEvent(new KeyboardEvent('keydown', { key: 'Home', bubbles: true }));
    });
    expect(field.value).toBe('Only one');
    expect(list!.querySelector('[aria-selected="true"]')?.textContent).toBe('Design → Engineering');
    expect(document.querySelector('[role="alert"]') !== null).toBe(true);
  });
});
