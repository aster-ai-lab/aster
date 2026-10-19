import { cn } from '../../lib/cn';

type SectionHeaderProps = {
  badge?: string;
  title: string;
  titleMobile?: string[];
  description?: string;
  size?: 'h1' | 'h2' | 'h3';
  align?: 'center' | 'left';
  as?: 'h1' | 'h2' | 'h3';
};

/** DESIGN.md display scale — Inter 600, negative tracking on large sizes. */
const sizeClass = {
  h1: 'text-[32px] leading-[1.05] tracking-[-0.03em] md:text-[48px] md:leading-[1.1] md:tracking-[-0.03em]',
  h2: 'text-[28px] leading-[1.15] tracking-[-0.03em] md:text-[36px] md:leading-[1.15]',
  h3: 'text-[22px] leading-[1.25] tracking-[-0.02em] md:text-[28px] md:leading-[1.2]',
};

export function SectionHeader({
  badge,
  title,
  titleMobile,
  description,
  size = 'h2',
  align = 'center',
  as,
}: SectionHeaderProps) {
  const Tag = as ?? (size === 'h1' ? 'h1' : size === 'h3' ? 'h3' : 'h2');
  const alignCls = align === 'center' ? 'text-center mx-auto' : 'text-left';

  return (
    <div className={cn('relative z-[3]', alignCls)}>
      {badge ? (
        <div
          className={cn(
            'mb-3 inline-flex items-center rounded-full bg-surface px-2.5 py-1',
            align === 'center' && 'mx-auto',
          )}
        >
          <span className="text-xs text-ink">
            {badge}
          </span>
        </div>
      ) : null}
      <Tag
        className={cn(
          'font-sans font-semibold text-ink',
          sizeClass[size],
          alignCls,
        )}
      >
        {titleMobile?.length ? (
          <>
            <span className="hidden md:block">{title}</span>
            <span className="md:hidden">
              {titleMobile.map((line) => (
                <span key={line} className="block">
                  {line}
                </span>
              ))}
            </span>
          </>
        ) : (
          title
        )}
      </Tag>
      {description ? (
        <p
          className={cn(
            'mt-3 max-w-[455px] text-base font-normal leading-6 text-ink-muted',
            align === 'center' && 'mx-auto',
          )}
        >
          {description}
        </p>
      ) : null}
    </div>
  );
}
