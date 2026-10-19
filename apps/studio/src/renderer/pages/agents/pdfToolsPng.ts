/** RGBA raster and a PNG codec. Node and the window share this path. */

import { validateRasterSize } from './documentTools/rasterLimits'

export type Raster = { width: number; height: number; rgba: Uint8Array }

const PNG = [137, 80, 78, 71, 13, 10, 26, 10]

export async function encodePng(raster: Raster): Promise<Uint8Array> {
  validateRasterSize(raster.width, raster.height)
  if (raster.width <= 0 || raster.height <= 0) throw new Error('empty')
  if (raster.rgba.length !== raster.width * raster.height * 4) throw new Error('rgba')
  const raw = new Uint8Array((raster.width * 4 + 1) * raster.height)
  for (let y = 0; y < raster.height; y++) {
    const row = y * (raster.width * 4 + 1)
    raw[row] = 0
    raw.set(raster.rgba.subarray(y * raster.width * 4, (y + 1) * raster.width * 4), row + 1)
  }
  const idat = await deflate(raw)
  const ihdr = new Uint8Array(13)
  const view = new DataView(ihdr.buffer)
  view.setUint32(0, raster.width)
  view.setUint32(4, raster.height)
  ihdr[8] = 8
  ihdr[9] = 6
  const chunks = [chunk('IHDR', ihdr), chunk('IDAT', idat), chunk('IEND', new Uint8Array())]
  const size = PNG.length + chunks.reduce((sum, item) => sum + item.length, 0)
  const out = new Uint8Array(size)
  out.set(PNG, 0)
  let offset = PNG.length
  for (const item of chunks) {
    out.set(item, offset)
    offset += item.length
  }
  return out
}

export async function decodePng(bytes: Uint8Array): Promise<Raster> {
  if (bytes.length < 8 || !PNG.every((byte, index) => bytes[index] === byte)) throw new Error('png')
  let width = 0
  let height = 0
  let color = 0
  let depth = 0
  const idat: Uint8Array[] = []
  let offset = 8
  let ended = false
  while (offset + 12 <= bytes.length) {
    const length = new DataView(bytes.buffer, bytes.byteOffset + offset, 4).getUint32(0)
    if (offset + 12 + length > bytes.length) throw new Error('png')
    const type = String.fromCharCode(bytes[offset + 4], bytes[offset + 5], bytes[offset + 6], bytes[offset + 7])
    const data = bytes.subarray(offset + 8, offset + 8 + length)
    if (type === 'IHDR') {
      if (length !== 13 || width !== 0 || height !== 0) throw new Error('png')
      const view = new DataView(data.buffer, data.byteOffset, data.byteLength)
      width = view.getUint32(0)
      height = view.getUint32(4)
      validateRasterSize(width, height)
      depth = data[8]
      color = data[9]
      if (data[10] !== 0 || data[11] !== 0 || data[12] !== 0) throw new Error('png')
    } else if (type === 'IDAT') {
      idat.push(data)
    } else if (type === 'IEND') {
      if (length !== 0) throw new Error('png')
      ended = true
      break
    }
    offset += 12 + length
  }
  if (!ended || idat.length === 0 || depth !== 8 || (color !== 6 && color !== 2) || width <= 0 || height <= 0)
    throw new Error('png')
  const channels = color === 6 ? 4 : 3
  const stride = width * channels
  const inflated = await inflate(concat(idat), (stride + 1) * height)
  if (inflated.byteLength !== (stride + 1) * height) throw new Error('png')
  const rgba = new Uint8Array(width * height * 4)
  let source = 0
  let previous = new Uint8Array(stride)
  for (let y = 0; y < height; y++) {
    const filter = inflated[source]
    source += 1
    const row = inflated.subarray(source, source + stride)
    source += stride
    const current = new Uint8Array(stride)
    for (let index = 0; index < stride; index++) {
      const left = index >= channels ? current[index - channels] : 0
      const up = previous[index]
      const upLeft = index >= channels ? previous[index - channels] : 0
      const value = row[index]
      if (filter === 0) current[index] = value
      else if (filter === 1) current[index] = (value + left) & 255
      else if (filter === 2) current[index] = (value + up) & 255
      else if (filter === 3) current[index] = (value + Math.floor((left + up) / 2)) & 255
      else if (filter === 4) current[index] = (value + paeth(left, up, upLeft)) & 255
      else throw new Error('png')
    }
    previous = current
    for (let x = 0; x < width; x++) {
      const at = (y * width + x) * 4
      const from = x * channels
      rgba[at] = current[from]
      rgba[at + 1] = current[from + 1]
      rgba[at + 2] = current[from + 2]
      rgba[at + 3] = channels === 4 ? current[from + 3] : 255
    }
  }
  return { width, height, rgba }
}

