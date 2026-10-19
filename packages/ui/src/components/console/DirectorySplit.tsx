import type { ReactNode } from 'react';
import { cn } from '../../lib/cn';

export type DirectorySplitProps = {
  tree: ReactNode;
  detail: ReactNode;
  className?: string;
  treeLabel?: string;
  detailLabel?: string;
};

/**
 * Master–detail shell for enterprise org directories:
 * left tree pane + right detail pane.
 */
export function DirectorySplit({
  tree,
  detail,
  className = '',
  treeLabel,
  detailLabel,
}: DirectorySplitProps) {
  return (
    <div className={cn('ui-directory-split', className)}>
      <aside className="ui-card ui-directory-split__tree" aria-label={treeLabel}>
        {tree}
      </aside>
      <section
        className="ui-card ui-directory-split__detail"
        aria-label={detailLabel}
        aria-live="polite"
      >
        {detail}
      </section>
    </div>
  );
}
