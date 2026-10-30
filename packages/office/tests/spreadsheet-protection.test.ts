import { describe, expect, it } from 'bun:test';
import { checkCellIsLocked, defaultContext, isAllowEdit, type Cell, type Sheet } from '@fortune-sheet/core';
import { defaultSheetProtectionAuthority } from '../src/internal/features/work/work-spreadsheet-protection';

function context(protectedSheet: boolean, cell?: Cell, compactLock?: boolean) {
  const authority = defaultSheetProtectionAuthority(protectedSheet);
  if (compactLock !== undefined) authority.cellProtectionRanges.push({
    range: { row: [0, 0], column: [0, 0] }, locked: compactLock, hidden: false,
  });
  const sheet: Sheet = { id: 'fixture', name: 'Sheet1', data: [[cell ?? { v: 'Draft' }]], config: { authority } };
  const value = defaultContext({} as Parameters<typeof defaultContext>[0]);
  value.config = sheet.config!;
  value.luckysheetfile = [sheet];
  value.currentSheetId = 'fixture';
  value.luckysheet_select_save = [{ row: [0, 0], column: [0, 0] }];
  return value;
}

describe('spreadsheet runtime protection', () => {
  it('keeps imported locked cell properties dormant until sheet protection is enabled', () => {
    for (const value of [context(false, { v: 'Draft', lo: 1 }), context(false, undefined, true)]) {
      expect(checkCellIsLocked(value, 0, 0, 'fixture')).toBe(false);
      expect(isAllowEdit(value)).toBe(true);
    }
  });

  it('continues to reject protected cells while allowing explicit unlocked cells and ranges', () => {
    for (const value of [context(true), context(true, { v: 'Draft', lo: 1 }), context(true, undefined, true)]) {
      expect(checkCellIsLocked(value, 0, 0, 'fixture')).toBe(true);
      expect(isAllowEdit(value)).toBe(false);
    }
    for (const value of [context(true, { v: 'Draft', lo: 0 }), context(true, undefined, false)]) {
      expect(checkCellIsLocked(value, 0, 0, 'fixture')).toBe(false);
      expect(isAllowEdit(value)).toBe(true);
    }
  });
});