export function solid(width: number, height: number, pixel: readonly [number, number, number, number]): Raster {
  const rgba = new Uint8Array(width * height * 4)
  for (let index = 0; index < rgba.length; index += 4) {
    rgba[index] = pixel[0]
    rgba[index + 1] = pixel[1]
    rgba[index + 2] = pixel[2]
    rgba[index + 3] = pixel[3]
  }
  return { width, height, rgba }
}

function paeth(left: number, up: number, upLeft: number): number {
  const estimate = left + up - upLeft
  const leftDistance = Math.abs(estimate - left)
  const upDistance = Math.abs(estimate - up)
  const diagonal = Math.abs(estimate - upLeft)
  if (leftDistance <= upDistance && leftDistance <= diagonal) return left
  if (upDistance <= diagonal) return up
  return upLeft
}

function chunk(type: string, data: Uint8Array): Uint8Array {
  const out = new Uint8Array(12 + data.length)
  const view = new DataView(out.buffer)
  view.setUint32(0, data.length)
  out[4] = type.charCodeAt(0)
  out[5] = type.charCodeAt(1)
  out[6] = type.charCodeAt(2)
  out[7] = type.charCodeAt(3)
  out.set(data, 8)
  view.setUint32(8 + data.length, crc32(out.subarray(4, 8 + data.length)))
  return out
}

let crcTable: Uint32Array | null = null

function crc32(bytes: Uint8Array): number {
  if (!crcTable) {
    crcTable = new Uint32Array(256)
    for (let index = 0; index < 256; index++) {
      let value = index
      for (let bit = 0; bit < 8; bit++) value = value & 1 ? 0xedb88320 ^ (value >>> 1) : value >>> 1
      crcTable[index] = value
    }
  }
  let crc = 0xffffffff
  for (const byte of bytes) crc = crcTable[(crc ^ byte) & 255] ^ (crc >>> 8)
  return (crc ^ 0xffffffff) >>> 0
}

function concat(parts: Uint8Array[]): Uint8Array {
  const size = parts.reduce((sum, part) => sum + part.length, 0)
  const out = new Uint8Array(size)
  let offset = 0
  for (const part of parts) {
    out.set(part, offset)
    offset += part.length
  }
  return out
}

async function deflate(raw: Uint8Array): Promise<Uint8Array> {
  return pipe(raw, new CompressionStream('deflate'))
}

async function inflate(bytes: Uint8Array, limit: number): Promise<Uint8Array> {
  return pipe(bytes, new DecompressionStream('deflate'), limit)
}

async function pipe(
  bytes: Uint8Array,
  stream: CompressionStream | DecompressionStream,
  limit = Number.POSITIVE_INFINITY
): Promise<Uint8Array> {
  // Read while writing. Awaiting the write first deadlocks once the chunk exceeds the stream watermark.
  const reader = stream.readable.getReader()
  const output = (async () => {
    const chunks: Uint8Array[] = []
    let size = 0
    try {
      for (;;) {
        const item = await reader.read()
        if (item.done) return concat(chunks)
        size += item.value.byteLength
        if (size > limit) throw new Error('raster-limit')
        chunks.push(item.value)
      }
    } catch (error) {
      await reader.cancel(error).catch(() => undefined)
      throw error
    } finally {
      reader.releaseLock()
    }
  })()
  const writer = stream.writable.getWriter()
  const writing = (async () => {
    try {
      await writer.write(Uint8Array.from(bytes))
      await writer.close()
    } finally {
      writer.releaseLock()
    }
  })()
  const [result] = await Promise.all([output, writing])
  return result
}
