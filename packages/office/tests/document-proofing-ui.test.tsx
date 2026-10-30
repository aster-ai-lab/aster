import { afterEach, describe, expect, it } from 'bun:test';
import { act } from 'react';
import { createRoot, type Root } from 'react-dom/client';
import { DocumentProofingDialog } from '../src/internal/features/work/editors/document-proofing-dialog';
import type { DocumentProofingDialogPatch } from '../src/internal/features/work/editors/document-proofing-dialog-model';
import { OfficeSurface } from '../src/office-surface';

let root: Root | undefined;

afterEach(async () => {
  await act(async () => root?.unmount());
  root = undefined;
  document.body.replaceChildren();
});

async function mount(onApply: (patch: DocumentProofingDialogPatch) => boolean) {
  const host = document.createElement('div');
  document.body.append(host);
  root = createRoot(host);
  await act(async () => root!.render(<OfficeSurface><DocumentProofingDialog
    source={{ latin: { value: 'en-US', mixed: false }, eastAsia: { value: 'zh-CN', mixed: false }, bidi: { value: null, mixed: false }, noProof: { value: false, mixed: false }, selectedCharacters: 0 }}
    restoreFocusTarget={() => null}
    onApply={onApply}
    onClose={() => {}}
  /></OfficeSurface>));
}

describe('document proofing language input', () => {
  it('draws suggestions while preserving arbitrary valid language tags', async () => {
    const applied: DocumentProofingDialogPatch[] = [];
    await mount((patch) => { applied.push(patch); return true; });
    expect(document.querySelector('datalist, select') === null).toBe(true);
    const field = document.querySelector<HTMLInputElement>('input[role="combobox"]')!;
    expect(field !== null).toBe(true);
    await act(async () => {
      Object.getOwnPropertyDescriptor(window.HTMLInputElement.prototype, 'value')!.set!.call(field, 'pt-BR');
      field.dispatchEvent(new Event('input', { bubbles: true }));
    });
    await act(async () => field.dispatchEvent(new KeyboardEvent('keydown', { key: 'Escape', bubbles: true, cancelable: true })));
    expect(field.value).toBe('pt-BR');
    await act(async () => document.querySelector<HTMLButtonElement>('button[type="submit"]')!.click());
    expect(applied).toEqual([{ languages: { latin: 'pt-BR' } }]);
  });

  it('rejects invalid language drafts without calling the editor mutation', async () => {
    let applied = 0;
    await mount(() => { applied += 1; return true; });
    const field = document.querySelector<HTMLInputElement>('input')!;
    await act(async () => {
      Object.getOwnPropertyDescriptor(window.HTMLInputElement.prototype, 'value')!.set!.call(field, 'bad tag!');
      field.dispatchEvent(new Event('input', { bubbles: true }));
    });
    expect(field.getAttribute('aria-invalid')).toBe('true');
    expect(document.querySelector<HTMLButtonElement>('button[type="submit"]')!.disabled).toBe(true);
    expect(applied).toBe(0);
  });

  it('chooses a suggestion without clearing it and retains the inherit action', async () => {
    const applied: DocumentProofingDialogPatch[] = [];
    await mount((patch) => { applied.push(patch); return true; });
    const field = document.querySelector<HTMLInputElement>('input[role="combobox"]')!;
    await act(async () => field.focus());
    const suggestion = Array.from(document.querySelectorAll<HTMLElement>('[role="option"]'))
      .find((option) => option.textContent === 'en-GB');
    expect(suggestion !== undefined).toBe(true);
    await act(async () => suggestion!.click());
    expect(field.value).toBe('en-GB');
    await act(async () => document.querySelector<HTMLButtonElement>('.work-document-proofing-dialog-language .ds-button')!.click());
    await act(async () => document.querySelector<HTMLButtonElement>('button[type="submit"]')!.click());
    expect(applied).toEqual([{ languages: { latin: null } }]);
  });
});
