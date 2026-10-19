import type { WorkBook, WorkSheet } from 'xlsx';
import {
  attribute,
  contentTypeForPart,
  descendants,
  directChild,
  directChildren,
  firstDescendant,
  OoxmlPackage,
  parseXml,
  xmlContainsAnyElement,
} from './work-ooxml-package';
import {
  emptySpreadsheetWorksheetCompatibilitySummary,
  type SpreadsheetWorksheetCompatibilitySummary,
  updateSpreadsheetWorksheetCompatibilitySummary,
} from './work-spreadsheet-compatibility-summary';
import {
  parseSpreadsheetPrintTitles,
  stripSpreadsheetSheetQualifier,
} from './work-spreadsheet-ranges';
import type {
  WorkCompatibilityIssue,
  WorkCompatibilityReport,
} from './work-types';
import { diagnoseXlsxCharts } from './work-xlsx-chart-diagnostics';
import { diagnoseXlsxConditionalFormatting } from './work-xlsx-conditional-format-diagnostics';
import { diagnoseXlsxFormulas } from './work-xlsx-formula-diagnostics';
import type { XlsxFormulaFeatures } from './work-xlsx-formulas';
import { readXlsxImageTransform } from './work-xlsx-image-transform';
import {
  isSupportedXlsxWorksheetImageContentType,
  MAX_XLSX_WORKSHEET_IMAGE_BYTES,
} from './work-xlsx-images';
import { diagnoseXlsxPageSetup } from './work-xlsx-page-setup-diagnostics';
import { diagnoseXlsxPivots } from './work-xlsx-pivot-diagnostics';
import type { XlsxPivotReadResult } from './work-xlsx-pivots';
import { diagnoseXlsxProtection } from './work-xlsx-protection';
import { xlsxWorksheetCellEntries } from './work-xlsx-worksheet';
import type { XlsxWorksheetXmlScan } from './work-xlsx-worksheet-scan';

export interface SpreadsheetCompatibilityImportMetadata {
  formulaFeatures?: XlsxFormulaFeatures | null;
  pivotFeatures?: XlsxPivotReadResult | null;
  worksheetScans?: Readonly<Record<string, XlsxWorksheetXmlScan>>;
}

const XLSX_DIAGNOSTIC_WORKSHEET_ELEMENTS = [
  'conditionalFormatting',
  'dataValidation',
  'sheetProtection',
  'protectedRange',
  'pageSetup',
  'pageMargins',
  'printOptions',
  'headerFooter',
  'pageSetUpPr',
  'rowBreaks',
  'colBreaks',
] as const;

