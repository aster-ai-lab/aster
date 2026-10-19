import { Blob as NodeBlob, File as NodeFile } from 'node:buffer'

import type { SpreadsheetContent } from '@a3s-lab/office/core'
import { cleanup, fireEvent, render, screen, waitFor } from '@testing-library/react'
import userEvent from '@testing-library/user-event'
import { afterEach, beforeEach, expect, it, vi } from 'vitest'

import { OfficeDocumentSurface } from '../OfficeDocumentSurface'

// The editor owns grid interaction; this host test keeps the real Office codecs.
vi.mock('@a3s-lab/office/react', async () => {
  const { createElement, useState } = await import('react')
  return {
    SpreadsheetEditor: ({
      content,
      onChange
    }: {
      content: SpreadsheetContent
      onChange?: (next: SpreadsheetContent) => void
    }) => {
      const [draft, setDraft] = useState(content)
      const update = (next: SpreadsheetContent) => {
        setDraft(next)
        onChange?.(next)
      }
      return createElement(
        'div',
        null,
        createElement(
          'button',
          {
            onClick: () => {
              const sheet = draft.sheets[0]
              if (!sheet?.data?.[1]) throw new Error('Expected imported cells')
              const data = sheet.data.slice()
              data[1] = [{ v: '已编辑' }, ...sheet.data[1].slice(1)]
              update({ ...draft, sheets: [{ ...sheet, data }, ...draft.sheets.slice(1)] })
            }
          },
          'Edit first name'
        ),
        createElement(
          'button',
          {
            onClick: () => {
              const sheet = draft.sheets[0]
              if (!sheet) throw new Error('Expected a worksheet')
              update({ ...draft, sheets: [...draft.sheets, { ...sheet, id: 'second', name: 'Second' }] })
            }
          },
          'Add worksheet'
        ),
        createElement(
          'button',
          {
            onClick: () => update({ ...draft, sheets: draft.sheets.slice(0, 1) })
          },
          'Keep first worksheet'
        )
      )
    }
  }
})

beforeEach(() => {
  vi.stubGlobal('Blob', NodeBlob)
  vi.stubGlobal('File', NodeFile)
  vi.stubGlobal('Worker', undefined)
})

afterEach(() => {
  cleanup()
  vi.unstubAllGlobals()
})

it.each([
  [
    'registrations.csv',
    'name,count,code,date\r\n甲,0,0007,2026-10-01',
    '"name","count","code","date"\r\n"已编辑","0","0007","2026-10-01"'
  ],
  [
    'registrations.TSV',
    'name\tcount\tcode\tdate\r\n甲\t0\t0007\t2026-10-01',
    '"name"\t"count"\t"code"\t"date"\r\n"已编辑"\t"0"\t"0007"\t"2026-10-01"'
  ]
])('saves edits to %s as the original text format', async (fileName, input, expected) => {
  let saved: Uint8Array | undefined
  let dirty = false
  render(
    <OfficeDocumentSurface
      surface="spreadsheet"
      bytes={new TextEncoder().encode(input)}
      fileName={fileName}
      frame="workspace"
      onSave={async (bytes) => {
        saved = bytes
      }}
      onOfficeDirty={(value) => {
        dirty = value
      }}
    />
  )
  const user = userEvent.setup()
  await user.click(await screen.findByRole('button', { name: 'Edit first name' }, { timeout: 10_000 }))
  expect(dirty).toBe(true)
  fireEvent.keyDown(window, { key: 's', metaKey: true })
  await waitFor(() => expect(dirty).toBe(false))
  expect(new TextDecoder('utf-8', { fatal: true }).decode(saved)).toBe(expected)
})

it('keeps the draft and shows a failed export until a lossless retry succeeds', async () => {
  let saved: Uint8Array | undefined
  let dirty = false
  render(
    <OfficeDocumentSurface
      surface="spreadsheet"
      bytes={new TextEncoder().encode('name,count\n甲,0')}
      fileName="registrations.csv"
      frame="workspace"
      onSave={async (bytes) => {
        saved = bytes
      }}
      onOfficeDirty={(value) => {
        dirty = value
      }}
    />
  )
  const user = userEvent.setup()
  await user.click(await screen.findByRole('button', { name: 'Edit first name' }, { timeout: 10_000 }))
  await user.click(screen.getByRole('button', { name: 'Add worksheet' }))
  fireEvent.keyDown(window, { key: 's', ctrlKey: true })
  expect(await screen.findByRole('alert')).toHaveTextContent('无法保存此文件')
  expect(dirty).toBe(true)
  expect(saved).toBeUndefined()

  await user.click(screen.getByRole('button', { name: 'Keep first worksheet' }))
  fireEvent.keyDown(window, { key: 's', ctrlKey: true })
  await waitFor(() => expect(dirty).toBe(false))
  expect(screen.queryByRole('alert')).not.toBeInTheDocument()
  expect(new TextDecoder('utf-8', { fatal: true }).decode(saved)).toBe('"name","count"\r\n"已编辑","0"')
})
