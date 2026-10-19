import type { CSSProperties } from 'react'

import { cn } from './utils'

/**
 * State a collection row shows while an optimistic change is in flight or has just settled.
 * - `leaving`: removed by the reader; plays the exit before the row leaves the list.
 * - `pending`: changed by the reader; shows the new values until the owner answers.
 * - `updated`: the owner accepted the change; a brief success tint.
 * - `restored`: the owner rejected the change; the row returns with a brief error tint.
 */
export type RowMotion = 'leaving' | 'pending' | 'updated' | 'restored'

/** Rows rise in when they mount: the first load, a page/filter change, or a rollback that brings a row back. */
const ENTER = 'a3s-row-enter motion-reduce:animate-none'
const LEAVE = 'pointer-events-none a3s-row-leave motion-reduce:animate-none motion-reduce:opacity-0'
const SETTLED: Record<Exclude<RowMotion, 'leaving'>, string> = {
  pending: 'opacity-60',
  updated: 'bg-[#effaf3]',
  restored: 'bg-[#fef3f2]'
}

/** The leave motion lasts this long; a removed row stays mounted for it. */
export const ROW_LEAVE_MS = 200
const STAGGER_STEP_MS = 16
const STAGGER_ROWS = 12

/**
 * Class names for a collection row. Merge them last so a tint overrides the row's resting fill;
 * the tint fades back through the colour transition once the motion clears.
 */
export function rowMotionClassName(motion?: RowMotion) {
  return cn(
    'transition-[background-color,opacity] duration-500',
    motion === 'leaving' ? LEAVE : ENTER,
    motion && motion !== 'leaving' && SETTLED[motion]
  )
}

/** Staggers the enter motion of the first rows so a page reads top to bottom; later rows enter together. */
export function rowMotionStyle(index: number, motion?: RowMotion): CSSProperties | undefined {
  if (motion === 'leaving' || index <= 0) return undefined
  return { animationDelay: `${Math.min(index, STAGGER_ROWS) * STAGGER_STEP_MS}ms` }
}
