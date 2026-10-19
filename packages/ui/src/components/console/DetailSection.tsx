import type { ReactNode } from 'react';
import { cn } from '../../lib/cn';

export type DetailSectionProps = {
  title: ReactNode;
  actions?: ReactNode;
  children: ReactNode;
  className?: string;
};

/** Titled block inside DirectorySplit detail (members / children / workspace). */
export function DetailSection({
  title,
  actions,
  children,
  className = '',
}: DetailSectionProps) {
  return (
    <section className={cn('ui-detail-section', className)}>
      <div className="ui-detail-section__head">
        <h3 className="ui-detail-section__title">{title}</h3>
        {actions ? (
          <div className="ui-detail-section__actions">{actions}</div>
        ) : null}
      </div>
      <div className="ui-detail-section__body">{children}</div>
    </section>
  );
}
