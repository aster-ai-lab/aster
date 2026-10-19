import { useEffect, useId, useMemo, useRef, useState } from 'react';
import { createPortal } from 'react-dom';
import { cn } from '../../lib/cn';
import { Calendar } from './calendar';
import { addMonths, parseDayKey, startOfMonth, toDayKey } from '../../lib/date';

export type DateRangeValue = {
  start: string; // YYYY-MM-DD or YYYY-MM-DDTHH:mm
  end: string;
};

export type DateRangePickerProps = {
  value: DateRangeValue;
  onChange: (value: DateRangeValue) => void;
  className?: string;
  startLabel?: string;
  endLabel?: string;
  /** Shown in the trigger when start/end are empty. */
  placeholder?: string;
  /** date = day only · datetime = day + hour/minute */
  mode?: 'date' | 'datetime';
  /** popover under trigger · modal centered dialog */
  variant?: 'popover' | 'modal';
  /** Controlled open state (especially for modal) */
  open?: boolean;
  onOpenChange?: (open: boolean) => void;
  title?: string;
  cancelLabel?: string;
  weekStartsOn?: 0 | 1;
  weekdayLabels?: string[];
  monthFormatter?: (month: Date) => string;
  confirmLabel?: string;
  preset30dLabel?: string;
  /**
   * Render the range trigger control. Set false when an external control
   * opens the picker (controlled `open`). Default true.
   */
  showTrigger?: boolean;
  'aria-label'?: string;
};

function splitValue(raw: string): { day: string; hour: string; minute: string } {
  if (!raw) return { day: '', hour: '00', minute: '00' };
  if (raw.includes('T')) {
    const [day, time = '00:00'] = raw.split('T');
    const [hour = '00', minute = '00'] = time.split(':');
    return {
      day: day ?? '',
      hour: hour.padStart(2, '0'),
      minute: minute.padStart(2, '0'),
    };
  }
  return { day: raw, hour: '00', minute: '00' };
}

function joinValue(
  day: string,
  hour: string,
  minute: string,
  mode: 'date' | 'datetime',
) {
  if (!day) return '';
  if (mode === 'date') return day;
  return `${day}T${hour.padStart(2, '0')}:${minute.padStart(2, '0')}`;
}

function clampRange(next: DateRangeValue): DateRangeValue {
  if (!next.start || !next.end) return next;
  if (next.start <= next.end) return next;
  return { start: next.end, end: next.start };
}

function formatSummary(value: DateRangeValue, mode: 'date' | 'datetime') {
  if (!value.start || !value.end) return '—';
  if (mode === 'date') {
    const a = splitValue(value.start).day;
    const b = splitValue(value.end).day;
    return `${a} → ${b}`;
  }
  return `${value.start.replace('T', ' ')} → ${value.end.replace('T', ' ')}`;
}

const HOURS = Array.from({ length: 24 }, (_, i) => String(i).padStart(2, '0'));
const MINUTES = ['00', '15', '30', '45'];

function TimeSelect({
  label,
  hour,
  minute,
  onChange,
}: {
  label: string;
  hour: string;
  minute: string;
  onChange: (hour: string, minute: string) => void;
}) {
  return (
    <div className="ui-date-range__time">
      <span>{label}</span>
      <div className="ui-date-range__time-fields">
        <div
          className="ui-date-range__time-col"
          role="listbox"
          aria-label={`${label} hour`}
        >
          {HOURS.map((h) => (
            <button
              key={h}
              type="button"
              role="option"
              aria-selected={h === hour}
              className={cn(
                'ui-date-range__time-opt',
                h === hour && 'is-selected',
              )}
              onClick={() => onChange(h, minute)}
            >
              {h}
            </button>
          ))}
        </div>
        <div
          className="ui-date-range__time-col"
          role="listbox"
          aria-label={`${label} minute`}
        >
          {MINUTES.map((m) => (
            <button
              key={m}
              type="button"
              role="option"
              aria-selected={m === minute}
              className={cn(
                'ui-date-range__time-opt',
                m === minute && 'is-selected',
              )}
              onClick={() => onChange(hour, m)}
            >
              {m}
            </button>
          ))}
        </div>
      </div>
    </div>
  );
}

