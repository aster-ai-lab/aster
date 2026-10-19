import { useMemo, useState, type FormEvent, type ReactNode } from 'react';
import { cn } from '../../lib/cn';
import { Button } from '../primitives/button';
import { FormField } from './FormField';
import { Input } from '../primitives/input';
import { SearchSelect } from './SearchSelect';
import { TreeSelect, type TreeSelectOption } from '../primitives/tree-select';
import type { TreeNode } from './Tree';
import { WizardShell, type WizardStep } from './WizardShell';

const toTreeSelectOptions = (nodes: TreeNode[]): TreeSelectOption[] =>
  nodes.map((node) => ({
    value: node.id,
    label: node.label,
    icon: node.icon,
    disabled: node.disabled,
    children: node.children ? toTreeSelectOptions(node.children) : undefined,
  }));

export type InviteWizardSelectOption = {
  value: string;
  label: string;
  icon?: ReactNode;
};

export type InviteWizardValues = {
  email: string;
  displayName: string;
  departmentId: string;
  roleCode: string;
  message: string;
};

export type InviteWizardResult = {
  /** Evidence id shown on success (invitation / operation). */
  invitationId: string;
};

export type InviteWizardProps = {
  /** Hierarchical departments for primary-department TreeSelect. */
  departmentNodes: TreeNode[];
  roleOptions: InviteWizardSelectOption[];
  initialValues?: Partial<InviteWizardValues>;
  onSubmit: (values: InviteWizardValues) => Promise<InviteWizardResult>;
  onCancel?: () => void;
  onViewAudit?: (invitationId: string) => void;
  labels?: Partial<{
    stepBasic: string;
    stepAssign: string;
    stepConfirm: string;
    email: string;
    emailPlaceholder: string;
    displayName: string;
    displayNamePlaceholder: string;
    department: string;
    departmentPlaceholder: string;
    role: string;
    rolePlaceholder: string;
    message: string;
    messagePlaceholder: string;
    back: string;
    next: string;
    submit: string;
    cancel: string;
    successTitle: string;
    successBody: string;
    viewAudit: string;
    done: string;
  }>;
  className?: string;
  footerExtra?: ReactNode;
};

const DEFAULT_STEPS: WizardStep[] = [
  { id: 'basic', title: 'Basic info' },
  { id: 'assign', title: 'Department & role' },
  { id: 'confirm', title: 'Confirm' },
];

function firstNodeId(nodes: TreeNode[]): string {
  return nodes[0]?.id ?? '';
}

function findNodeLabel(nodes: TreeNode[], id: string): string | undefined {
  for (const n of nodes) {
    if (n.id === id) return n.label;
    if (n.children?.length) {
      const hit = findNodeLabel(n.children, id);
      if (hit) return hit;
    }
  }
  return undefined;
}

/**
 * W-P0-3 / USR-2 invite wizard — 3 steps + evidence id on success.
 * Presentation only; submit calls control-plane via `onSubmit`.
 */
