export type FilesRouteSearch = {
  entryId?: string
  driveFile?: string
}

export function parseFilesRouteSearch(search: Record<string, unknown>): FilesRouteSearch {
  return {
    entryId: typeof search.entryId === 'string' && search.entryId.length > 0 ? search.entryId : undefined,
    driveFile: typeof search.driveFile === 'string' && search.driveFile.length > 0 ? search.driveFile : undefined
  }
}
