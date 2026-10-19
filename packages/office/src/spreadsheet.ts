export type { WorkFileImportOptions as OfficeFileImportOptions } from './internal/features/work/work-file-import';
export {
  createWorkSpreadsheetArtifactBlob as createArtifactBlob,
  importWorkSpreadsheetOpen as importOfficeFile,
} from './internal/features/work/work-spreadsheet-open';
export type {
  WorkArtifact as OfficeArtifact,
  WorkSpreadsheetContent as SpreadsheetContent,
} from './internal/features/work/work-types';
