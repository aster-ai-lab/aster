import { useEffect, useRef, useState, type ReactNode } from 'react';
import { useInViewport } from 'ahooks';
import { cn } from '../../lib/cn';

type RevealProps = {
  children: ReactNode;
  className?: string;
  delayMs?: number;
};

export function Reveal({ children, className = '', delayMs = 0 }: RevealProps) {
  const ref = useRef<HTMLDivElement>(null);
  const [inViewport] = useInViewport(ref, {
    rootMargin: '-40px',
    threshold: 0.12,
  });
  const [visible, setVisible] = useState(false);

  useEffect(() => {
    if (!inViewport || visible) return;
    const id = window.setTimeout(() => setVisible(true), delayMs);
    return () => window.clearTimeout(id);
  }, [inViewport, visible, delayMs]);

  return (
    <div
      ref={ref}
      className={cn('reveal', visible && 'is-visible', className)}
    >
      {children}
    </div>
  );
}
