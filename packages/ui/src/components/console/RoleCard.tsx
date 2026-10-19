import type { ReactNode } from 'react';
import { cn } from '../../lib/cn';
import { Badge } from '../primitives/badge';
import { Button } from '../primitives/button';
import { Card } from './Card';
import { PopConfirm } from './PopConfirm';

export type RoleCardProps = {
  name: string;
  code: string;
  isSystem?: boolean;
  permissionCount?: number;
  memberCount?: number;
  systemLabel?: string;
  permissionsLabel?: string;
  membersLabel?: string;
  editLabel?: string;
  deleteLabel?: string;
  deleteConfirmTitle?: string;
  deleteConfirmDescription?: string;
  deleteCancelLabel?: string;
  deleteDisabled?: boolean;
  onEdit?: () => void;
  onDelete?: () => void | Promise<void>;
  footer?: ReactNode;
  className?: string;
};

function TrashIcon() {
  return (
    <svg
      width="16"
      height="16"
      viewBox="0 0 24 24"
      fill="none"
      stroke="currentColor"
      strokeWidth="1.75"
      strokeLinecap="round"
      strokeLinejoin="round"
      aria-hidden
    >
      <path d="M4 7h16" />
      <path d="M9 7V5h6v2" />
      <path d="M8 7l1 13h6l1-13" />
    </svg>
  );
}

/** W-P0-5 role card — name · code · system mark · counts · actions. */
export function RoleCard({
  name,
  code,
  isSystem = false,
  permissionCount,
  memberCount,
  systemLabel = 'System',
  permissionsLabel = 'permissions',
  membersLabel = 'members',
  editLabel = 'Edit',
  deleteLabel = 'Delete',
  deleteConfirmTitle,
  deleteConfirmDescription,
  deleteCancelLabel = 'Cancel',
  deleteDisabled = false,
  onEdit,
  onDelete,
  footer,
  className = '',
}: RoleCardProps) {
  return (
    <Card className={cn('flex flex-col gap-3 !p-4', className)}>
      <div className="flex flex-wrap items-start justify-between gap-2">
        <div className="min-w-0">
          <h3 className="truncate text-sm font-medium text-ink">{name}</h3>
          <p className="mt-0.5 font-mono text-xs text-ink-muted">{code}</p>
        </div>
        {isSystem ? <Badge tone="neutral">{systemLabel}</Badge> : null}
      </div>
      <dl className="grid grid-cols-2 gap-2 text-sm">
        {typeof permissionCount === 'number' ? (
          <div>
            <dt className="text-xs text-ink-faint">{permissionsLabel}</dt>
            <dd className="font-medium tabular-nums text-ink">
              {permissionCount}
            </dd>
          </div>
        ) : null}
        {typeof memberCount === 'number' ? (
          <div>
            <dt className="text-xs text-ink-faint">{membersLabel}</dt>
            <dd className="font-medium tabular-nums text-ink">{memberCount}</dd>
          </div>
        ) : null}
      </dl>
      {footer ? <div className="min-w-0">{footer}</div> : null}
      <div className="mt-auto flex items-center gap-2">
        {onEdit ? (
          <Button type="button" size="sm" variant="outline" onClick={onEdit}>
            {editLabel}
          </Button>
        ) : null}
        {onDelete && !isSystem ? (
          <PopConfirm
            className="ml-auto"
            title={deleteConfirmTitle ?? deleteLabel}
            description={deleteConfirmDescription}
            confirmLabel={deleteLabel}
            cancelLabel={deleteCancelLabel}
            disabled={deleteDisabled}
            onConfirm={onDelete}
          >
            <Button
              type="button"
              size="sm"
              variant="ghost"
              aria-label={deleteLabel}
              className="!size-8 !px-0 text-ink-muted hover:!text-[color:var(--color-error)]"
            >
              <TrashIcon />
            </Button>
          </PopConfirm>
        ) : null}
      </div>
    </Card>
  );
}
