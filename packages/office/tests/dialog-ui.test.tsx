import { afterEach, describe, expect, it } from 'bun:test';
import { act, useState } from 'react';
import { AnimatePresence } from 'motion/react';
import { createRoot, type Root } from 'react-dom/client';
import { Dialog } from '../src/internal/design-system/primitives/dialog/dialog';
import { OfficeSurface } from '../src/office-surface';

let root: Root | undefined;

afterEach(async () => {
  await act(async () => root?.unmount());
  root = undefined;
  document.body.replaceChildren();
});

async function mount(closeDisabled = false) {
  let closed = 0;
  let cancelled = 0;
  const host = document.createElement('div');
  document.body.append(host);
  root = createRoot(host);
  await act(async () => root!.render(<OfficeSurface>
    <Dialog title="Format" closeLabel="Close format" closeDisabled={closeDisabled} onClose={() => { closed += 1; }} onEscape={() => { cancelled += 1; }}>
      <button type="button">Inside</button>
    </Dialog>
  </OfficeSurface>));
  return { counts: () => [closed, cancelled] };
}

async function mountPresence() {
  const host = document.createElement('div');
  document.body.append(host);
  root = createRoot(host);
  function Harness() {
    const [open, setOpen] = useState(false);
    return <OfficeSurface>
      <button id="invoker" onClick={() => setOpen(true)}>Open</button>
      <AnimatePresence>{open && <Dialog title="Format" closeLabel="Close format" onClose={() => setOpen(false)}>
        <input aria-label="Name" />
      </Dialog>}</AnimatePresence>
    </OfficeSurface>;
  }
  await act(async () => root!.render(<Harness />));
  const invoker = document.getElementById('invoker')!;
  invoker.focus();
  await act(async () => invoker.click());
  return { invoker, panel: document.querySelector<HTMLElement>('.ds-dialog')! };
}

describe('Office dialog dismissal', () => {
  it('retains a closing dialog and background isolation until the exit completes, then restores focus', async () => {
    const { invoker, panel } = await mountPresence();
    expect(invoker.inert).toBe(true);
    await act(async () => panel.querySelector<HTMLButtonElement>('[aria-label="Close format"]')!.click());
    expect(panel.isConnected).toBe(true);
    expect(panel.getAttribute('data-state')).toBe('closed');
    expect(invoker.inert).toBe(true);
    await act(async () => { await new Promise((resolve) => setTimeout(resolve, 250)); });
    expect(panel.isConnected).toBe(false);
    expect(invoker.inert).toBe(false);
    expect(document.activeElement === invoker).toBe(true);
  });

  it('finishes on its own exit animation, ignoring descendant and entrance events', async () => {
    const { panel } = await mountPresence();
    await act(async () => panel.querySelector<HTMLButtonElement>('[aria-label="Close format"]')!.click());
    const end = (target: HTMLElement, name: string) => {
      const event = new Event('animationend', { bubbles: true });
      Object.defineProperty(event, 'animationName', { value: name });
      target.dispatchEvent(event);
    };
    await act(async () => end(panel.querySelector('input')!, 'aster-dialog-out'));
    expect(panel.isConnected).toBe(true);
    await act(async () => end(panel, 'aster-dialog-in'));
    expect(panel.isConnected).toBe(true);
    await act(async () => end(panel, 'aster-dialog-out'));
    expect(panel.isConnected).toBe(false);
  });

  it('releases immediately when the platform requests reduced motion', async () => {
    const original = window.matchMedia;
    window.matchMedia = ((query: string) => ({ matches: query === '(prefers-reduced-motion: reduce)', addEventListener() {}, removeEventListener() {} })) as typeof window.matchMedia;
    try {
      const { invoker, panel } = await mountPresence();
      await act(async () => panel.querySelector<HTMLButtonElement>('[aria-label="Close format"]')!.click());
      expect(panel.isConnected).toBe(false);
      expect(invoker.inert).toBe(false);
      expect(document.activeElement === invoker).toBe(true);
    } finally {
      window.matchMedia = original;
    }
  });

  it('uses the supplied close name and preserves the explicit close callback', async () => {
    const dialog = await mount();
    const close = document.querySelector<HTMLButtonElement>('[aria-label="Close format"]');
    expect(close !== null).toBe(true);
    await act(async () => close!.click());
    expect(dialog.counts()).toEqual([1, 0]);
  });

  it('routes scrim dismissal through the existing Escape/draft policy, excluding content clicks', async () => {
    const dialog = await mount();
    await act(async () => document.querySelector<HTMLButtonElement>('.ds-dialog-body button')!.click());
    expect(dialog.counts()).toEqual([0, 0]);
    await act(async () => document.querySelector<HTMLElement>('.ds-dialog-backdrop')!.click());
    expect(dialog.counts()).toEqual([0, 1]);
  });

  it('keeps pending dialogs open on scrim, close and Escape', async () => {
    const dialog = await mount(true);
    const close = document.querySelector<HTMLButtonElement>('.ds-dialog header button')!;
    expect(close.disabled).toBe(true);
    await act(async () => {
      close.click();
      document.querySelector<HTMLElement>('.ds-dialog-backdrop')!.click();
      close.dispatchEvent(new KeyboardEvent('keydown', { key: 'Escape', bubbles: true }));
    });
    expect(dialog.counts()).toEqual([0, 0]);
  });
});
