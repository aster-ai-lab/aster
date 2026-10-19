import { cn } from '../../lib/cn';
import { Button } from '../primitives/button';
import { Card } from './Card';
import { Checkbox } from '../primitives/checkbox';

export type PermissionMatrixProps = {
  rowLabels: string[];
  columnLabels: string[];
  /** matrix[row][col] */
  value: boolean[][];
  readOnly?: boolean;
  /** When not fully readOnly, disable specific rows (e.g. system roles). */
  rowDisabled?: boolean[];
  onChange?: (next: boolean[][]) => void;
  onSave?: () => void;
  className?: string;
};

export function PermissionMatrix({
  rowLabels,
  columnLabels,
  value,
  readOnly = true,
  rowDisabled,
  onChange,
  onSave,
  className = '',
}: PermissionMatrixProps) {
  const toggle = (r: number, c: number) => {
    if (readOnly || !onChange || rowDisabled?.[r]) return;
    const next = value.map((row, ri) =>
      row.map((cell, ci) => (ri === r && ci === c ? !cell : cell)),
    );
    onChange(next);
  };

  return (
    <div className={cn('flex flex-col gap-3', className)}>
      {!readOnly && onSave ? (
        <div className="flex justify-end">
          <Button size="sm" onClick={onSave}>
            Save changes
          </Button>
        </div>
      ) : null}
      <Card className="overflow-auto">
        <table className="w-full min-w-[480px] border-collapse text-left text-sm">
          <thead className="bg-surface text-xs text-ink-faint">
            <tr>
              <th className="px-3 py-2 font-medium">Principal</th>
              {columnLabels.map((col) => (
                <th key={col} className="px-3 py-2 font-medium">
                  {col}
                </th>
              ))}
            </tr>
          </thead>
          <tbody>
            {rowLabels.map((row, ri) => {
              const disabled = readOnly || Boolean(rowDisabled?.[ri]);
              return (
                <tr key={row}>
                  <td className="px-3 py-2 font-medium text-ink">{row}</td>
                  {columnLabels.map((col, ci) => (
                    <td key={col} className="px-3 py-2">
                      <Checkbox
                        checked={Boolean(value[ri]?.[ci])}
                        disabled={disabled}
                        aria-label={`${row} · ${col}`}
                        onChange={() => toggle(ri, ci)}
                      />
                    </td>
                  ))}
                </tr>
              );
            })}
          </tbody>
        </table>
      </Card>
    </div>
  );
}
