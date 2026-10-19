import type { Buffer as NodeBuffer } from 'node:buffer'
import type * as NodeChildren from 'node:child_process'
import type * as NodeFiles from 'node:fs/promises'

import { OFFICE_OPERATION_MS, OFFICE_OUTPUT_LIMIT, runOwnedOfficeProcess } from './documentTools/officeProcess'
import { officeSvgPages, type OfficeSvgPage } from './documentTools/officeSvg'
import { OFFICE_CAPTURE_DPI } from './pdfToolsRaster'

const OFFICE_BIN = '../../../../../../packages/office/target/release/a3s-office'

export type OfficeCapture = { pages: Uint8Array[]; sourcePages: readonly number[]; text: string; dpi: number }

type NodeFs = typeof NodeFiles
type AbortSignal = InstanceType<typeof globalThis.AbortController>['signal']
type Blob = InstanceType<typeof globalThis.Blob>
const { TextDecoder, TextEncoder, performance, btoa, atob, URL, Blob, Image, document } = globalThis

export async function captureOfficePages(
  bytes: Uint8Array,
  name: string,
  signal?: AbortSignal,
  pageRange = ''
): Promise<OfficeCapture> {
  checkAbort(signal)
  if (bytes.byteLength > OFFICE_OUTPUT_LIMIT) throw new Error('unreadable')
  if (!(await nodeFilesAvailable())) return captureOfficePagesInPage(bytes, name, signal, pageRange)
  return withOfficeFile(
    bytes,
    name,
    async (file, run) => {
      const text = await officeText(run, file)
      const svgPath = `${file}.svg`
      await run(['view', file, 'svg', '--output', svgPath, '--json'])
      const svg = new TextDecoder('utf-8', { fatal: true }).decode(await readOfficeOutput(svgPath, signal))
      const pages: Uint8Array[] = []
      const projected = officeSvgPages(svg, pageRange)
      let outputSize = 0
      for (const page of projected) {
        checkAbort(signal)
        const png = await rasterSvg(page.svg)
        outputSize += png.byteLength
        if (outputSize > OFFICE_OUTPUT_LIMIT) throw new Error('office-output-limit')
        pages.push(png)
      }
      checkAbort(signal)
      if (pages.length === 0 || pages.some((png) => png.byteLength < 8 || png[0] !== 137)) throw new Error('office')
      return { pages, sourcePages: projected.map((page) => page.sourcePage), text, dpi: OFFICE_CAPTURE_DPI }
    },
    signal
  )
}

export async function writeOfficeDocx(text: string): Promise<Uint8Array> {
  validateText(text)
  if (!(await nodeFilesAvailable())) return bytesFromHost(await officeHost('docx', { text }))
  return withOfficeFile(new Uint8Array(), 'minutes.docx', async (_file, run, dir) => {
    const output = await join(dir, 'out.docx')
    await run(['create', output, '--json'])
    await run(['set', output, '/body/p[1]', '--text', text, '--output', output, '--json'])
    return readOfficeOutput(output)
  })
}

export async function writeOfficeXlsx(rows: readonly (readonly string[])[]): Promise<Uint8Array> {
  if (rows.length > 500) throw new Error('unreadable')
  let size = 0
  for (const row of rows) {
    if (row.length > 26) throw new Error('unreadable')
    for (const value of row) {
      size += validateText(value)
      if (size > 1_000_000) throw new Error('unreadable')
    }
  }
  if (!(await nodeFilesAvailable()))
    return bytesFromHost(await officeHost('xlsx', { rows: rows.map((row) => [...row]) }))
  return withOfficeFile(new Uint8Array(), 'table.xlsx', async (_file, run, dir) => {
    const output = await join(dir, 'out.xlsx')
    await run(['create', output, '--json'])
    for (const [rowIndex, row] of rows.entries()) {
      for (const [columnIndex, value] of row.entries()) {
        const cell = `/Sheet1/${columnName(columnIndex)}${rowIndex + 1}`
        await run(['set', output, cell, '--text', value, '--output', output, '--json'])
      }
    }
    return readOfficeOutput(output)
  })
}

