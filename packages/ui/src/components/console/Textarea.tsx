import type { TextareaHTMLAttributes } from 'react';
import { cn } from '../../lib/cn';

/** Fill control — soft gray fill, no outline stroke by default. */
export function Textarea({
  className = '',
  ...props
}: TextareaHTMLAttributes<HTMLTextAreaElement>) {
  return (
    <textarea
      className={cn(
        'min-h-[5.5rem] w-full resize-y rounded-lg border border-transparent bg-[color:var(--color-control-fill)] px-3 py-2 text-[13px] font-normal leading-5 text-ink outline-none transition placeholder:text-ink-faint',
        'focus-visible:shadow-[var(--ring-focus)]',
        className,
      )}
      {...props}
    />
  );
}
