import { useEffect, useMemo, useState } from 'react';
import { Button } from '../primitives/button';
import { Checkbox } from '../primitives/checkbox';
import { Dialog } from './Dialog';
import { FormField } from './FormField';

export type ExportField = {
  id: string;
  label: string;
};

export type ExportRange = 'filtered' | 'all';

export type ExportDialogProps = {
  open: boolean;
  onOpenChange: (open: boolean) => void;
  title?: string;
  fields: ExportField[];
  filteredCount: number;
  allCount: number;
  onExport: (result: {
    range: ExportRange;
    fieldIds: string[];
  }) => void;
  labels?: Partial<{
    range: string;
    rangeFiltered: string;
    rangeAll: string;
    fields: string;
    selectAll: string;
    cancel: string;
    confirm: string;
  }>;
};

/**
 * Export scope dialog — choose filtered vs all rows and which columns to include.
 * All fields are selected by default.
 */
export function ExportDialog({
  open,
  onOpenChange,
  title = 'Export',
  fields,
  filteredCount,
  allCount,
  onExport,
  labels = {},
}: ExportDialogProps) {
  const [range, setRange] = useState<ExportRange>('filtered');
  const [selected, setSelected] = useState<string[]>(() =>
    fields.map((f) => f.id),
  );

  useEffect(() => {
    if (!open) return;
    setRange('filtered');
    setSelected(fields.map((f) => f.id));
  }, [open, fields]);

  const allSelected = useMemo(
    () => fields.length > 0 && fields.every((f) => selected.includes(f.id)),
    [fields, selected],
  );

  function toggleField(id: string) {
    setSelected((prev) =>
      prev.includes(id) ? prev.filter((x) => x !== id) : [...prev, id],
    );
  }

  function toggleAll() {
    setSelected(allSelected ? [] : fields.map((f) => f.id));
  }

  return (
    <Dialog
      open={open}
      onOpenChange={onOpenChange}
      title={title}
      footer={
        <>
          <Button
            type="button"
            size="sm"
            variant="outline"
            onClick={() => onOpenChange(false)}
          >
            {labels.cancel ?? 'Cancel'}
          </Button>
          <Button
            type="button"
            size="sm"
            disabled={selected.length === 0}
            onClick={() => {
              onExport({ range, fieldIds: selected });
              onOpenChange(false);
            }}
          >
            {labels.confirm ?? 'Export'}
          </Button>
        </>
      }
    >
      <div className="grid gap-4">
        <FormField label={labels.range ?? 'Data range'}>
          <div className="flex flex-col gap-1.5" role="radiogroup">
            <label className="inline-flex cursor-pointer items-center gap-2 text-sm text-ink">
              <input
                type="radio"
                name="export-range"
                className="size-4 accent-ink"
                checked={range === 'filtered'}
                onChange={() => setRange('filtered')}
              />
              <span>
                {(labels.rangeFiltered ?? 'Current filters ({count})').replace(
                  '{count}',
                  String(filteredCount),
                )}
              </span>
            </label>
            <label className="inline-flex cursor-pointer items-center gap-2 text-sm text-ink">
              <input
                type="radio"
                name="export-range"
                className="size-4 accent-ink"
                checked={range === 'all'}
                onChange={() => setRange('all')}
              />
              <span>
                {(labels.rangeAll ?? 'All data ({count})').replace(
                  '{count}',
                  String(allCount),
                )}
              </span>
            </label>
          </div>
        </FormField>

        <div className="grid gap-1">
          <div className="flex items-center justify-between gap-2">
            <span className="text-xs text-ink-faint">
              {labels.fields ?? 'Fields'}
            </span>
            <button
              type="button"
              className="text-xs text-[color:var(--color-text-link)] hover:opacity-80"
              onClick={toggleAll}
            >
              {labels.selectAll ?? 'Select all'}
            </button>
          </div>
          <div className="grid max-h-48 gap-1.5 overflow-auto rounded-md bg-[color:var(--color-control-fill)] p-2">
            {fields.map((field) => (
              <label
                key={field.id}
                className="flex cursor-pointer items-center gap-2 text-[13px] text-foreground"
              >
                <Checkbox
                  checked={selected.includes(field.id)}
                  onCheckedChange={() => toggleField(field.id)}
                />
                {field.label}
              </label>
            ))}
          </div>
        </div>
      </div>
    </Dialog>
  );
}
