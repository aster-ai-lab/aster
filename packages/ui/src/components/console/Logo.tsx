import { useSyncExternalStore } from 'react';
import iconDark from '../../assets/a3s-icon-dark-transparent.svg';
import iconLight from '../../assets/a3s-icon-light-transparent.svg';
import horizontalDark from '../../assets/a3s-horizontal-dark-transparent.svg';
import horizontalLight from '../../assets/a3s-horizontal-light-transparent.svg';
import verticalDark from '../../assets/a3s-vertical-dark-transparent.svg';
import verticalLight from '../../assets/a3s-vertical-light-transparent.svg';
import { cn } from '../../lib/cn';

/**
 * Brand layouts (from logo manifest):
 * - horizontal — nav, auth chrome, email signatures, wide banners
 * - vertical — footers, posters, covers, tall brand columns
 * - icon — app chrome, favicons, avatars, square tiles (safe for circular crop)
 *
 * Tone matches **surface**, same as asset filenames:
 * - light — dark mark on light surfaces
 * - dark — light mark on dark surfaces
 * - auto — follow `document.documentElement.dataset.theme`
 *   (defaults to light when theme is unset; pass an explicit tone on always-dark sites)
 */
export type LogoLayout = 'horizontal' | 'vertical' | 'icon';
export type LogoTone = 'auto' | 'light' | 'dark';

type LogoProps = {
  className?: string;
  /** @deprecated Prefer `layout="icon"` or `LogoMark`. */
  markOnly?: boolean;
  layout?: LogoLayout;
  /** Height in CSS pixels; width follows layout aspect. */
  size?: number;
  tone?: LogoTone;
  alt?: string;
};

const ASPECT: Record<LogoLayout, number> = {
  horizontal: 106 / 44,
  vertical: 80 / 88,
  icon: 1,
};

const SRC: Record<LogoLayout, { light: string; dark: string }> = {
  icon: { dark: iconDark, light: iconLight },
  horizontal: { dark: horizontalDark, light: horizontalLight },
  vertical: { dark: verticalDark, light: verticalLight },
};

function readSurfaceTone(): 'light' | 'dark' {
  if (typeof document === 'undefined') return 'light';
  return document.documentElement.dataset.theme === 'dark' ? 'dark' : 'light';
}

function subscribeTheme(onChange: () => void) {
  if (typeof document === 'undefined') return () => {};
  const root = document.documentElement;
  const observer = new MutationObserver(onChange);
  observer.observe(root, { attributes: true, attributeFilter: ['data-theme'] });
  return () => observer.disconnect();
}

function useSurfaceTone(tone: LogoTone): 'light' | 'dark' {
  const autoTone = useSyncExternalStore(subscribeTheme, readSurfaceTone, readSurfaceTone);
  if (tone === 'auto') return autoTone;
  return tone;
}

export function LogoMark({
  size = 28,
  className = '',
  tone = 'auto',
  alt = 'A3S',
}: {
  size?: number;
  className?: string;
  tone?: LogoTone;
  alt?: string;
}) {
  return <Logo layout="icon" size={size} className={className} tone={tone} alt={alt} />;
}

export function Logo({
  className = '',
  markOnly = false,
  layout: layoutProp,
  size = 28,
  tone = 'auto',
  alt = 'A3S',
}: LogoProps) {
  const surface = useSurfaceTone(tone);
  const layout: LogoLayout = markOnly ? 'icon' : (layoutProp ?? 'horizontal');
  const width = Math.round(size * ASPECT[layout]);

  return (
    <img
      src={SRC[layout][surface]}
      width={width}
      height={size}
      alt={alt}
      decoding="async"
      className={cn(`site-logo site-logo--${layout}`, className)}
      draggable={false}
    />
  );
}