export async function analyzeSpreadsheetCompatibility(
  file: File,
  extension: string,
  workbook: WorkBook,
  sourcePackage?: OoxmlPackage | null,
  worksheetSummaries?: ReadonlyMap<
    string,
    SpreadsheetWorksheetCompatibilitySummary
  >,
  importedMetadata?: SpreadsheetCompatibilityImportMetadata,
): Promise<WorkCompatibilityReport | null> {
  if (extension === 'csv') return null;
  const sourceFormat = extension.toUpperCase();
  const issues: WorkCompatibilityIssue[] = [];

  if (extension === 'xls') {
    issues.push(
      issue(
        'xls.legacy',
        'Legacy workbook',
        'The binary XLS workbook is converted to the modern native sheet model.',
      ),
    );
  } else if (extension === 'ods') {
    issues.push(
      issue(
        'ods.styles',
        'OpenDocument formatting',
        'Advanced ODS formatting and chart behavior may be normalized on export to XLSX.',
      ),
    );
  }

  const definedNames = workbook.Workbook?.Names ?? [];
  const names = definedNames.filter((name) => !/^_xlnm\./i.test(name.Name));
  if (names.length) {
    issues.push(
      issue(
        'sheet.names',
        'Named ranges',
        `${names.length} workbook or worksheet named range(s) are preserved and editable; external references are not refreshed.`,
        'info',
      ),
    );
  }
  const printAreas = definedNames.filter(
    (name) => name.Name.toLowerCase() === '_xlnm.print_area',
  );
  if (printAreas.length) {
    issues.push(
      issue(
        'sheet.print-area',
        'Print areas',
        `${printAreas.length} worksheet print area(s) are preserved and used by PDF export; disjoint regions may be combined into one bounding layout.`,
        'info',
      ),
    );
  }
  const printTitles = definedNames.filter(
    (name) => name.Name.toLowerCase() === '_xlnm.print_titles',
  );
  const validPrintTitles = printTitles.filter((name) => {
    if (typeof name.Sheet !== 'number') return false;
    const sheetName = workbook.SheetNames[name.Sheet];
    return Boolean(
      sheetName &&
        parseSpreadsheetPrintTitles(
          stripSpreadsheetSheetQualifier(name.Ref, sheetName),
        ),
    );
  });
  if (validPrintTitles.length) {
    issues.push(
      issue(
        'sheet.print-titles',
        'Print titles',
        `${validPrintTitles.length} worksheet print-title setting(s) are preserved, editable, and repeated in PDF output.`,
        'info',
      ),
    );
  }
  if (validPrintTitles.length < printTitles.length) {
    issues.push(
      issue(
        'sheet.print-titles.invalid',
        'Print titles',
        'One or more malformed print-title definitions remain in the original workbook only.',
      ),
    );
  }
  for (const [index, name] of workbook.SheetNames.entries()) {
    const worksheet = workbook.Sheets[name];
    inspectWorksheetModel(
      worksheet,
      name,
      issues,
      extension === 'xlsx',
      worksheetSummaries?.get(name),
    );
    if ((workbook.Workbook?.Sheets?.[index]?.Hidden ?? 0) > 0) {
      issues.push(
        issue(
          'sheet.hidden',
          'Hidden worksheets',
          'Hidden worksheet state is preserved but can be changed by the editor.',
          'info',
          name,
        ),
      );
    }
  }

  if (extension === 'xlsx') {
    try {
      const archive =
        sourcePackage === undefined
          ? await OoxmlPackage.load(await file.arrayBuffer())
          : sourcePackage;
      if (!archive) throw new Error('The XLSX package could not be loaded.');
      await inspectXlsxPackage(archive, issues, importedMetadata);
    } catch {
      issues.push(
        issue(
          'xlsx.inspect',
          'Package inspection',
          'Some XLSX features could not be inspected before conversion.',
        ),
      );
    }
  }

  return issues.length ? report(file, sourceFormat, deduplicate(issues)) : null;
}

function inspectWorksheetModel(
  worksheet: WorkSheet | undefined,
  name: string,
  issues: WorkCompatibilityIssue[],
  nativeXlsxFormulaInspection: boolean,
  importedSummary?: SpreadsheetWorksheetCompatibilitySummary,
) {
  if (!worksheet) return;
  if (worksheet['!autofilter']) {
    issues.push(
      issue(
        'sheet.filter',
        'Filters',
        'The auto-filter range and hidden rows are preserved; advanced active criteria may be normalized.',
        'info',
        name,
      ),
    );
  }
  const {
    hasArrayFormulas,
    hasComments,
    hasCommentThreads,
    hasLinks,
    hasRichText,
  } = importedSummary ?? summarizeSpreadsheetWorksheet(worksheet);
  if (hasComments) {
    issues.push(
      issue(
        'sheet.comments',
        'Cell comments',
        'Plain-text cell comments and one author are preserved and editable through the spreadsheet comment tools.',
        'info',
        name,
      ),
    );
  }
  if (hasCommentThreads) {
    issues.push(
      issue(
        'sheet.comment-threads',
        'Comment threads',
        'Multiple comment blocks and threaded replies are flattened into one editable legacy comment.',
        'warning',
        name,
      ),
    );
  }
  if (hasLinks) {
    issues.push(
      issue(
        'sheet.links',
        'Cell hyperlinks',
        'Web and in-workbook hyperlinks are preserved; advanced screen tips may be normalized.',
        'info',
        name,
      ),
    );
  }
  if (hasArrayFormulas && !nativeXlsxFormulaInspection) {
    issues.push(
      issue(
        'sheet.array-formula',
        'Array formulas',
        'Array-formula ranges may be recalculated as ordinary formulas.',
        'warning',
        name,
      ),
    );
  }
  if (hasRichText) {
    issues.push(
      issue(
        'sheet.rich-text',
        'Rich cell text',
        'Rich text inside a cell is converted to uniform text.',
        'warning',
        name,
      ),
    );
  }
}

function summarizeSpreadsheetWorksheet(
  worksheet: WorkSheet,
): SpreadsheetWorksheetCompatibilitySummary {
  const summary = emptySpreadsheetWorksheetCompatibilitySummary();
  for (const { cell } of xlsxWorksheetCellEntries(worksheet)) {
    updateSpreadsheetWorksheetCompatibilitySummary(summary, cell);
  }
  return summary;
}

