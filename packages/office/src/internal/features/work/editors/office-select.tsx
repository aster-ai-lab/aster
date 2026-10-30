import { SearchSelect } from '@a3s-lab/ui/components/console/SearchSelect';
import { type CSSProperties, use } from 'react';
import { OfficeControlSizeContext } from '../../../design-system/primitives/form/control-size';
import { officeFloatingPortalRoot } from '../../../design-system/primitives/overlay/portal-root';
import { officeMessage } from '../../../i18n/office-locale';
import { useOfficeMessages } from './office-messages-context';

export interface OfficeSelectOption<T extends string = string> {
  value: T;
  label: string;
  disabled?: boolean;
  group?: string;
  meta?: string;
  previewStyle?: CSSProperties;
  searchText?: string;
}

export function OfficeSelect<T extends string>({
  ariaLabel,
  value,
  options,
  onValueChange,
  disabled = false,
  placeholder,
  className = '',
  menuClassName = '',
  ariaKeyShortcuts,
  initialFocus = false,
  id,
  'aria-describedby': ariaDescribedBy,
  'aria-invalid': ariaInvalid,
  'aria-required': ariaRequired,
}: {
  ariaLabel: string;
  value: T;
  options: readonly OfficeSelectOption<T>[];
  onValueChange: (value: T) => void;
  disabled?: boolean;
  placeholder?: string;
  className?: string;
  /** Extra class on the portaled list, so one menu can follow a different chrome. */
  menuClassName?: string;
  ariaKeyShortcuts?: string;
  initialFocus?: boolean;
  id?: string;
  'aria-describedby'?: string;
  'aria-invalid'?: boolean;
  'aria-required'?: boolean;
}) {
  const messages = useOfficeMessages();
  const size = use(OfficeControlSizeContext);
  return (
    <SearchSelect
      data-ui="office.select"
      role="combobox"
      id={id}
      value={value}
      options={options.map(({ searchText, ...option }) => ({ ...option, keywords: searchText }))}
      onChange={(next) => {
        const selected = options.find((option) => option.value === next && !option.disabled);
        if (selected) onValueChange(selected.value);
      }}
      className={`work-office-select ${className}`}
      panelClassName={`work-office-select-menu ${menuClassName}`}
      portalContainer={(trigger) => officeFloatingPortalRoot(trigger.ownerDocument, trigger)}
      size={size}
      disabled={disabled}
      searchable={options.length > 8}
      allowClear={false}
      placeholder={placeholder ?? officeMessage(messages, 'office.select.placeholder')}
      searchPlaceholder={officeMessage(messages, 'office.select.search')}
      emptyText={officeMessage(messages, 'office.select.empty')}
      aria-label={ariaLabel}
      aria-describedby={ariaDescribedBy}
      aria-invalid={ariaInvalid}
      aria-required={ariaRequired}
      aria-keyshortcuts={ariaKeyShortcuts}
      data-autofocus={initialFocus ? 'true' : undefined}
    />
  );
}
