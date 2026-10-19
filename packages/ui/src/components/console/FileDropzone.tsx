import { useCallback, useState, type DragEvent } from 'react';
import { cn } from '../../lib/cn';

export type FileDropzoneProps = {
  accept?: string;
  multiple?: boolean;
  disabled?: boolean;
  label?: string;
  hint?: string;
  onFiles: (files: File[]) => void;
  className?: string;
};

export function FileDropzone({
  accept,
  multiple = false,
  disabled,
  label = 'Drop files here or click to browse',
  hint,
  onFiles,
  className = '',
}: FileDropzoneProps) {
  const [active, setActive] = useState(false);

  const take = useCallback(
    (list: FileList | null) => {
      if (!list || disabled) return;
      onFiles(Array.from(list));
    },
    [disabled, onFiles],
  );

  const onDrop = (e: DragEvent) => {
    e.preventDefault();
    setActive(false);
    take(e.dataTransfer.files);
  };

  return (
    <label
      className={cn(
        'flex cursor-pointer flex-col items-center justify-center gap-1 rounded-xl border border-dashed border-[color:var(--color-quiet-border)] bg-[color:var(--color-control-fill)] px-6 py-10 text-center transition-colors hover:bg-[color:var(--color-quiet-hover)]',
        active && 'bg-[color:var(--color-control-fill-active)]',
        disabled && 'cursor-not-allowed opacity-50',
        className,
      )}
      onDragEnter={(e) => {
        e.preventDefault();
        setActive(true);
      }}
      onDragOver={(e) => e.preventDefault()}
      onDragLeave={() => setActive(false)}
      onDrop={onDrop}
    >
      <span className="text-sm font-medium text-ink">{label}</span>
      {hint ? <span className="text-xs text-ink-faint">{hint}</span> : null}
      <input
        type="file"
        className="sr-only"
        accept={accept}
        multiple={multiple}
        disabled={disabled}
        onChange={(e) => take(e.target.files)}
      />
    </label>
  );
}
