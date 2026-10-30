import { afterEach, describe, expect, it } from 'bun:test';
import { act } from 'react';
import { createRoot, type Root } from 'react-dom/client';
import { OfficeSurface } from '../src/office-surface';
import { useOfficeDialog } from '../src/internal/features/work/editors/office-dialog';
import { OfficeMessagesProvider } from '../src/internal/features/work/editors/office-messages-context';
import { resolveOfficeMessages } from '../src/internal/i18n/office-locale';

let root: Root | undefined;

afterEach(async () => {
  await act(async () => root?.unmount());
  root = undefined;
  document.body.replaceChildren();
});

const catalog = resolveOfficeMessages({ locale: 'en-US', messages: {
  'office.dialog.ok': 'Accept',
  'office.dialog.required': 'Value needed',
  'office.dialog.gotIt': 'Understood',
  'office.dialog.continue': 'Proceed',
  'office.dialog.cancel': 'Back',
} });

function Harness() {
  const dialog = useOfficeDialog();
  return <OfficeSurface>
    <button onClick={() => { void dialog.prompt({ title: 'Prompt', fieldLabel: 'Value', required: true }); }}>Prompt</button>
    <button onClick={() => { void dialog.notice({ title: 'Notice' }); }}>Notice</button>
    <button onClick={() => { void dialog.confirm({ title: 'Confirm' }); }}>Confirm</button>
    {dialog.dialog}
  </OfficeSurface>;
}

async function openDialog(kind: 'Prompt' | 'Notice' | 'Confirm') {
  const host = document.createElement('div');
  document.body.append(host);
  root = createRoot(host);
  await act(async () => root!.render(<OfficeMessagesProvider catalog={catalog}><Harness /></OfficeMessagesProvider>));
  const invoker = [...document.querySelectorAll('button')].find((button) => button.textContent === kind)!;
  await act(async () => invoker.click());
}

describe('Office imperative dialog messages', () => {
  for (const [kind, labels] of [
    ['Prompt', ['Back', 'Accept']],
    ['Notice', ['Understood']],
    ['Confirm', ['Back', 'Proceed']],
  ] as const) {
    it(`uses the owning catalog for ${kind} defaults and validation`, async () => {
      await openDialog(kind);
      expect([...document.querySelectorAll('.ds-dialog footer button')].map((button) => button.textContent)).toEqual([...labels]);
      if (kind === 'Prompt') {
        const input = document.querySelector<HTMLInputElement>('.ds-dialog input')!;
        await act(async () => input.dispatchEvent(new FocusEvent('focusout', { bubbles: true })));
        expect(document.querySelector('[role="alert"]')?.textContent).toBe('Value needed');
      }
    });
  }

  it('does not reopen a cancelled prompt when its leaving input loses focus', async () => {
    await openDialog('Prompt');
    const input = document.querySelector<HTMLInputElement>('.ds-dialog input')!;
    await act(async () => input.dispatchEvent(new KeyboardEvent('keydown', { key: 'Escape', bubbles: true })));
    const panel = document.querySelector<HTMLElement>('.ds-dialog')!;
    expect(panel.getAttribute('data-state')).toBe('closed');
    await act(async () => input.dispatchEvent(new FocusEvent('focusout', { bubbles: true })));
    expect(panel.getAttribute('data-state')).toBe('closed');
    await act(async () => { await new Promise((resolve) => setTimeout(resolve, 250)); });
    expect(document.querySelector('.ds-dialog') === null).toBe(true);
  });
});