export function InviteWizard({
  departmentNodes,
  roleOptions,
  initialValues,
  onSubmit,
  onCancel,
  onViewAudit,
  labels = {},
  className = '',
  footerExtra,
}: InviteWizardProps) {
  const steps = useMemo<WizardStep[]>(
    () => [
      { id: 'basic', title: labels.stepBasic ?? DEFAULT_STEPS[0].title },
      { id: 'assign', title: labels.stepAssign ?? DEFAULT_STEPS[1].title },
      { id: 'confirm', title: labels.stepConfirm ?? DEFAULT_STEPS[2].title },
    ],
    [labels.stepBasic, labels.stepAssign, labels.stepConfirm],
  );

  const [stepId, setStepId] = useState('basic');
  const [busy, setBusy] = useState(false);
  const [error, setError] = useState<string | null>(null);
  const [invitationId, setInvitationId] = useState<string | null>(null);
  const [values, setValues] = useState<InviteWizardValues>({
    email: initialValues?.email ?? '',
    displayName: initialValues?.displayName ?? '',
    departmentId:
      initialValues?.departmentId ?? firstNodeId(departmentNodes),
    roleCode: initialValues?.roleCode ?? roleOptions[0]?.value ?? '',
    message: initialValues?.message ?? '',
  });

  const idx = steps.findIndex((s) => s.id === stepId);

  function patch(partial: Partial<InviteWizardValues>) {
    setValues((prev) => ({ ...prev, ...partial }));
  }

  function goNext() {
    setError(null);
    if (stepId === 'basic') {
      if (!values.email.trim()) {
        setError('Email is required');
        return;
      }
      setStepId('assign');
      return;
    }
    if (stepId === 'assign') {
      if (!values.departmentId) {
        setError('Department is required');
        return;
      }
      setStepId('confirm');
    }
  }

  function goBack() {
    setError(null);
    if (stepId === 'assign') setStepId('basic');
    if (stepId === 'confirm') setStepId('assign');
  }

  async function submit(event?: FormEvent) {
    event?.preventDefault();
    setBusy(true);
    setError(null);
    try {
      const result = await onSubmit({
        ...values,
        email: values.email.trim(),
        displayName: values.displayName.trim(),
      });
      setInvitationId(result.invitationId);
    } catch (err) {
      setError(err instanceof Error ? err.message : String(err));
    } finally {
      setBusy(false);
    }
  }

  if (invitationId) {
    return (
      <div className={cn('flex flex-col gap-3', className)}>
        <div>
          <h3 className="text-sm font-medium text-ink">
            {labels.successTitle ?? 'Invitation sent'}
          </h3>
          <p className="mt-0.5 text-sm text-ink-muted">
            {labels.successBody ?? 'Save this evidence id for audit.'}
          </p>
          <p className="mt-2 font-mono text-sm text-ink">{invitationId}</p>
        </div>
        <div className="flex flex-wrap gap-2">
          {onViewAudit ? (
            <Button
              type="button"
              size="sm"
              variant="outline"
              onClick={() => onViewAudit(invitationId)}
            >
              {labels.viewAudit ?? 'View audit'}
            </Button>
          ) : null}
          <Button type="button" size="sm" onClick={onCancel}>
            {labels.done ?? 'Done'}
          </Button>
        </div>
      </div>
    );
  }

  return (
    <form
      className={cn('flex flex-col gap-3', className)}
      onSubmit={(e) => {
        e.preventDefault();
        if (stepId === 'confirm') void submit();
        else goNext();
      }}
    >
      <WizardShell
        steps={steps}
        currentId={stepId}
        onBack={idx > 0 ? goBack : undefined}
        onNext={
          stepId === 'confirm'
            ? () => void submit()
            : goNext
        }
        nextLabel={
          stepId === 'confirm'
            ? (labels.submit ?? 'Send invite')
            : (labels.next ?? 'Continue')
        }
        backLabel={labels.back ?? 'Back'}
        nextDisabled={busy}
        footerExtra={
          <>
            {footerExtra}
            {onCancel ? (
              <Button
                type="button"
                size="sm"
                variant="ghost"
                onClick={onCancel}
                disabled={busy}
              >
                {labels.cancel ?? 'Cancel'}
              </Button>
            ) : null}
          </>
        }
      >
        {error ? (
          <p className="text-sm text-[color:var(--color-error)]" role="alert">
            {error}
          </p>
        ) : null}

        {stepId === 'basic' ? (
          <div className="grid gap-3">
            <FormField label={labels.email ?? 'Email'} required>
              <Input
                type="email"
                required
                value={values.email}
                onChange={(e) => patch({ email: e.target.value })}
                placeholder={labels.emailPlaceholder ?? 'name@company.com'}
                autoFocus
              />
            </FormField>
            <FormField label={labels.displayName ?? 'Display name'}>
              <Input
                value={values.displayName}
                onChange={(e) => patch({ displayName: e.target.value })}
                placeholder={labels.displayNamePlaceholder}
              />
            </FormField>
          </div>
        ) : null}

        {stepId === 'assign' ? (
          <div className="grid gap-3">
            <FormField label={labels.department ?? 'Department'} required>
              <TreeSelect
                className="w-full"
                value={values.departmentId}
                onChange={(departmentId) => patch({ departmentId })}
                treeData={toTreeSelectOptions(departmentNodes)}
                placeholder={
                  labels.departmentPlaceholder ?? labels.department ?? 'Department'
                }
                aria-label={labels.department ?? 'Department'}
              />
            </FormField>
            <FormField label={labels.role ?? 'Admin role'}>
              <SearchSelect
                className="w-full"
                value={values.roleCode}
                onChange={(roleCode) => patch({ roleCode })}
                options={[
                  { value: '', label: '—' },
                  ...roleOptions,
                ]}
                placeholder={
                  labels.rolePlaceholder ?? labels.role ?? 'Admin role'
                }
                aria-label={labels.role ?? 'Admin role'}
              />
            </FormField>
            <FormField label={labels.message ?? 'Message (optional)'}>
              <Input
                value={values.message}
                onChange={(e) => patch({ message: e.target.value })}
                placeholder={labels.messagePlaceholder}
              />
            </FormField>
          </div>
        ) : null}

        {stepId === 'confirm' ? (
          <dl className="grid gap-2 text-sm">
            <div className="grid grid-cols-[7rem_1fr] gap-2">
              <dt className="text-ink-faint">{labels.email ?? 'Email'}</dt>
              <dd className="text-ink">{values.email}</dd>
            </div>
            <div className="grid grid-cols-[7rem_1fr] gap-2">
              <dt className="text-ink-faint">
                {labels.displayName ?? 'Display name'}
              </dt>
              <dd className="text-ink">{values.displayName || '—'}</dd>
            </div>
            <div className="grid grid-cols-[7rem_1fr] gap-2">
              <dt className="text-ink-faint">
                {labels.department ?? 'Department'}
              </dt>
              <dd className="text-ink">
                {findNodeLabel(departmentNodes, values.departmentId) ??
                  values.departmentId}
              </dd>
            </div>
            <div className="grid grid-cols-[7rem_1fr] gap-2">
              <dt className="text-ink-faint">{labels.role ?? 'Admin role'}</dt>
              <dd className="text-ink">
                {roleOptions.find((o) => o.value === values.roleCode)?.label ||
                  values.roleCode ||
                  '—'}
              </dd>
            </div>
          </dl>
        ) : null}
      </WizardShell>
    </form>
  );
}
