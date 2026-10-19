import type { ElementType, ReactNode } from 'react';
import { cn } from '../../lib/cn';

type CardOwnProps<T extends ElementType> = {
  as?: T;
  className?: string;
  children?: ReactNode;
};

export type CardProps<T extends ElementType = 'div'> = CardOwnProps<T> &
  Omit<React.ComponentPropsWithoutRef<T>, keyof CardOwnProps<T>>;

/**
 * Content card. Chrome is `.ui-card`: elevated surface, 0.5px hairline, 10px radius, no drop shadow.
 * Padding and layout stay on `className`.
 */
export function Card<T extends ElementType = 'div'>({
  as,
  className = '',
  children,
  ...props
}: CardProps<T>) {
  const Comp = as ?? 'div';
  return (
    <Comp className={cn('ui-card', className)} {...(props)}>
      {children}
    </Comp>
  );
}
