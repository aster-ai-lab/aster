import {
  Children,
  cloneElement,
  isValidElement,
  useId,
  type ReactElement,
  type ReactNode,
} from 'react'
import { cn } from '../../lib/cn'
import {
  Field,
  FieldDescription,
  FieldError,
  FieldLabel,
} from '../primitives/field'
import { RequiredMark } from '../primitives/label'

export type FormFieldProps = {
  label?: string
  hint?: string
  error?: string
  required?: boolean
  htmlFor?: string
  children: ReactNode
  className?: string
}

/**
 * Console labeled control shell on shared Field primitives.
 * Prefer composing `Field` / `FieldLabel` / `FieldError` for new Studio forms.
 */
export function FormField({
  label,
  hint,
  error,
  required = false,
  htmlFor,
  children,
  className = '',
}: FormFieldProps) {
  const autoId = useId()
  const fieldId = htmlFor ?? autoId
  const hintId = `${fieldId}-hint`
  const errorId = `${fieldId}-error`
  const describedBy = [error ? errorId : null, hint && !error ? hintId : null]
    .filter(Boolean)
    .join(' ')

  const control = Children.map(children, (child) => {
    if (!isValidElement(child)) return child
    const el = child as ReactElement<Record<string, unknown>>
    return cloneElement(el, {
      id: (el.props.id as string | undefined) ?? fieldId,
      'aria-invalid': error ? true : el.props['aria-invalid'],
      'aria-describedby':
        describedBy || (el.props['aria-describedby'] as string | undefined),
    })
  })

  return (
    <Field
      data-invalid={error ? true : undefined}
      className={cn('gap-1.5', className)}
    >
      {label ? (
        <FieldLabel htmlFor={fieldId} className="text-xs font-medium text-ink-muted">
          {label}
          {required ? <RequiredMark /> : null}
        </FieldLabel>
      ) : null}
      {control}
      {error ? (
        <FieldError id={errorId} className="mt-0.5 text-xs text-[color:var(--color-error)]">
          {error}
        </FieldError>
      ) : hint ? (
        <FieldDescription id={hintId} className="mt-0.5 text-xs leading-relaxed text-ink-faint">
          {hint}
        </FieldDescription>
      ) : null}
    </Field>
  )
}
