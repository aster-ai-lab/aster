import { Badge, type BadgeTone } from '../primitives/badge';

export type IdentityStatus =
  | 'active'
  | 'invited'
  | 'disabled'
  | 'resigned'
  | (string & {});

function toneFor(status: string): BadgeTone {
  switch (status) {
    case 'active':
      return 'success';
    case 'invited':
      return 'warn';
    case 'disabled':
    case 'resigned':
      return 'neutral';
    default:
      return 'warn';
  }
}

export type IdentityStatusBadgeProps = {
  status: IdentityStatus;
  /** Localized label; defaults to raw status string. */
  label?: string;
};

/** Status chip — DESIGN `badge-pill` (uppercase caption · pill radius). */
export function IdentityStatusBadge({
  status,
  label,
}: IdentityStatusBadgeProps) {
  return <Badge tone={toneFor(status)}>{label ?? status}</Badge>;
}
