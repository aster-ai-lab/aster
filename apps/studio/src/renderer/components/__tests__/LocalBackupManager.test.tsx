import '@testing-library/jest-dom/vitest'
import { act, cleanup, fireEvent, render, screen, waitFor, within } from '@testing-library/react'
import type { ReactNode } from 'react'
import { afterEach, beforeEach, describe, expect, it, vi } from 'vitest'

import { toast } from '@renderer/services/toast'

import { LocalBackupManager } from '../LocalBackupManager'

type Column = { id?: string; accessorKey?: string; cell?: (context: unknown) => ReactNode }
type TableProps = {
  columns: Column[]
  data: { fileName: string }[]
  rowMotion?: (record: { fileName: string }) => string | undefined
}

vi.mock('@a3s-lab/ui', async (importOriginal) => ({
  ...(await importOriginal<object>()),
  DataTable: ({ columns, data, rowMotion }: TableProps) => (
    <table>
      <tbody>
        {data.map((record) => (
          <tr key={record.fileName} data-motion={rowMotion?.(record)}>
            {columns.map((column) => (
              <td key={column.id ?? column.accessorKey}>
                {column.id === 'action'
                  ? column.cell?.({ row: { original: record } })
                  : column.accessorKey === 'fileName'
                    ? record.fileName
                    : null}
              </td>
            ))}
          </tr>
        ))}
      </tbody>
    </table>
  )
}))

const files = [
  { fileName: 'a.zip', modifiedTime: '2026-10-01T00:00:00Z', size: 10 },
  { fileName: 'b.zip', modifiedTime: '2026-10-02T00:00:00Z', size: 20 }
]

function deferred() {
  let resolve!: () => void
  let reject!: (error: Error) => void
  const promise = new Promise<void>((done, fail) => {
    resolve = done
    reject = fail
  })
  return { promise, resolve, reject }
}

const row = (fileName: string) => screen.getByText(fileName).closest('tr') as HTMLElement

describe('LocalBackupManager', () => {
  let list: ReturnType<typeof vi.fn>
  let remove: ReturnType<typeof vi.fn>

  beforeEach(() => {
    list = vi.fn(async () => files)
    remove = vi.fn()
    vi.stubGlobal('api', { backup: { listLocalBackupFiles: list, deleteLocalBackupFile: remove } })
  })

  afterEach(() => {
    cleanup()
    vi.unstubAllGlobals()
  })

  async function deleteRow(fileName: string) {
    render(<LocalBackupManager visible onClose={() => {}} localBackupDir="/backups" />)
    await screen.findByText(fileName)
    fireEvent.click(within(row(fileName)).getByRole('button', { name: '删除' }))
  }

  it('starts the row leaving before the owner deletes it, without a loading overlay', async () => {
    const write = deferred()
    remove.mockReturnValueOnce(write.promise)
    await deleteRow('a.zip')

    await waitFor(() => expect(row('a.zip')).toHaveAttribute('data-motion', 'leaving'))
    expect(remove).toHaveBeenCalledWith('a.zip', '/backups')

    list.mockResolvedValue(files.slice(1))
    await act(async () => write.resolve())
    await waitFor(() => expect(screen.queryByText('a.zip')).not.toBeInTheDocument())
    expect(screen.queryByText('加载中...')).not.toBeInTheDocument()
    expect(toast.success).toHaveBeenCalled()
  })

  it('restores the row when the owner refuses the delete', async () => {
    const write = deferred()
    remove.mockReturnValueOnce(write.promise)
    await deleteRow('a.zip')
    await waitFor(() => expect(row('a.zip')).toHaveAttribute('data-motion', 'leaving'))

    await act(async () => write.reject(new Error('locked')))
    await waitFor(() => expect(row('a.zip')).toHaveAttribute('data-motion', 'restored'))
    expect(toast.error).toHaveBeenCalled()
  })
})
