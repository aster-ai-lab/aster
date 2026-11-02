/**
 * Internal utilities for UI components.
 *
 * This module is for INTERNAL use only and should NOT be exposed to external consumers.
 * External utilities should be placed in `utils/` instead.
 *
 * @internal
 */

import { type ClassValue, clsx } from 'clsx'
import { twMerge } from 'tailwind-merge'

// Arc module CSS resolves colors/radii/motion from these tokens; every arc-skinned primitive imports cn, so load them here.
import '../components/arc/foundation.css'

/**
 * Merges Tailwind CSS class names with conflict resolution.
 * Combines clsx for conditional classes and tailwind-merge for deduplication.
 *
 * @example
 * cn('px-2 py-1', 'px-4') // => 'py-1 px-4'
 * cn('text-red-500', isActive && 'text-blue-500')
 */
export function cn(...inputs: ClassValue[]) {
  return twMerge(clsx(inputs))
}
