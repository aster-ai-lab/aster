import { useEffect, useState } from 'react'

import { readStudioRuntime } from '@shared/studioRuntime'

import type { CapabilityEntry } from '../capabilities/capabilitiesData'
import { CapabilityMark } from '../capabilities/capabilityMark'

const mediaObjectUrls = new Map<string, string>()

function usePluginMedia(path: string | undefined): string | null | undefined {
  const [src, setSrc] = useState<string | null | undefined>(path ? (mediaObjectUrls.get(path) ?? undefined) : null)
  useEffect(() => {
    if (!path) return
    const cached = mediaObjectUrls.get(path)
    if (cached) {
      setSrc(cached)
      return
    }
    const controller = new AbortController()
    let cancelled = false
    void loadPluginMedia(path, controller.signal).then((url) => {
      if (cancelled) return
      if (url) mediaObjectUrls.set(path, url)
      setSrc(url)
    })
    return () => {
      cancelled = true
      controller.abort()
    }
  }, [path])
  return path ? src : null
}

async function loadPluginMedia(path: string, signal: AbortSignal): Promise<string | null> {
  try {
    const runtime = readStudioRuntime()
    if (!runtime) return null
    const response = await fetch(`${runtime.apiBaseUrl}${path}`, {
      headers: { authorization: `Bearer ${runtime.token}` },
      signal,
      cache: 'force-cache'
    })
    if (!response.ok) return null
    return URL.createObjectURL(await response.blob())
  } catch {
    return null
  }
}

/** The 132px card artwork. A plugin cover fills it. Otherwise the logo sits on the light field. */
export function PluginCardArt({ entry }: { entry: CapabilityEntry }) {
  const cover = usePluginMedia(entry.coverUrl)
  if (cover) {
    return <img src={cover} alt="" className="size-full object-cover" />
  }
  if (entry.coverUrl && cover === undefined) {
    return <div className="size-full bg-[#f4f5f7]" />
  }
  return (
    <div className="flex size-full items-center gap-3 bg-[#f4f5f7] px-4">
      <CapabilityMark entry={entry} />
      <span aria-hidden className="min-w-0 truncate text-[16px] font-medium text-[#1a1a1a]">
        {entry.name}
      </span>
    </div>
  )
}
