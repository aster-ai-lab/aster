import { useTranslation } from 'react-i18next'

import { formatCompactDiffCount } from './diffStat'

/** Aligned +/− counts for a file row. Hidden when the turn did not add or remove a line. */
export function DiffStatLabel({ additions, deletions }: { additions: number; deletions: number }) {
  const { t } = useTranslation()
  if (additions === 0 && deletions === 0) return null
  return (
    <span
      role="group"
      aria-label={t('workbench.code.diff_stat', {
        additions,
        deletions,
        defaultValue: '{{additions}} additions, {{deletions}} deletions'
      })}
      className="inline-grid grid-cols-[4ch_4ch] gap-2 text-right font-mono text-[12px] tabular-nums">
      <span aria-hidden="true" className="text-[#16845b]">
        +{formatCompactDiffCount(additions)}
      </span>
      <span aria-hidden="true" className="text-[#c93d45]">
        −{formatCompactDiffCount(deletions)}
      </span>
    </span>
  )
}
