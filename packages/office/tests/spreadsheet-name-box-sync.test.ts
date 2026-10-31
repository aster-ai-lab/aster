import { afterEach, describe, expect, it } from 'bun:test';
import { bindSpreadsheetNameBox } from '../src/internal/features/work/editors/spreadsheet-name-box-sync';

afterEach(() => document.body.replaceChildren());

function workbook(text: string) {
  const container = document.createElement('div');
  const box = document.createElement('div');
  box.className = 'fortune-name-box';
  box.textContent = text;
  container.append(box);
  document.body.append(container);
  return { container, box };
}

const settle = () => new Promise((resolve) => setTimeout(resolve, 0));

describe('spreadsheet name box sync', () => {
  it('names the active cell instead of the stale range Fortune last drew', () => {
    const { container, box } = workbook('A25');
    const release = bindSpreadsheetNameBox(container, 'C2');
    expect(box.textContent).toBe('C2');
    release();
  });

  it('reverts a later rewrite by Fortune until released', async () => {
    const { container, box } = workbook('C2');
    const release = bindSpreadsheetNameBox(container, 'C2');
    box.textContent = 'A25';
    await settle();
    expect(box.textContent).toBe('C2');
    release();
    box.textContent = 'A25';
    await settle();
    expect(box.textContent).toBe('A25');
  });

  it('waits for a formula bar that mounts after the binding', async () => {
    const container = document.createElement('div');
    document.body.append(container);
    const release = bindSpreadsheetNameBox(container, 'B7');
    const box = document.createElement('div');
    box.className = 'fortune-name-box';
    box.textContent = 'A1';
    container.append(box);
    await settle();
    expect(box.textContent).toBe('B7');
    release();
  });

  it('rebinds when Fortune remounts the name box under the same container', async () => {
    const { container, box: first } = workbook('A25');
    const release = bindSpreadsheetNameBox(container, 'C2');
    expect(first.textContent).toBe('C2');
    first.remove();
    const second = document.createElement('div');
    second.className = 'fortune-name-box';
    second.textContent = 'A25';
    container.append(second);
    await settle();
    expect(second.textContent).toBe('C2');
    second.textContent = 'A25';
    await settle();
    expect(second.textContent).toBe('C2');
    release();
  });
});