export function DateRangePicker({
  value,
  onChange,
  className = '',
  startLabel = 'Start',
  endLabel = 'End',
  placeholder = 'Select range',
  mode = 'date',
  variant = 'popover',
  open: openProp,
  onOpenChange,
  title,
  cancelLabel = 'Cancel',
  weekStartsOn = 0,
  weekdayLabels,
  monthFormatter,
  confirmLabel = 'OK',
  preset30dLabel = '30d',
  showTrigger = true,
  'aria-label': ariaLabel,
}: DateRangePickerProps) {
  const rootRef = useRef<HTMLDivElement>(null);
  const dialogRef = useRef<HTMLDivElement>(null);
  const panelId = useId();
  const titleId = useId();
  const [uncontrolledOpen, setUncontrolledOpen] = useState(false);
  const open = openProp ?? uncontrolledOpen;

  function setOpen(next: boolean) {
    onOpenChange?.(next);
    if (openProp === undefined) setUncontrolledOpen(next);
  }

  const isModal = variant === 'modal';
  const [draft, setDraft] = useState(value);
  const active = isModal ? draft : value;

  const startParts = splitValue(active.start);
  const endParts = splitValue(active.end);

  const initialMonth = startParts.day
    ? startOfMonth(parseDayKey(startParts.day))
    : startOfMonth(new Date());
  const [viewMonth, setViewMonth] = useState(initialMonth);
  const [hoverDay, setHoverDay] = useState<string | null>(null);
  const [picking, setPicking] = useState<'start' | 'end'>(
    active.start && !active.end ? 'end' : 'start',
  );

  useEffect(() => {
    if (!open) return;
    setDraft(value);
    setViewMonth(
      splitValue(value.start).day
        ? startOfMonth(parseDayKey(splitValue(value.start).day))
        : startOfMonth(new Date()),
    );
    setPicking(value.start && !value.end ? 'end' : 'start');
    setHoverDay(null);
  }, [open, value]);

  useEffect(() => {
    if (!open || isModal) return;
    const onDoc = (e: MouseEvent) => {
      if (!rootRef.current?.contains(e.target as Node)) setOpen(false);
    };
    document.addEventListener('mousedown', onDoc);
    return () => document.removeEventListener('mousedown', onDoc);
  }, [open, isModal]);

  useEffect(() => {
    if (!open || !isModal) return;
    const previouslyFocused = document.activeElement as HTMLElement | null;
    const frame = window.requestAnimationFrame(() => {
      dialogRef.current
        ?.querySelector<HTMLElement>('button, [href], input')
        ?.focus();
    });
    const onKeyDown = (event: KeyboardEvent) => {
      if (event.key !== 'Escape') return;
      event.preventDefault();
      event.stopImmediatePropagation();
      setOpen(false);
    };
    document.addEventListener('keydown', onKeyDown, true);
    document.body.classList.add('ui-modal-open');
    return () => {
      window.cancelAnimationFrame(frame);
      document.removeEventListener('keydown', onKeyDown, true);
      document.body.classList.remove('ui-modal-open');
      previouslyFocused?.focus?.();
    };
  }, [open, isModal]);

  const rangePreview = useMemo(() => {
    const start = startParts.day;
    const end = endParts.day;
    if (start && end) return { start, end };
    if (start && hoverDay) {
      return start <= hoverDay
        ? { start, end: hoverDay }
        : { start: hoverDay, end: start };
    }
    return { start, end };
  }, [startParts.day, endParts.day, hoverDay]);

  function emit(next: DateRangeValue) {
    const clamped = clampRange(next);
    if (isModal) setDraft(clamped);
    else onChange(clamped);
  }

  function onDayClick(dayKey: string) {
    if (picking === 'start' || (startParts.day && endParts.day)) {
      emit({
        start: joinValue(dayKey, startParts.hour, startParts.minute, mode),
        end: '',
      });
      setPicking('end');
      return;
    }
    const startDay = startParts.day;
    if (!startDay) {
      emit({
        start: joinValue(dayKey, startParts.hour, startParts.minute, mode),
        end: '',
      });
      setPicking('end');
      return;
    }
    if (dayKey < startDay) {
      emit({
        start: joinValue(dayKey, endParts.hour, endParts.minute, mode),
        end: joinValue(startDay, startParts.hour, startParts.minute, mode),
      });
    } else {
      emit({
        start: active.start,
        end: joinValue(dayKey, endParts.hour, endParts.minute, mode),
      });
    }
    setPicking('start');
  }

  function patchTime(which: 'start' | 'end', hour: string, minute: string) {
    const day =
      which === 'start'
        ? startParts.day || endParts.day
        : endParts.day || startParts.day;
    if (!day) return;
    if (which === 'start') {
      emit({
        start: joinValue(day, hour, minute, mode),
        end: active.end,
      });
    } else {
      emit({
        start: active.start,
        end: joinValue(day, hour, minute, mode),
      });
    }
  }

  function confirm() {
    if (!active.start || !active.end) return;
    onChange(clampRange(active));
    setOpen(false);
  }

  function cancel() {
    setOpen(false);
  }

  const formatMonth =
    monthFormatter ??
    ((d: Date) =>
      d.toLocaleString(undefined, { month: 'short', year: 'numeric' }));

  const secondMonth = addMonths(viewMonth, 1);
  const canConfirm = Boolean(active.start && active.end);

  const panel = (
    <div
      id={panelId}
      ref={isModal ? dialogRef : undefined}
      className={cn(
        'ui-date-range__panel',
        isModal && 'ui-date-range__panel--modal',
      )}
      role="dialog"
      aria-modal={isModal || undefined}
      aria-labelledby={isModal && title ? titleId : undefined}
      aria-label={!title ? ariaLabel : undefined}
    >
      {isModal && title ? (
        <div className="ui-date-range__dialog-head">
          <h2 id={titleId}>{title}</h2>
          <button
            type="button"
            className="ui-date-range__dialog-close"
            aria-label={cancelLabel}
            onClick={cancel}
          >
            ×
          </button>
        </div>
      ) : null}

      <div className="ui-date-range__months">
        <div className="ui-date-range__month">
          <div className="ui-date-range__month-head">
            <button
              type="button"
              className="ui-date-range__nav"
              aria-label="Previous month"
              onClick={() => setViewMonth((m) => addMonths(m, -1))}
            >
              ‹
            </button>
            <span>{formatMonth(viewMonth)}</span>
            <span className="ui-date-range__nav-spacer" />
          </div>
          <Calendar
            month={viewMonth}
            weekStartsOn={weekStartsOn}
            weekdayLabels={weekdayLabels}
            isRangeStart={(k) => k === rangePreview.start}
            isRangeEnd={(k) =>
              Boolean(rangePreview.end) && k === rangePreview.end
            }
            isInRange={(k) =>
              Boolean(rangePreview.start) &&
              Boolean(rangePreview.end) &&
              k >= rangePreview.start &&
              k <= rangePreview.end
            }
            isSelected={(k) =>
              k === rangePreview.start || k === rangePreview.end
            }
            onDayClick={onDayClick}
            onDayHover={setHoverDay}
          />
        </div>
        <div className="ui-date-range__month">
          <div className="ui-date-range__month-head">
            <span className="ui-date-range__nav-spacer" />
            <span>{formatMonth(secondMonth)}</span>
            <button
              type="button"
              className="ui-date-range__nav"
              aria-label="Next month"
              onClick={() => setViewMonth((m) => addMonths(m, 1))}
            >
              ›
            </button>
          </div>
          <Calendar
            month={secondMonth}
            weekStartsOn={weekStartsOn}
            weekdayLabels={weekdayLabels}
            isRangeStart={(k) => k === rangePreview.start}
            isRangeEnd={(k) =>
              Boolean(rangePreview.end) && k === rangePreview.end
            }
            isInRange={(k) =>
              Boolean(rangePreview.start) &&
              Boolean(rangePreview.end) &&
              k >= rangePreview.start &&
              k <= rangePreview.end
            }
            isSelected={(k) =>
              k === rangePreview.start || k === rangePreview.end
            }
            onDayClick={onDayClick}
            onDayHover={setHoverDay}
          />
        </div>
      </div>

      {mode === 'datetime' ? (
        <div className="ui-date-range__times">
          <TimeSelect
            label={startLabel}
            hour={startParts.hour}
            minute={
              MINUTES.includes(startParts.minute) ? startParts.minute : '00'
            }
            onChange={(h, m) => patchTime('start', h, m)}
          />
          <TimeSelect
            label={endLabel}
            hour={endParts.hour}
            minute={
              MINUTES.includes(endParts.minute) ? endParts.minute : '00'
            }
            onChange={(h, m) => patchTime('end', h, m)}
          />
        </div>
      ) : null}

      <div className="ui-date-range__footer">
        <button
          type="button"
          className="ui-date-range__action"
          onClick={() => {
            const end = new Date();
            const start = new Date(end);
            start.setDate(end.getDate() - 29);
            emit({
              start: joinValue(toDayKey(start), '00', '00', mode),
              end: joinValue(toDayKey(end), '23', '45', mode),
            });
            setPicking('start');
          }}
        >
          {preset30dLabel}
        </button>
        <div className="ui-date-range__footer-actions">
          {isModal ? (
            <button
              type="button"
              className="ui-date-range__action"
              onClick={cancel}
            >
              {cancelLabel}
            </button>
          ) : null}
          <button
            type="button"
            className="ui-date-range__action is-primary"
            disabled={!canConfirm}
            onClick={confirm}
          >
            {confirmLabel}
          </button>
        </div>
      </div>
    </div>
  );

  const summary = formatSummary(value, mode);
  const triggerLabel = summary === '—' ? placeholder : summary;

  const trigger = (
    <button
      type="button"
      className="ui-date-range__trigger"
      aria-expanded={open}
      aria-controls={panelId}
      aria-label={ariaLabel}
      onClick={() => setOpen(!open)}
    >
      <span
        className={cn(
          'ui-date-range__trigger-label',
          summary === '—' && 'is-placeholder',
        )}
      >
        {triggerLabel}
      </span>
      <svg
        width="14"
        height="14"
        viewBox="0 0 24 24"
        fill="none"
        stroke="currentColor"
        strokeWidth="1.75"
        aria-hidden
      >
        <rect x="3" y="5" width="18" height="16" rx="2" />
        <path d="M3 10h18M8 3v4M16 3v4" />
      </svg>
    </button>
  );

  if (isModal) {
    return (
      <div
        ref={rootRef}
        className={cn(
          'ui-date-range',
          open && 'is-open',
          !showTrigger && 'ui-date-range--triggerless',
          className,
        )}
      >
        {showTrigger ? trigger : null}
        {open
          ? createPortal(
              <div className="ui-date-range-modal" role="presentation">
                <button
                  type="button"
                  className="ui-date-range-modal__backdrop"
                  aria-label={cancelLabel}
                  onClick={cancel}
                />
                {panel}
              </div>,
              document.body,
            )
          : null}
      </div>
    );
  }

  return (
    <div
      ref={rootRef}
      className={cn('ui-date-range', open && 'is-open', className)}
      aria-label={ariaLabel}
    >
      {trigger}
      {open ? panel : null}
    </div>
  );
}