async function officeText(run: (args: string[]) => Promise<string>, file: string): Promise<string> {
  const stdout = await run(['view', file, 'text', '--json'])
  const parsed = JSON.parse(stdout) as { data?: { result?: { text?: string } } }
  return parsed.data?.result?.text ?? ''
}

async function withOfficeFile<T>(
  bytes: Uint8Array,
  name: string,
  body: (file: string, run: (args: string[]) => Promise<string>, dir: string) => Promise<T>,
  signal?: AbortSignal
): Promise<T> {
  checkAbort(signal)
  const deadline = performance.now() + OFFICE_OPERATION_MS
  const fs = await nodeFs()
  const os = await import(/* @vite-ignore */ 'node:os')
  const dir = await fs.mkdtemp(await join(os.tmpdir(), 'a3s-pdf-tools-'))
  try {
    const file = await join(dir, safeName(name))
    if (bytes.byteLength > 0) await fs.writeFile(file, bytes)
    checkAbort(signal)
    return await body(file, (args) => spawnOffice(args, deadline, signal), dir)
  } finally {
    await fs.rm(dir, { recursive: true, force: true })
  }
}

async function spawnOffice(args: string[], deadline: number, signal?: AbortSignal): Promise<string> {
  checkAbort(signal)
  const [childProcess, bin] = await Promise.all([nodeChild(), officeBin()])
  return runOwnedOfficeProcess(childProcess.spawn, bin, args, deadline, signal)
}

async function nodeFilesAvailable(): Promise<boolean> {
  try {
    // The renderer build replaces this specifier with an empty module.
    // Only a real readFile means this process can spawn a3s-office.
    const fs = (await import(/* @vite-ignore */ 'node:fs/promises')) as { readFile?: unknown }
    return typeof fs.readFile === 'function'
  } catch {
    return false
  }
}

type OfficeHostResult = { svg?: string; textJson?: string; bytesBase64?: string }

async function officeHost(
  action: 'capture' | 'docx' | 'xlsx',
  payload: Record<string, unknown>,
  signal?: AbortSignal
): Promise<OfficeHostResult> {
  checkAbort(signal)
  const invoke = (
    globalThis as typeof globalThis & {
      __TAURI_INTERNALS__?: { invoke?: (command: string, args?: Record<string, unknown>) => Promise<unknown> }
    }
  ).__TAURI_INTERNALS__?.invoke
  if (!invoke) throw new Error('office')
  try {
    const result = (await invoke('office_tool', { action, ...payload })) as OfficeHostResult
    checkAbort(signal)
    return result
  } catch (error) {
    const message = error instanceof Error ? error.message : typeof error === 'string' ? error : ''
    checkAbort(signal)
    throw new Error(['unreadable', 'office-timeout', 'office-output-limit'].includes(message) ? message : 'office')
  }
}

async function captureOfficePagesInPage(
  bytes: Uint8Array,
  name: string,
  signal?: AbortSignal,
  pageRange = ''
): Promise<OfficeCapture> {
  const result = await officeHost('capture', { name, bytesBase64: bytesToBase64(bytes) }, signal)
  const svg = result.svg ?? ''
  if (!svg) throw new Error('office')
  const pages: Uint8Array[] = []
  const projected = officeSvgPages(svg, pageRange)
  let outputSize = 0
  for (const page of projected) {
    checkAbort(signal)
    const png = await rasterSvgInPage(page)
    outputSize += png.byteLength
    if (outputSize > OFFICE_OUTPUT_LIMIT) throw new Error('office-output-limit')
    pages.push(png)
  }
  checkAbort(signal)
  if (pages.length === 0 || pages.some((png) => png.byteLength < 8 || png[0] !== 137)) throw new Error('office')
  return {
    pages,
    sourcePages: projected.map((page) => page.sourcePage),
    text: officeTextFrom(result.textJson ?? ''),
    dpi: OFFICE_CAPTURE_DPI
  }
}

function checkAbort(signal?: AbortSignal): void {
  if (signal?.aborted) throw new Error('abort')
}

function validateText(text: string): number {
  if (text.length > 1_000_000 || text.includes('\0')) throw new Error('unreadable')
  const size = new TextEncoder().encode(text).byteLength
  if (size > 1_000_000) throw new Error('unreadable')
  return size
}

