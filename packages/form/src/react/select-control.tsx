import {
  Children,
  isValidElement,
  useCallback,
  useEffect,
  useId,
  useLayoutEffect,
  useRef,
  useState,
  type ChangeEvent,
  type CSSProperties,
  type FocusEventHandler,
  type KeyboardEvent,
  type ReactElement,
  type ReactNode,
  type Ref,
} from 'react';

interface PickerOption {
  value: string;
  label: ReactNode;
  text: string;
  disabled: boolean;
  group?: string;
}

export interface SelectControlProps {
  id?: string;
  name?: string;
  className?: string;
  children?: ReactNode;
  value?: string | number | readonly string[];
  disabled?: boolean;
  required?: boolean;
  ref?: Ref<HTMLButtonElement>;
  onChange?: (event: ChangeEvent<HTMLSelectElement>) => void;
  onBlur?: FocusEventHandler<HTMLElement>;
  onFocus?: FocusEventHandler<HTMLElement>;
  'aria-label'?: string;
  'aria-labelledby'?: string;
  'aria-invalid'?: boolean | 'true' | 'false';
  'aria-describedby'?: string;
}

function textOf(node: ReactNode): string {
  if (node === null || node === undefined || typeof node === 'boolean') return '';
  if (typeof node === 'string' || typeof node === 'number') return String(node);
  if (Array.isArray(node)) return node.map(textOf).join('');
  if (isValidElement<{ children?: ReactNode }>(node)) return textOf(node.props.children);
  return '';
}

function collectOptions(children: ReactNode, group?: string, out: PickerOption[] = []): PickerOption[] {
  Children.forEach(children, (child) => {
    if (!isValidElement(child)) return;
    const element = child as ReactElement<{
      value?: string | number;
      label?: string;
      disabled?: boolean;
      children?: ReactNode;
    }>;
    if (element.type === 'optgroup') {
      collectOptions(element.props.children, element.props.label, out);
      return;
    }
    if (element.type !== 'option') return;
    const text = textOf(element.props.children);
    out.push({
      value: String(element.props.value ?? text),
      label: element.props.children,
      text,
      disabled: Boolean(element.props.disabled),
      group,
    });
  });
  return out;
}

function changeEvent(value: string): ChangeEvent<HTMLSelectElement> {
  const target = { value } as HTMLSelectElement;
  return { target, currentTarget: target } as ChangeEvent<HTMLSelectElement>;
}

const GAP = 6;
const PANEL_MAX_HEIGHT = 280;

/**
 * A drawn single-choice picker with the native select call shape: `<option>` / `<optgroup>`
 * children, a string `value`, and `onChange(event.target.value)`. The menu is a styled
 * listbox, never the operating-system dropdown.
 */
