import { materializeWorkFileSource } from './work-file-data';
import {
  type WorkFileImportOptions,
  WorkFileImportController,
} from './work-file-import';
import { workFileExtension, workKindForFile } from './work-file-kind';
import {
  assertOoxmlArchiveWithinLimits,
  isZipArchive,
} from './work-ooxml-archive-guard';
import {
  createWorkSpreadsheetBlob,
  type WorkSpreadsheetExportFormat,
} from './work-spreadsheet-file-export';
import { importWorkSpreadsheetFile } from './work-spreadsheet-file-import';
import type { WorkArtifact } from './work-types';

export async function importWorkSpreadsheetOpen(
  file: File,
  options: WorkFileImportOptions = {},
): Promise<WorkArtifact> {
  if (workKindForFile(file) !== 'spreadsheet') {
    throw new Error('只能导入表格文件。');
  }
  const controller = new WorkFileImportController(options, file.size);
  const source = await materializeWorkFileSource(file, controller);
  const sourceBytes = new Uint8Array(source.bytes);
  if (isZipArchive(sourceBytes)) assertOoxmlArchiveWithinLimits(sourceBytes);
  const context = {
    bytes: source.bytes,
    controller,
    preserveDelimitedText: options.preserveDelimitedText,
    ...(options.spreadsheetSheetIds
      ? { spreadsheetSheetIds: options.spreadsheetSheetIds }
      : {}),
  };
  controller.report('parsing', 0);
  const artifact = await importWorkSpreadsheetFile(
    source.file,
    workFileExtension(file.name),
    context,
  );
  controller.report('analyzing', 1);
  controller.report('finalizing', 0);
  controller.complete();
  return artifact;
}

export async function createWorkSpreadsheetArtifactBlob(
  artifact: WorkArtifact,
  options?: { spreadsheetFormat?: WorkSpreadsheetExportFormat },
): Promise<Blob> {
  if (artifact.kind !== 'spreadsheet') {
    throw new Error('只能导出表格。');
  }
  return createWorkSpreadsheetBlob(artifact, options?.spreadsheetFormat);
}
