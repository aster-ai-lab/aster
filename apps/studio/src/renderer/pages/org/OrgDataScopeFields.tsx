import { Checkbox, Select, SelectContent, SelectItem, SelectTrigger, SelectValue } from '@a3s-lab/ui'
import { useTranslation } from 'react-i18next'

import type { OrgDataScope } from './orgApi'
import type { OrgDepartment } from './orgApi'

const SCOPES = [
  { kind: 'self', label: 'org.scope.self' },
  { kind: 'department', label: 'org.scope.department' },
  { kind: 'department_tree', label: 'org.scope.departmentTree' },
  { kind: 'custom', label: 'org.scope.custom' },
  { kind: 'custom_tree', label: 'org.scope.customTree' },
  { kind: 'all', label: 'org.scope.all' }
] as const

export function validOrgDataScope(scope: OrgDataScope | undefined): scope is OrgDataScope {
  return (
    !!scope &&
    Array.isArray(scope.department_ids) &&
    scope.department_ids.every((id) => typeof id === 'string' && id.trim().length > 0) &&
    new Set(scope.department_ids).size === scope.department_ids.length &&
    SCOPES.some((item) => item.kind === scope.kind) &&
    (scope.kind === 'custom' || scope.kind === 'custom_tree'
      ? scope.department_ids.length > 0 && scope.department_ids.length <= 100
      : scope.department_ids.length === 0)
  )
}

export function sameOrgDataScope(left: OrgDataScope | undefined, right: OrgDataScope | undefined): boolean {
  return (
    !!left &&
    !!right &&
    left.kind === right.kind &&
    left.department_ids.length === right.department_ids.length &&
    left.department_ids.every((id) => right.department_ids.includes(id))
  )
}

export function OrgDataScopeFields({
  scope,
  departments,
  disabled,
  allowedKinds,
  onChange
}: {
  scope: OrgDataScope | undefined
  departments: OrgDepartment[] | undefined
  disabled: boolean
  allowedKinds?: readonly OrgDataScope['kind'][]
  onChange: (scope: OrgDataScope) => void
}) {
  const { t } = useTranslation()
  const custom = scope?.kind === 'custom' || scope?.kind === 'custom_tree'
  return (
    <fieldset disabled={disabled} className="grid min-w-0 gap-2 border-0 p-0">
      <legend className="mb-2 text-[13px] font-medium text-[#171717]">{t('org.scope.title')}</legend>
      <Select
        value={scope?.kind ?? ''}
        disabled={disabled}
        onValueChange={(kind) => {
          const selected = SCOPES.find((item) => item.kind === kind)
          if (!selected || (allowedKinds && !allowedKinds.includes(selected.kind))) return
          const keepDepartments = selected.kind === 'custom' || selected.kind === 'custom_tree'
          onChange({ kind: selected.kind, department_ids: keepDepartments ? (scope?.department_ids ?? []) : [] })
        }}>
        <SelectTrigger aria-label={t('org.scope.title')} className="h-[30px] max-w-sm text-[13px]">
          <SelectValue placeholder={t('org.scope.unavailable')} />
        </SelectTrigger>
        <SelectContent>
          {SCOPES.map((item) => (
            <SelectItem key={item.kind} value={item.kind} disabled={allowedKinds && !allowedKinds.includes(item.kind)}>
              {t(item.label)}
            </SelectItem>
          ))}
        </SelectContent>
      </Select>
      <p className="m-0 text-[12px] leading-5 text-[#8a8a8a]">{t('org.scope.description')}</p>
      {custom && scope ? (
        <div className="grid max-h-56 gap-1 overflow-y-auto rounded-[8px] border border-[#ececec] p-2">
          {departments?.map((department) => (
            <label key={department.id} className="flex min-h-8 items-center gap-2 text-[13px] text-[#3f3f3f]">
              <Checkbox
                checked={scope.department_ids.includes(department.id)}
                disabled={disabled}
                onCheckedChange={(checked) =>
                  onChange({
                    ...scope,
                    department_ids:
                      checked === true
                        ? [...scope.department_ids.filter((id) => id !== department.id), department.id]
                        : scope.department_ids.filter((id) => id !== department.id)
                  })
                }
              />
              {department.name}
            </label>
          ))}
          {!departments?.length ? (
            <p role="status" className="m-0 text-[12px] text-[#8a8a8a]">
              {t('org.scope.departmentsUnavailable')}
            </p>
          ) : null}
        </div>
      ) : null}
      {custom && !scope?.department_ids.length ? (
        <p role="alert" className="m-0 text-[12px] text-[#b42318]">
          {t('org.scope.selectDepartments')}
        </p>
      ) : null}
    </fieldset>
  )
}
