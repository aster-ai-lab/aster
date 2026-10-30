import { afterAll, beforeAll, describe, expect, it } from 'bun:test';
import { createRequire } from 'node:module';
import JSZip from 'jszip';
import * as XLSX from 'xlsx';
import { WorkFileImportController } from '../src/internal/features/work/work-file-import';
import { OoxmlPackage } from '../src/internal/features/work/work-ooxml-package';
import { importWorkSpreadsheetFile } from '../src/internal/features/work/work-spreadsheet-file-import';
import { readPlainXlsxPackage } from '../src/internal/features/work/work-spreadsheet-package-scan-worker-client';
import { spreadsheetMatrixProfile } from '../src/internal/features/work/work-spreadsheet-matrix-profile';

// Reuse the workspace host's installed DOM test dependency; production Office does not depend on it.
const require = createRequire(new URL('../../../apps/studio/package.json', import.meta.url));
const { JSDOM } = require('jsdom');
const dom = new JSDOM('');
const previous = {
  DOMParser: globalThis.DOMParser,
  XMLSerializer: globalThis.XMLSerializer,
  Worker: globalThis.Worker,
};

beforeAll(() => {
  globalThis.DOMParser = dom.window.DOMParser;
  globalThis.XMLSerializer = dom.window.XMLSerializer;
  Object.defineProperty(globalThis, 'Worker', { configurable: true, value: undefined });
});

afterAll(() => {
  Object.assign(globalThis, previous);
  dom.window.close();
});

async function importBytes(bytes: ArrayBuffer) {
  const original = new Uint8Array(bytes).slice();
  const file = new File([bytes], '清洗 数据.xlsx');
  const artifact = await importWorkSpreadsheetFile(file, 'xlsx', {
    bytes,
    controller: new WorkFileImportController({}, bytes.byteLength),
  });
  expect(new Uint8Array(bytes)).toEqual(original);
  if (artifact.content.type !== 'spreadsheet') throw new Error('Expected a spreadsheet');
  return artifact.content.sheets[0].data!;
}

describe('numeric spreadsheet import', () => {
  it('materializes merged cells before the imported matrix enters editor history', async () => {
    const sheet = XLSX.utils.aoa_to_sheet([['Heading', 'Covered'], []]);
    sheet['!merges'] = [{ s: { r: 0, c: 0 }, e: { r: 1, c: 2 } }];
    const workbook = XLSX.utils.book_new();
    XLSX.utils.book_append_sheet(workbook, sheet, 'Merged');
    const data = await importBytes(XLSX.write(workbook, { type: 'array', bookType: 'xlsx' }));
    expect(data[0][0]?.v).toBe('Heading');
    expect(data[0][0]?.mc).toEqual({ r: 0, c: 0, rs: 2, cs: 3 });
    expect(data[0][1]?.v).toBe('Covered');
    expect(data[0][1]?.mc).toEqual({ r: 0, c: 0 });
    expect(data[1][2]?.mc).toEqual({ r: 0, c: 0 });
    expect(spreadsheetMatrixProfile(data)?.fortuneReady).toBe(true);
    expect(spreadsheetMatrixProfile(data)?.populatedCellCount).toBe(6);
  });

  it('renders typed General numbers, booleans and cached formulas without changing date or hidden formats', async () => {
    const sheet = XLSX.utils.aoa_to_sheet([[12000, 0, -45, 1.5, true, null, 46030, 99, '【缺失】']]);
    sheet.F1 = { t: 'n', v: 11955, f: 'SUM(A1:C1)' };
    sheet.G1.z = 'yyyy-mm-dd';
    sheet.H1.z = ';;;';
    const workbook = XLSX.utils.book_new();
    XLSX.utils.book_append_sheet(workbook, sheet, '清洗后数据');
    const data = await importBytes(XLSX.write(workbook, { type: 'array', bookType: 'xlsx' }));

    for (const [column, value, display] of [
      [0, 12000, '12000'],
      [1, 0, '0'],
      [2, -45, '-45'],
      [3, 1.5, '1.5'],
      [4, true, 'TRUE'],
      [5, 11955, '11955'],
    ] as const) {
      expect(data[0][column]?.v).toBe(value);
      expect(data[0][column]?.m).toBe(display);
    }
    expect(data[0][5]?.f).toBe('=SUM(A1:C1)');
    expect(data[0][6]?.v).toBe(46030);
    expect(data[0][6]?.m).toBe('2026-01-08');
    expect(data[0][7]?.v).toBe(99);
    expect(data[0][7]?.m).toBe('');
    expect(data[0][8]?.v).toBe('【缺失】');
    expect(data[0][9]).toBeUndefined();
  });

  it('also renders numbers and booleans from the plain XLSX fast path', async () => {
    const zip = new JSZip();
    const main = 'http://schemas.openxmlformats.org/spreadsheetml/2006/main';
    const rel = 'http://schemas.openxmlformats.org/officeDocument/2006/relationships';
    const packageRel = 'http://schemas.openxmlformats.org/package/2006/relationships';
    zip.file('[Content_Types].xml', '<Types xmlns="http://schemas.openxmlformats.org/package/2006/content-types"><Default Extension="rels" ContentType="application/vnd.openxmlformats-package.relationships+xml"/><Default Extension="xml" ContentType="application/xml"/><Override PartName="/xl/workbook.xml" ContentType="application/vnd.openxmlformats-officedocument.spreadsheetml.sheet.main+xml"/><Override PartName="/xl/worksheets/sheet1.xml" ContentType="application/vnd.openxmlformats-officedocument.spreadsheetml.worksheet+xml"/></Types>');
    zip.file('_rels/.rels', `<Relationships xmlns="${packageRel}"><Relationship Id="rId1" Type="${rel}/officeDocument" Target="xl/workbook.xml"/></Relationships>`);
    zip.file('xl/workbook.xml', `<workbook xmlns="${main}" xmlns:r="${rel}"><sheets><sheet name="数据" sheetId="1" r:id="rId1"/></sheets></workbook>`);
    zip.file('xl/_rels/workbook.xml.rels', `<Relationships xmlns="${packageRel}"><Relationship Id="rId1" Type="${rel}/worksheet" Target="worksheets/sheet1.xml"/></Relationships>`);
    zip.file('xl/worksheets/sheet1.xml', `<worksheet xmlns="${main}"><dimension ref="A1:F1"/><sheetData><row r="1"><c r="A1"><v>12000</v></c><c r="B1"><v>0</v></c><c r="C1"><v>-45</v></c><c r="D1"><v>1.5</v></c><c r="E1" t="b"><v>1</v></c><c r="F1" t="b"><v>0</v></c></row></sheetData></worksheet>`);
    const bytes = await zip.generateAsync({ type: 'arraybuffer' });
    const plain = await readPlainXlsxPackage(await OoxmlPackage.load(bytes));
    expect(plain?.plainWorksheets?.['数据']?.data[0]?.[0]).toMatchObject({ v: 12000, m: '12000' });
    const data = await importBytes(bytes);
    for (const [column, value, display] of [
      [0, 12000, '12000'], [1, 0, '0'], [2, -45, '-45'],
      [3, 1.5, '1.5'], [4, true, 'TRUE'], [5, false, 'FALSE'],
    ] as const) {
      expect(data[0][column]?.v).toBe(value);
      expect(data[0][column]?.m).toBe(display);
    }
  });
});
