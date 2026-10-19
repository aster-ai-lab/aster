import {
  Children,
  cloneElement,
  isValidElement,
  useId,
  type ReactElement,
  type ReactNode,
} from 'react';
import { cn } from '../../lib/cn';

export type FormFieldProps = {
  label?: string;
  hint?: string;
  error?: string;
  required?: boolean;
  htmlFor?: string;
  children: ReactNode;
  className?: string;
};

export function FormField({
  label,
  hint,
  error,
  required = false,
  htmlFor,
  children,
  className = '',
}: FormFieldProps) {
  const autoId = useId();
  const fieldId = htmlFor ?? autoId;
  const hintId = `${fieldId}-hint`;
  const errorId = `${fieldId}-error`;
  const describedBy = [
    error ? errorId : null,
    hint && !error ? hintId : null,
  ]
    .filter(Boolean)
    .join(' ');

  const control = Children.map(children, (child) => {
    if (!isValidElement(child)) return child;
    const el = child as ReactElement<Record<string, unknown>>;
    return cloneElement(el, {
      id: (el.props.id as string | undefined) ?? fieldId,
      'aria-invalid': error ? true : el.props['aria-invalid'],
      'aria-describedby':
        describedBy || (el.props['aria-describedby'] as string | undefined),
    });
  });

  return (
    <div className={cn('flex flex-col gap-1.5', className)}>
      {label ? (
        <label htmlFor={fieldId} className="text-xs font-medium text-ink-muted">
          {label}
          {required ? (
            <span className="text-[color:var(--color-error)]" aria-hidden>
              {' '}
              *
            </span>
          ) : null}
        </label>
      ) : null}
      {control}
      {error ? (
        <p
          id={errorId}
          className="mt-0.5 text-xs text-[color:var(--color-error)]"
          role="alert"
        >
          {error}
        </p>
      ) : hint ? (
        <p id={hintId} className="mt-0.5 text-xs leading-relaxed text-ink-faint">
          {hint}
        </p>
      ) : null}
    </div>
  );
}
