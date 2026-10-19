import { useMemo, useRef, useState, type ReactNode, type UIEvent } from 'react';
import { cn } from '../../lib/cn';

export type VirtualizedTableColumn<T> = {
  id: string;
  header: ReactNode;
  cell: (row: T) => ReactNode;
  width?: number;
};

export type VirtualizedTableProps<T> = {
  rows: T[];
  rowKey: (row: T) => string;
  columns: VirtualizedTableColumn<T>[];
  rowHeight?: number;
  height?: number;
  className?: string;
};

/** Lightweight windowed table for large catalogs (P2). */
export function VirtualizedTable<T>({
  rows,
  rowKey,
  columns,
  rowHeight = 40,
  height = 360,
  className = '',
}: VirtualizedTableProps<T>) {
  const [scrollTop, setScrollTop] = useState(0);
  const ref = useRef<HTMLDivElement>(null);
  const visible = Math.ceil(height / rowHeight) + 4;
  const start = Math.max(0, Math.floor(scrollTop / rowHeight) - 2);
  const end = Math.min(rows.length, start + visible);
  const slice = useMemo(() => rows.slice(start, end), [rows, start, end]);
  const offsetY = start * rowHeight;

  const onScroll = (e: UIEvent<HTMLDivElement>) => {
    setScrollTop(e.currentTarget.scrollTop);
  };

  return (
    <div
      ref={ref}
      className={cn(
        'ui-card overflow-auto',
        className,
      )}
      style={{ height }}
      onScroll={onScroll}
    >
      <div style={{ height: rows.length * rowHeight, position: 'relative' }}>
        <table className="w-full border-collapse text-left text-sm">
          <thead className="sticky top-0 z-10 bg-bg-elevated text-xs text-ink-faint">
            <tr>
              {columns.map((col) => (
                <th
                  key={col.id}
                  className="h-9 border-b border-[color:var(--color-hairline)] px-4 font-normal"
                  style={col.width ? { width: col.width } : undefined}
                >
                  {col.header}
                </th>
              ))}
            </tr>
          </thead>
          <tbody>
            <tr aria-hidden>
              <td
                colSpan={columns.length}
                style={{ height: offsetY, padding: 0, border: 0 }}
              />
            </tr>
            {slice.map((row) => (
              <tr key={rowKey(row)} className="border-b border-[color:var(--color-hairline)] hover:bg-surface-2" style={{ height: rowHeight }}>
                {columns.map((col) => (
                  <td key={col.id} className="px-4 py-2 text-ink">
                    {col.cell(row)}
                  </td>
                ))}
              </tr>
            ))}
            <tr aria-hidden>
              <td
                colSpan={columns.length}
                style={{
                  height: Math.max(0, (rows.length - end) * rowHeight),
                  padding: 0,
                  border: 0,
                }}
              />
            </tr>
          </tbody>
        </table>
      </div>
    </div>
  );
}