export function SelectControl({
  id,
  name,
  className,
  children,
  value,
  disabled,
  required,
  ref,
  onChange,
  onBlur,
  onFocus,
  'aria-label': ariaLabel,
  'aria-labelledby': ariaLabelledBy,
  'aria-invalid': ariaInvalid,
  'aria-describedby': ariaDescribedBy,
}: SelectControlProps) {
  const options = collectOptions(children);
  const current = Array.isArray(value) ? String(value[0] ?? '') : String(value ?? '');
  const selected = options.find((option) => option.value === current);
  const [open, setOpen] = useState(false);
  const [active, setActive] = useState(-1);
  const [placement, setPlacement] = useState<CSSProperties>({});
  const rootRef = useRef<HTMLSpanElement>(null);
  const triggerRef = useRef<HTMLButtonElement | null>(null);
  const listId = `${useId()}-listbox`;
  const optionId = (index: number) => `${listId}-${index}`;

  const setTrigger = useCallback(
    (node: HTMLButtonElement | null) => {
      triggerRef.current = node;
      if (typeof ref === 'function') ref(node);
      else if (ref) (ref as { current: HTMLButtonElement | null }).current = node;
    },
    [ref],
  );

  const place = useCallback(() => {
    const trigger = triggerRef.current;
    if (!trigger) return;
    const rect = trigger.getBoundingClientRect();
    const below = window.innerHeight - rect.bottom - GAP;
    const above = rect.top - GAP;
    const up = below < Math.min(PANEL_MAX_HEIGHT, 160) && above > below;
    setPlacement({
      left: rect.left,
      minWidth: rect.width,
      maxHeight: Math.max(120, Math.min(PANEL_MAX_HEIGHT, (up ? above : below) - 8)),
      ...(up ? { bottom: window.innerHeight - rect.top + GAP } : { top: rect.bottom + GAP }),
    });
  }, []);

  useLayoutEffect(() => {
    if (open) place();
  }, [open, place]);

  useEffect(() => {
    if (!open) return;
    const close = (event: PointerEvent) => {
      if (!rootRef.current?.contains(event.target as Node)) setOpen(false);
    };
    document.addEventListener('pointerdown', close);
    window.addEventListener('resize', place);
    window.addEventListener('scroll', place, true);
    return () => {
      document.removeEventListener('pointerdown', close);
      window.removeEventListener('resize', place);
      window.removeEventListener('scroll', place, true);
    };
  }, [open, place]);

  const openAt = (index: number) => {
    setActive(index);
    setOpen(true);
  };

  const choose = (option: PickerOption) => {
    if (option.disabled) return;
    setOpen(false);
    triggerRef.current?.focus();
    if (option.value !== current) onChange?.(changeEvent(option.value));
  };

  const step = (from: number, delta: number) => {
    for (let index = from + delta; index >= 0 && index < options.length; index += delta) {
      if (!options[index].disabled) return index;
    }
    return from;
  };

  const onKeyDown = (event: KeyboardEvent<HTMLButtonElement>) => {
    const selectedIndex = options.findIndex((option) => option.value === current);
    if (!open) {
      if (['ArrowDown', 'ArrowUp', 'Enter', ' '].includes(event.key)) {
        event.preventDefault();
        openAt(selectedIndex >= 0 ? selectedIndex : step(-1, 1));
      }
      return;
    }
    if (event.key === 'ArrowDown' || event.key === 'ArrowUp') {
      event.preventDefault();
      setActive((index) => step(index, event.key === 'ArrowDown' ? 1 : -1));
    } else if (event.key === 'Home' || event.key === 'End') {
      event.preventDefault();
      setActive(event.key === 'Home' ? step(-1, 1) : step(options.length, -1));
    } else if (event.key === 'Enter' || event.key === ' ') {
      event.preventDefault();
      const option = options[active];
      if (option) choose(option);
    } else if (event.key === 'Escape') {
      event.preventDefault();
      setOpen(false);
    } else if (event.key === 'Tab') {
      setOpen(false);
    }
  };

  const classes = ['select', 'a3s-form-select-trigger', className].filter(Boolean).join(' ');
  let lastGroup: string | undefined;
  return (
    <span ref={rootRef} className="a3s-form-select-control" data-open={open || undefined}>
      <button
        ref={setTrigger}
        id={id}
        type="button"
        role="combobox"
        className={classes}
        disabled={disabled}
        aria-label={ariaLabel}
        aria-labelledby={ariaLabelledBy}
        aria-invalid={ariaInvalid}
        aria-describedby={ariaDescribedBy}
        aria-required={required || undefined}
        aria-haspopup="listbox"
        aria-expanded={open}
        aria-controls={listId}
        aria-activedescendant={open && active >= 0 ? optionId(active) : undefined}
        data-placeholder={selected && selected.value !== '' ? undefined : ''}
        onClick={() => (open ? setOpen(false) : openAt(options.indexOf(selected as PickerOption)))}
        onKeyDown={onKeyDown}
        onBlur={onBlur}
        onFocus={onFocus}
      >
        <span className="a3s-form-select-value">{selected?.label ?? ''}</span>
      </button>
      {name ? <input type="hidden" name={name} value={current} /> : null}
      {open ? (
        <span id={listId} role="listbox" aria-label={ariaLabel} className="a3s-form-select-panel" style={placement}>
          {options.map((option, index) => {
            const heading = option.group !== lastGroup && option.group ? option.group : undefined;
            lastGroup = option.group;
            return (
              <span key={`${option.group ?? ''}\u0000${option.value}`} className="a3s-form-select-row">
                {heading ? (
                  <span className="a3s-form-select-group" aria-hidden="true">
                    {heading}
                  </span>
                ) : null}
                <span
                  id={optionId(index)}
                  role="option"
                  aria-selected={option.value === current}
                  aria-disabled={option.disabled || undefined}
                  data-active={index === active || undefined}
                  className="a3s-form-select-option"
                  onPointerDown={(event) => event.preventDefault()}
                  onPointerEnter={() => !option.disabled && setActive(index)}
                  onClick={() => choose(option)}
                >
                  <span className="a3s-form-select-option-label">{option.label}</span>
                  <svg className="a3s-form-select-check" width="14" height="14" viewBox="0 0 14 14" aria-hidden="true">
                    <path
                      d="m3 7.5 2.5 2.5L11 4.5"
                      fill="none"
                      stroke="currentColor"
                      strokeWidth="1.6"
                      strokeLinecap="round"
                      strokeLinejoin="round"
                    />
                  </svg>
                </span>
              </span>
            );
          })}
        </span>
      ) : null}
    </span>
  );
}
