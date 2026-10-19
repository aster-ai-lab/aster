import { useEffect, useState } from 'react'

import { capabilityIconObjectUrl } from '@renderer/services/capabilityIcon'

const iconObjectUrls = new Map<string, string>()

/** A loadable address for an authenticated catalog icon path, or null until it loads or when it fails. */
export function useCatalogIconSrc(iconPath: string | undefined): string | null {
  const [src, setSrc] = useState<string | null>(iconPath ? (iconObjectUrls.get(iconPath) ?? null) : null)
  useEffect(() => {
    if (!iconPath) return
    const cached = iconObjectUrls.get(iconPath)
    if (cached) {
      setSrc(cached)
      return
    }
    const controller = new AbortController()
    let cancelled = false
    void capabilityIconObjectUrl(iconPath, controller.signal).then((url) => {
      if (cancelled || !url) return
      iconObjectUrls.set(iconPath, url)
      setSrc(url)
    })
    return () => {
      cancelled = true
      controller.abort()
    }
  }, [iconPath])
  return iconPath ? src : null
}