async function readOfficeOutput(path: string, signal?: AbortSignal): Promise<Uint8Array> {
  const fs = await nodeFs()
  const file = await fs.open(path, 'r')
  try {
    const chunks: Uint8Array[] = []
    let size = 0
    for (;;) {
      checkAbort(signal)
      const chunk = new Uint8Array(Math.min(65536, OFFICE_OUTPUT_LIMIT + 1 - size))
      const { bytesRead } = await file.read(chunk, 0, chunk.length, null)
      if (bytesRead === 0) break
      size += bytesRead
      if (size > OFFICE_OUTPUT_LIMIT) throw new Error('office-output-limit')
      chunks.push(chunk.subarray(0, bytesRead))
    }
    const result = new Uint8Array(size)
    let offset = 0
    for (const chunk of chunks) {
      result.set(chunk, offset)
      offset += chunk.length
    }
    return result
  } finally {
    await file.close()
  }
}

function officeTextFrom(stdout: string): string {
  if (!stdout.trim()) return ''
  const parsed = JSON.parse(stdout) as { data?: { result?: { text?: string } } }
  return parsed.data?.result?.text ?? ''
}

async function bytesFromHost(result: OfficeHostResult): Promise<Uint8Array> {
  const bytes = base64ToBytes(result.bytesBase64 ?? '')
  if (bytes.byteLength < 4 || bytes[0] !== 80 || bytes[1] !== 75) throw new Error('office')
  return bytes
}

function bytesToBase64(bytes: Uint8Array): string {
  let binary = ''
  for (let index = 0; index < bytes.length; index += 8192) {
    binary += String.fromCharCode(...bytes.subarray(index, index + 8192))
  }
  return btoa(binary)
}

function base64ToBytes(value: string): Uint8Array {
  if (!value) return new Uint8Array()
  const binary = atob(value)
  const bytes = new Uint8Array(binary.length)
  for (let index = 0; index < binary.length; index += 1) bytes[index] = binary.charCodeAt(index)
  return bytes
}

async function rasterSvgInPage(page: OfficeSvgPage): Promise<Uint8Array> {
  const { width, height } = page
  const sized = page.svg
  const url = URL.createObjectURL(new Blob([sized], { type: 'image/svg+xml;charset=utf-8' }))
  try {
    const image = new Image()
    image.src = url
    await image.decode()
    const canvas = document.createElement('canvas')
    canvas.width = width
    canvas.height = height
    const context = canvas.getContext('2d')
    if (!context) throw new Error('office')
    context.drawImage(image, 0, 0, width, height)
    const png = await new Promise<Blob | null>((resolve) => canvas.toBlob((value) => resolve(value), 'image/png'))
    if (!png) throw new Error('office')
    return new Uint8Array(await png.arrayBuffer())
  } finally {
    URL.revokeObjectURL(url)
  }
}

async function nodeFs(): Promise<NodeFs> {
  return import(/* @vite-ignore */ 'node:fs/promises')
}

async function nodeChild(): Promise<typeof NodeChildren> {
  return import(/* @vite-ignore */ 'node:child_process')
}

async function officeBin(): Promise<string> {
  const { fileURLToPath } = await import(/* @vite-ignore */ 'node:url')
  const { dirname, join } = await import(/* @vite-ignore */ 'node:path')
  return join(dirname(fileURLToPath(import.meta.url)), OFFICE_BIN)
}

async function join(...parts: string[]): Promise<string> {
  const path = await import(/* @vite-ignore */ 'node:path')
  return path.join(...parts)
}

function safeName(name: string): string {
  const base = name.split(/[/\\]/).pop() || 'file.bin'
  return base.replace(/[^\w.-]+/g, '_')
}

function columnName(index: number): string {
  return String.fromCharCode('A'.charCodeAt(0) + index)
}

async function rasterSvg(svg: string): Promise<Uint8Array> {
  const { Buffer } = await import(/* @vite-ignore */ 'node:buffer')
  const sharpModule = (await import(/* @vite-ignore */ 'sharp')) as {
    default: (
      input: NodeBuffer,
      options?: { density?: number }
    ) => { png: () => { toBuffer: () => Promise<Uint8Array> } }
  }
  return new Uint8Array(await sharpModule.default(Buffer.from(svg)).png().toBuffer())
}
