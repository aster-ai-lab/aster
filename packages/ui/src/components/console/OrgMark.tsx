import { cn } from '../../lib/cn';

export type OrgMarkProps = {
  name: string;
  /** Stable seed for color (defaults to name). */
  seed?: string;
  /** Optional remote / data URL logo — falls back to initials mark. */
  logoUrl?: string | null;
  size?: number;
  className?: string;
  alt?: string;
};

function markDataUrl(name: string, seed: string): string {
  const letter = (name.trim().charAt(0) || '?').toUpperCase();
  let hash = 0;
  for (let i = 0; i < seed.length; i += 1) {
    hash = (hash * 31 + seed.charCodeAt(i)) >>> 0;
  }
  const tones = ['#111111', '#2a2a2a', '#3d3d3d', '#525252', '#1a1a1a'];
  const bg = tones[hash % tones.length];
  const svg = `<svg xmlns="http://www.w3.org/2000/svg" viewBox="0 0 32 32"><rect width="32" height="32" rx="8" fill="${bg}"/><text x="16" y="21" text-anchor="middle" fill="#f8fafc" font-size="13" font-family="Inter,ui-sans-serif,system-ui,sans-serif" font-weight="700">${letter}</text></svg>`;
  return `data:image/svg+xml,${encodeURIComponent(svg)}`;
}

/** Monochrome org / tenant mark — initials SVG or provided logo_url. */
export function OrgMark({
  name,
  seed,
  logoUrl,
  size = 20,
  className = '',
  alt = '',
}: OrgMarkProps) {
  const src = logoUrl?.trim() || markDataUrl(name, seed ?? name);
  return (
    <img
      className={cn('block rounded-md object-cover', className)}
      src={src}
      alt={alt}
      width={size}
      height={size}
      style={{ width: size, height: size }}
    />
  );
}
