const DECODE_CHUNK_BYTES = 256 * 1024

function yieldToMain(): Promise<void> {
  const scheduler = (globalThis as { scheduler?: { yield?: () => Promise<void> } }).scheduler
  if (typeof scheduler?.yield === 'function') return scheduler.yield()
  return new Promise((resolve) => {
    setTimeout(resolve, 0)
  })
}

/** Decode UTF-8 without holding the window for the whole buffer. */
export async function decodeUtf8Async(bytes: Uint8Array, chunkBytes = DECODE_CHUNK_BYTES): Promise<string | null> {
  const decoder = new TextDecoder('utf-8', { fatal: true })
  if (bytes.byteLength <= chunkBytes) {
    try {
      return decoder.decode(bytes)
    } catch {
      return null
    }
  }
  const parts: string[] = []
  try {
    for (let offset = 0; offset < bytes.byteLength; offset += chunkBytes) {
      const end = Math.min(bytes.byteLength, offset + chunkBytes)
      parts.push(decoder.decode(bytes.subarray(offset, end), { stream: end < bytes.byteLength }))
      if (end < bytes.byteLength) await yieldToMain()
    }
    return parts.join('')
  } catch {
    return null
  }
}
