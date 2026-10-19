import { useMemo } from 'react';
import { cn } from '../../lib/cn';
import { startOfMonth, toDayKey, type CalendarDay } from '../../lib/date';

export type CalendarProps = {
  month: Date; // any day in the visible month
  weekStartsOn?: 0 | 1;
  weekdayLabels?: string[];
  isSelected?: (dayKey: string) => boolean;
  isInRange?: (dayKey: string) => boolean;
  isRangeStart?: (dayKey: string) => boolean;
  isRangeEnd?: (dayKey: string) => boolean;
  isDisabled?: (dayKey: string) => boolean;
  onDayClick?: (dayKey: string) => void;
  onDayHover?: (dayKey: string | null) => void;
  className?: string;
};

export { toDayKey, parseDayKey, startOfMonth, addMonths } from '../../lib/date';
export type { CalendarDay } from '../../lib/date';

function buildMonthGrid(month: Date, weekStartsOn: 0 | 1): CalendarDay[] {
  const first = startOfMonth(month);
  const firstWeekday = first.getDay();
  const pad = (firstWeekday - weekStartsOn + 7) % 7;
  const start = new Date(first);
  start.setDate(first.getDate() - pad);

  const cells: CalendarDay[] = [];
  for (let i = 0; i < 42; i += 1) {
    const date = new Date(start);
    date.setDate(start.getDate() + i);
    cells.push({
      key: toDayKey(date),
      date,
      inMonth: date.getMonth() === month.getMonth(),
    });
  }
  return cells;
}

const DEFAULT_WEEKDAYS = ['Su', 'Mo', 'Tu', 'We', 'Th', 'Fr', 'Sa'];

export function Calendar({
  month,
  weekStartsOn = 0,
  weekdayLabels,
  isSelected,
  isInRange,
  isRangeStart,
  isRangeEnd,
  isDisabled,
  onDayClick,
  onDayHover,
  className = '',
}: CalendarProps) {
  const days = useMemo(
    () => buildMonthGrid(month, weekStartsOn),
    [month, weekStartsOn],
  );

  const labels = useMemo(() => {
    const base = weekdayLabels ?? DEFAULT_WEEKDAYS;
    if (weekStartsOn === 0) return base;
    return [...base.slice(1), base[0]];
  }, [weekdayLabels, weekStartsOn]);

  return (
    <div className={cn('ui-calendar', className)}>
      <div className="ui-calendar__weekdays" aria-hidden>
        {labels.map((label) => (
          <span key={label}>{label}</span>
        ))}
      </div>
      <div className="ui-calendar__grid" role="grid">
        {days.map((day) => {
          const selected = isSelected?.(day.key) ?? false;
          const inRange = isInRange?.(day.key) ?? false;
          const rangeStart = isRangeStart?.(day.key) ?? false;
          const rangeEnd = isRangeEnd?.(day.key) ?? false;
          const disabled = isDisabled?.(day.key) ?? false;
          return (
            <button
              key={day.key}
              type="button"
              role="gridcell"
              className={cn(
                'ui-calendar__day',
                !day.inMonth && 'is-outside',
                selected && 'is-selected',
                inRange && 'is-in-range',
                rangeStart && 'is-range-start',
                rangeEnd && 'is-range-end',
              )}
              disabled={disabled}
              onClick={() => onDayClick?.(day.key)}
              onMouseEnter={() => onDayHover?.(day.key)}
              onMouseLeave={() => onDayHover?.(null)}
            >
              {day.date.getDate()}
            </button>
          );
        })}
      </div>
    </div>
  );
}
