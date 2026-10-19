import { cn } from '../../lib/cn';
import { CopyButton } from '../primitives/copy-button';

export type CodeBlockProps = {
  code: string;
  language?: string;
  className?: string;
  copyable?: boolean;
};

export function CodeBlock({
  code,
  language,
  className = '',
  copyable = true,
}: CodeBlockProps) {
  return (
    <div
      className={cn(
        'overflow-hidden rounded-lg border-0 bg-[#171717] shadow-[var(--shadow-card)]',
        className,
      )}
    >
      <div className="flex items-center justify-between bg-white/5 px-5 py-3">
        <span className="font-mono text-xs text-white/50">
          {language ?? 'text'}
        </span>
        {copyable ? <CopyButton value={code} className="!h-7 !text-xs" /> : null}
      </div>
      <pre className="overflow-auto p-5 font-mono text-[13px] leading-[1.5] text-white">
        <code>{code}</code>
      </pre>
    </div>
  );
}

export type DiffLine = {
  type: 'add' | 'remove' | 'context';
  text: string;
};

export type DiffViewerProps = {
  lines: DiffLine[];
  className?: string;
};

export function DiffViewer({ lines, className = '' }: DiffViewerProps) {
  return (
    <pre
      className={cn(
        'ui-card overflow-auto p-5 font-mono text-[13px] leading-[1.5]',
        className,
      )}
    >
      {lines.map((line, i) => (
        <div
          key={`${i}-${line.text.slice(0, 12)}`}
          className={cn(
            line.type === 'add' && 'bg-emerald-500/10 text-emerald-800',
            line.type === 'remove' && 'bg-rose-500/10 text-rose-700',
            line.type === 'context' && 'text-ink-muted',
          )}
        >
          <span className="inline-block w-4 select-none opacity-60">
            {line.type === 'add' ? '+' : line.type === 'remove' ? '-' : ' '}
          </span>
          {line.text}
        </div>
      ))}
    </pre>
  );
}