async function inspectXlsxPackage(
  archive: OoxmlPackage,
  issues: WorkCompatibilityIssue[],
  importedMetadata?: SpreadsheetCompatibilityImportMetadata,
) {
  issues.push(...(await diagnoseXlsxCharts(archive)));
  issues.push(
    ...(await diagnoseXlsxFormulas(
      archive,
      importedMetadata?.formulaFeatures,
      importedMetadata?.worksheetScans,
    )),
  );
  issues.push(
    ...(await diagnoseXlsxPivots(archive, importedMetadata?.pivotFeatures)),
  );
  const drawingParts = archive
    .paths('xl/drawings/')
    .filter((path) => /^xl\/drawings\/drawing\d+\.xml$/i.test(path));
  let preservedImages = 0;
  let hasUnsupportedImages = false;
  let hasImagesBeyondBudget = false;
  let hasNormalizedImageFormatting = false;
  let hasUnsupportedDrawings = false;
  let embeddedImageBytes = 0;
  for (const part of drawingParts) {
    const drawing = await archive.xml(part);
    const relationships = await archive.relationships(part);
    for (const picture of descendants(drawing, 'pic')) {
      const blip = firstDescendant(picture, 'blip');
      const relationshipId = blip
        ? (attribute(blip, 'r:embed') ?? attribute(blip, 'embed'))
        : null;
      const relationship = relationshipId
        ? relationships.get(relationshipId)
        : undefined;
      const contentType = relationship
        ? contentTypeForPart(relationship.target)
        : '';
      if (
        relationship &&
        relationship.targetMode !== 'External' &&
        relationship.type.endsWith('/image') &&
        archive.has(relationship.target) &&
        isSupportedXlsxWorksheetImageContentType(contentType)
      ) {
        const bytes = await archive.bytes(relationship.target);
        if (
          embeddedImageBytes + bytes.byteLength <=
          MAX_XLSX_WORKSHEET_IMAGE_BYTES
        ) {
          embeddedImageBytes += bytes.byteLength;
          preservedImages += 1;
        } else {
          hasImagesBeyondBudget = true;
        }
      } else {
        hasUnsupportedImages = true;
      }
      const sourceRectangle = firstDescendant(picture, 'srcRect');
      const transform = firstDescendant(directChild(picture, 'spPr'), 'xfrm');
      const transformRead = readXlsxImageTransform(transform);
      const hasCrop =
        !!sourceRectangle &&
        Array.from(sourceRectangle.attributes).some(
          (item) => Number(item.value) !== 0,
        );
      if (hasCrop || !transformRead.supported) {
        hasNormalizedImageFormatting = true;
      }
    }
    hasUnsupportedDrawings ||= ['sp', 'cxnSp', 'grpSp', 'contentPart'].some(
      (name) => descendants(drawing, name).length > 0,
    );
    hasUnsupportedDrawings ||= descendants(drawing, 'graphicFrame').some(
      (frame) => !firstDescendant(frame, 'chart'),
    );
  }
  if (preservedImages) {
    issues.push(
      issue(
        'xlsx.images',
        'Worksheet images',
        `${preservedImages} embedded raster worksheet image(s), positions, sizes, names, and alternative text are preserved and editable.`,
        'info',
      ),
    );
  }
  if (hasUnsupportedImages) {
    issues.push(
      issue(
        'xlsx.images.unsupported',
        'Worksheet images',
        'Linked, missing, or browser-incompatible worksheet images remain in the original XLSX only.',
      ),
    );
  }
  if (hasImagesBeyondBudget) {
    issues.push(
      issue(
        'xlsx.images.limit',
        'Large worksheet images',
        'Worksheet images beyond the 10 MiB editable image budget remain in the original XLSX only.',
      ),
    );
  }
  if (hasNormalizedImageFormatting) {
    issues.push(
      issue(
        'xlsx.images.format',
        'Worksheet image formatting',
        'Worksheet image crop or unsupported non-quadrant rotation/flip settings are normalized to an editable unrotated image. Supported 90-degree rotations and flips round-trip natively.',
      ),
    );
  }
  if (
    hasUnsupportedDrawings ||
    (archive.paths('xl/drawings/').length > 0 && drawingParts.length === 0)
  ) {
    issues.push(
      issue(
        'xlsx.drawings.unsupported',
        'Worksheet drawings',
        'Worksheet shapes, connectors, SmartArt, and non-chart drawing objects remain in the original XLSX only.',
      ),
    );
  }
  if (
    archive.paths('xl/comments').length ||
    archive.paths('xl/threadedComments/').length
  ) {
    issues.push(
      issue(
        'xlsx.comments',
        'Comments',
        'Legacy cell comments are imported and editable; rich formatting and threaded conversations may normalize.',
        'info',
      ),
    );
  }
  if (archive.paths('xl/tables/').length) {
    issues.push(
      issue(
        'xlsx.tables',
        'Structured tables',
        'Table values are preserved, but table names, styles, and totals are normalized.',
      ),
    );
  }
  if (archive.paths('xl/externalLinks/').length) {
    issues.push(
      issue(
        'xlsx.external-links',
        'External links',
        'External workbook links are not refreshed by Work.',
      ),
    );
  }
  if (archive.has('xl/vbaProject.bin')) {
    issues.push(
      issue(
        'xlsx.macros',
        'Macros',
        'VBA macros remain in the original workbook and are never executed.',
      ),
    );
  }
  let styles: Document | null = null;
  if (archive.has('xl/styles.xml')) {
    styles = await archive.xml('xl/styles.xml');
    const cellFormats = firstDescendant(styles, 'cellXfs');
    if (cellFormats && directChildren(cellFormats, 'xf').length > 1) {
      issues.push(
        issue(
          'xlsx.styles',
          'Cell formatting',
          'Common cell formatting is imported; advanced borders and number formats may be normalized.',
        ),
      );
    }
  }
  for (const part of archive.paths('xl/worksheets/')) {
    if (!part.endsWith('.xml')) continue;
    const scan = importedMetadata?.worksheetScans?.[part];
    if (scan && !scan.hasDiagnosticFeatures) continue;
    const source = await archive.text(part);
    if (
      !scan &&
      !xmlContainsAnyElement(source, XLSX_DIAGNOSTIC_WORKSHEET_ELEMENTS)
    ) {
      continue;
    }
    const worksheet = parseXml(source, part);
    if (descendants(worksheet, 'conditionalFormatting').length) {
      for (const diagnostic of diagnoseXlsxConditionalFormatting(
        worksheet,
        styles,
      )) {
        issues.push(
          issue(
            diagnostic.code,
            'Conditional formatting',
            diagnostic.message,
            diagnostic.severity,
          ),
        );
      }
    }
    if (descendants(worksheet, 'dataValidation').length) {
      issues.push(
        issue(
          'xlsx.validation',
          'Data validation',
          'Common list, numeric, date, and text-length rules are editable; custom formulas and very large ranges may be normalized.',
          'info',
        ),
      );
    }
    if (
      descendants(worksheet, 'sheetProtection').length ||
      descendants(worksheet, 'protectedRange').length
    ) {
      for (const diagnostic of diagnoseXlsxProtection(worksheet, styles)) {
        issues.push(
          issue(
            diagnostic.code,
            'Sheet protection',
            diagnostic.message,
            diagnostic.severity,
          ),
        );
      }
    }
    issues.push(...diagnoseXlsxPageSetup(worksheet));
    if (
      descendants(worksheet, 'brk').some((pageBreak) => {
        const manual = attribute(pageBreak, 'man')?.toLowerCase();
        return manual === '1' || manual === 'true';
      })
    ) {
      issues.push(
        issue(
          'xlsx.manual-page-breaks',
          'Manual page breaks',
          'Manual row and column page breaks are preserved, editable, and honored by PDF pagination.',
          'info',
        ),
      );
    }
  }
}

function report(
  file: File,
  sourceFormat: string,
  issues: WorkCompatibilityIssue[],
): WorkCompatibilityReport {
  return {
    sourceFormat,
    sourceName: file.name,
    assessedAt: Date.now(),
    issues: deduplicate(issues),
  };
}

function issue(
  code: string,
  feature: string,
  message: string,
  severity: WorkCompatibilityIssue['severity'] = 'warning',
  location?: string,
): WorkCompatibilityIssue {
  return { code, feature, message, severity, location };
}

function deduplicate(
  issues: WorkCompatibilityIssue[],
): WorkCompatibilityIssue[] {
  const seen = new Set<string>();
  return issues.filter((item) => {
    const key = `${item.code}:${item.location ?? ''}`;
    if (seen.has(key)) return false;
    seen.add(key);
    return true;
  });
}
