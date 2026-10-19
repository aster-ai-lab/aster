import type {
  WorkAgentProposalApplyResult,
  WorkAgentProposalChange,
  WorkAgentProposalConflict,
} from './work-agent-proposal';
import { presentationElementText } from './work-presentation-agent-context';
import type {
  WorkPresentationContent,
  WorkSlideElement,
} from './work-types';

export interface WorkAgentProposalApplyOutcome<T> {
  content: T;
  result: WorkAgentProposalApplyResult;
}

export { applySpreadsheetAgentProposalChanges } from './work-spreadsheet-agent-proposal-apply';

export function applyPresentationAgentProposalChanges(
  content: WorkPresentationContent,
  slideId: string,
  changes: readonly WorkAgentProposalChange[],
): WorkAgentProposalApplyOutcome<WorkPresentationContent> {
  const slideIndex = content.slides.findIndex((slide) => slide.id === slideId);
  if (slideIndex < 0)
    return failedOutcome(content, changes, '目标幻灯片已不存在。');

  const originalSlide = content.slides[slideIndex];
  const nextSlide = { ...originalSlide, elements: [...originalSlide.elements] };
  const clonedElements = new Set<number>();
  const appliedTargetIds: string[] = [];
  const conflicts: WorkAgentProposalConflict[] = [];
  const mutableElement = (index: number): WorkSlideElement => {
    if (!clonedElements.has(index)) {
      nextSlide.elements[index] = { ...nextSlide.elements[index] };
      clonedElements.add(index);
    }
    return nextSlide.elements[index];
  };

  for (const change of changes) {
    if (change.id === 'notes') {
      if ((nextSlide.notes ?? '') !== change.before) {
        conflicts.push(conflict(change, '演讲者备注在建议生成后已发生变化。'));
        continue;
      }
      nextSlide.notes = change.after;
      appliedTargetIds.push(change.id);
      continue;
    }

    if (change.id.startsWith('text:')) {
      const elementId = change.id.slice('text:'.length);
      const elementIndex = nextSlide.elements.findIndex(
        (element) => element.id === elementId,
      );
      if (elementIndex < 0) {
        conflicts.push(conflict(change, '目标演示元素已不存在。'));
        continue;
      }
      const element = nextSlide.elements[elementIndex];
      if (presentationElementText(element) !== change.before) {
        conflicts.push(conflict(change, '元素文本在建议生成后已发生变化。'));
        continue;
      }
      Object.assign(mutableElement(elementIndex), {
        text: change.after,
        textRuns: undefined,
      });
      appliedTargetIds.push(change.id);
      continue;
    }

    const tableTarget = change.id.match(/^table:(.+):(\d+):(\d+)$/);
    if (!tableTarget) {
      conflicts.push(conflict(change, '建议中的演示目标无效。'));
      continue;
    }
    const [, elementId, rowText, columnText] = tableTarget;
    const row = Number(rowText);
    const column = Number(columnText);
    const elementIndex = nextSlide.elements.findIndex(
      (element) => element.id === elementId,
    );
    const element = elementIndex >= 0 ? nextSlide.elements[elementIndex] : null;
    const current = element?.table?.rows[row]?.[column];
    if (current === undefined) {
      conflicts.push(conflict(change, '目标表格单元格已不存在。'));
      continue;
    }
    if (current !== change.before) {
      conflicts.push(conflict(change, '表格单元格在建议生成后已发生变化。'));
      continue;
    }
    const nextElement = mutableElement(elementIndex);
    const rows =
      nextElement.table?.rows.map((candidate) => [...candidate]) ?? [];
    rows[row][column] = change.after;
    nextElement.table = { ...nextElement.table!, rows };
    appliedTargetIds.push(change.id);
  }

  if (!appliedTargetIds.length)
    return { content, result: { appliedTargetIds, conflicts } };
  const slides = [...content.slides];
  slides[slideIndex] = nextSlide;
  return {
    content: { ...content, slides },
    result: { appliedTargetIds, conflicts },
  };
}

function failedOutcome<T>(
  content: T,
  changes: readonly WorkAgentProposalChange[],
  message: string,
): WorkAgentProposalApplyOutcome<T> {
  return {
    content,
    result: {
      appliedTargetIds: [],
      conflicts: changes.map((change) => conflict(change, message)),
    },
  };
}

function conflict(
  change: WorkAgentProposalChange,
  message: string,
): WorkAgentProposalConflict {
  return {
    targetId: change.id,
    label: change.label,
    message,
  };
}
