import type { Cell } from '@fortune-sheet/core';
import type {
  WorkAgentProposalApplyResult,
  WorkAgentProposalChange,
  WorkAgentProposalConflict,
} from './work-agent-proposal';
import { spreadsheetCellSourceText } from './work-spreadsheet-agent-context';
import type { WorkSpreadsheetContent } from './work-types';

export interface WorkAgentProposalApplyOutcome<T> {
  content: T;
  result: WorkAgentProposalApplyResult;
}

export function applySpreadsheetAgentProposalChanges(
  content: WorkSpreadsheetContent,
  sheetId: string,
  changes: readonly WorkAgentProposalChange[],
): WorkAgentProposalApplyOutcome<WorkSpreadsheetContent> {
  const sheetIndex = content.sheets.findIndex((sheet) => sheet.id === sheetId);
  if (sheetIndex < 0)
    return failedOutcome(content, changes, '目标工作表已不存在。');

  const sheet = content.sheets[sheetIndex];
  const data = [...(sheet.data ?? [])];
  const clonedRows = new Set<number>();
  const appliedTargetIds: string[] = [];
  const conflicts: WorkAgentProposalConflict[] = [];

  for (const change of changes) {
    const coordinate = parseCellReference(change.id);
    if (!coordinate) {
      conflicts.push(conflict(change, '建议中的单元格坐标无效。'));
      continue;
    }
    const currentCell = data[coordinate.row]?.[coordinate.column] ?? null;
    if (spreadsheetCellSourceText(currentCell) !== change.before) {
      conflicts.push(conflict(change, '单元格内容在建议生成后已发生变化。'));
      continue;
    }
    if (!clonedRows.has(coordinate.row)) {
      data[coordinate.row] = [...(data[coordinate.row] ?? [])];
      clonedRows.add(coordinate.row);
    }
    data[coordinate.row][coordinate.column] = spreadsheetCellWithContent(
      currentCell,
      change.after,
    );
    appliedTargetIds.push(change.id);
  }

  if (!appliedTargetIds.length)
    return { content, result: { appliedTargetIds, conflicts } };
  const sheets = [...content.sheets];
  sheets[sheetIndex] = { ...sheet, data };
  return {
    content: { ...content, sheets },
    result: { appliedTargetIds, conflicts },
  };
}

function spreadsheetCellWithContent(cell: Cell | null, value: string): Cell {
  const next: Cell = cell ? { ...cell } : {};
  if (value.startsWith('=')) {
    next.f = value;
    delete next.v;
    delete next.m;
    return next;
  }
  delete next.f;
  if (!value) {
    delete next.v;
    delete next.m;
    return next;
  }
  next.v = spreadsheetScalar(value);
  next.m = value;
  return next;
}

function spreadsheetScalar(value: string): string | number | boolean {
  if (/^[+-]?(?:0|[1-9]\d*)(?:\.\d+)?(?:e[+-]?\d+)?$/i.test(value)) {
    const numeric = Number(value);
    if (Number.isFinite(numeric)) return numeric;
  }
  if (value.toLocaleUpperCase() === 'TRUE') return true;
  if (value.toLocaleUpperCase() === 'FALSE') return false;
  return value;
}

function parseCellReference(
  reference: string,
): { row: number; column: number } | null {
  const match = reference.match(/^([A-Z]+)([1-9]\d*)$/i);
  if (!match) return null;
  let column = 0;
  for (const character of match[1].toLocaleUpperCase()) {
    column = column * 26 + character.charCodeAt(0) - 64;
  }
  const row = Number(match[2]) - 1;
  if (!Number.isSafeInteger(row) || row < 0 || column < 1) return null;
  return { row, column: column - 1 };
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
