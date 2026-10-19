/**
 * Floating menu card.
 *
 * Studio does not load `styles/components.css`, so a menu that only carries
 * BEM hooks renders as bare text. These utilities are the DESIGN.md card:
 * 12px radius, hairline, elevated surface, 13px ink, and the elevated shadow.
 * Tokens follow the theme bridge, including dark mode.
 */

export const menuCardClass =
  'z-[240] rounded-[12px] border border-[color:var(--color-border)] bg-[var(--color-bg-elevated)] text-[13px] text-[var(--color-ink)] shadow-[var(--shadow-elevated)] [-webkit-app-region:no-drag]'

export const menuRowClass =
  'rounded-[8px] text-[13px] text-[var(--color-ink-muted)] hover:bg-[var(--color-surface-2)] hover:text-[var(--color-ink)] focus:bg-[var(--color-surface-2)] focus:text-[var(--color-ink)]'

export const menuRowSelectedClass =
  'bg-[var(--color-surface)] font-medium text-[var(--color-ink)]'
