import type { CSSProperties } from 'react'

export function paperloomDialogStyle(width: number): CSSProperties {
  return {
    maxWidth: `min(${width}px, calc(100% - var(--sidebar-width, 60px) - 2rem))`,
    left: 'calc(50% + var(--sidebar-width, 60px) / 2)'
  }
}
