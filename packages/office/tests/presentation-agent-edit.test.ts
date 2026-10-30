import { expect, it } from 'bun:test';
import { applyPresentationAgentProposalChanges } from '../src/internal/features/work/work-agent-proposal-apply';
import { createWorkArtifact } from '../src/internal/features/work/work-templates';

it('keeps human changes to the same element and applies independent Agent edits without losing notes', () => {
  const artifact = createWorkArtifact('strategy-deck');
  if (artifact.content.type !== 'presentation') throw new Error('Expected presentation');
  const content = artifact.content;
  const slide = content.slides[0];
  const target = slide.elements.find((element) => element.type === 'text');
  if (!target) throw new Error('Expected title');
  const change = { id: `text:${target.id}`, label: 'Title', before: target.text ?? '', after: 'Agent title', reason: '' };
  const human = { ...content, slides: content.slides.map((item) => item.id === slide.id ? {
    ...item, notes: 'Human notes', elements: item.elements.map((element) => element.id === target.id ? { ...element, text: 'Human title' } : element),
  } : item) };
  const conflict = applyPresentationAgentProposalChanges(human, slide.id, [change]);
  expect(conflict.result.appliedTargetIds).toEqual([]);
  expect(conflict.result.conflicts).toHaveLength(1);
  expect(conflict.content).toBe(human);
  const independent = { ...content, slides: content.slides.map((item) => item.id === slide.id ? { ...item, notes: 'Human notes' } : item) };
  const applied = applyPresentationAgentProposalChanges(independent, slide.id, [change]);
  expect(applied.result.appliedTargetIds).toEqual([change.id]);
  expect(applied.content.slides[0].notes).toBe('Human notes');
  expect(applied.content.slides[0].elements.find((element) => element.id === target.id)?.text).toBe('Agent title');
});
