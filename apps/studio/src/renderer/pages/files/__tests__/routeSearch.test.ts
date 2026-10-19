import { describe, expect, it } from 'vitest'

import { parseFilesRouteSearch } from '../routeSearch'

describe('parseFilesRouteSearch', () => {
  it('keeps a non-empty entry id', () => {
    expect(parseFilesRouteSearch({ entryId: 'entry-1' })).toEqual({ entryId: 'entry-1', driveFile: undefined })
  })

  it('keeps a drive file id for an encyclopedia citation', () => {
    expect(parseFilesRouteSearch({ driveFile: 'file-1' })).toEqual({ entryId: undefined, driveFile: 'file-1' })
  })

  it('drops empty and non-string values', () => {
    expect(parseFilesRouteSearch({ entryId: '', driveFile: '' })).toEqual({ entryId: undefined, driveFile: undefined })
    expect(parseFilesRouteSearch({ entryId: 1, driveFile: 2 })).toEqual({ entryId: undefined, driveFile: undefined })
  })
})
