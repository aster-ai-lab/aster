import { afterEach, expect, it } from 'bun:test';
import { act } from 'react';
import { createRoot, type Root } from 'react-dom/client';
import { PresentationEditor } from '../src/internal/features/work/editors/presentation-editor';
import { createWorkArtifact } from '../src/internal/features/work/work-templates';

let root: Root | undefined;
afterEach(async () => {
  await act(async () => root?.unmount());
  root = undefined;
  document.body.replaceChildren();
});

it('previews real slides inline with bounded paging and no slideshow controls or edits', async () => {
  const artifact = createWorkArtifact('strategy-deck');
  if (artifact.content.type !== 'presentation') throw new Error('Expected presentation');
  const content = artifact.content;
  const host = document.createElement('div');
  document.body.append(host);
  root = createRoot(host);
  let changes = 0;
  await act(async () => root!.render(<PresentationEditor content={content} preview previewMode="inline" onChange={() => { changes += 1; }} />));
  expect(host.querySelector('.work-presentation-player')).toBeNull();
  expect(host.querySelector('.work-presentation-ribbon')).toBeNull();
  const previous = host.querySelector<HTMLButtonElement>('button[aria-label="上一张"]')!;
  const next = host.querySelector<HTMLButtonElement>('button[aria-label="下一张"]')!;
  expect(previous.disabled).toBe(true);
  expect(host.textContent).toContain('1 / 3');
  await act(async () => next.click());
  expect(host.textContent).toContain('2 / 3');
  expect(host.textContent).toContain(content.slides[1].elements.find((element) => element.type === 'text')!.text);
  await act(async () => next.click());
  expect(host.textContent).toContain('3 / 3');
  expect(next.disabled).toBe(true);
  await act(async () => previous.click());
  expect(host.textContent).toContain('2 / 3');
  await act(async () => root!.render(<PresentationEditor content={{ ...content, slides: content.slides.slice(0, 1) }} preview previewMode="inline" onChange={() => { changes += 1; }} />));
  expect(host.textContent).toContain('1 / 1');
  expect(next.disabled).toBe(true);
  expect(previous.disabled).toBe(true);
  expect(changes).toBe(0);
});

it('keeps the existing player available for a standalone read-only presentation', async () => {
  const artifact = createWorkArtifact('strategy-deck');
  if (artifact.content.type !== 'presentation') throw new Error('Expected presentation');
  const host = document.createElement('div');
  document.body.append(host);
  root = createRoot(host);
  await act(async () => root!.render(<PresentationEditor content={artifact.content} preview onChange={() => {}} />));
  expect(host.querySelector('.work-presentation-player')).not.toBeNull();
});
