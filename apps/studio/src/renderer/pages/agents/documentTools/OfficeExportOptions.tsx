import { SearchSelect } from '@a3s-lab/ui'
import { useTranslation } from 'react-i18next'

import type { OfficeExportOptions as Options } from './officeSvg'
import { PDF_IMAGE_FORMATS, parsePdfImageFormat } from './pdfExport'

const FIELD = 'h-9 min-w-0 rounded-[8px] border border-[var(--spec-button-border,#e5e5e5)] px-2 focus-visible:outline-2'

export function OfficeExportOptions({
  value,
  spreadsheet,
  image,
  printLayout = false,
  busy,
  onChange
}: {
  value: Options
  spreadsheet: boolean
  image: boolean
  printLayout?: boolean
  busy: boolean
  onChange: (value: Options) => void
}) {
  const { t } = useTranslation()
  return (
    <fieldset disabled={busy} className="mt-4 flex flex-col gap-4 text-[13px]">
      <legend className="mb-2 font-medium">{t('workbench.feature.pdf.tools.workspace.office.export')}</legend>
      <label className="flex flex-col gap-1.5">
        {t(
          spreadsheet
            ? 'workbench.feature.pdf.tools.workspace.office.sheets'
            : printLayout
              ? 'workbench.feature.pdf.tools.workspace.office.print_pages'
              : 'workbench.feature.pdf.tools.workspace.office.pages'
        )}
        <input
          className={FIELD}
          value={value.pageRange}
          maxLength={1024}
          placeholder={t('workbench.feature.pdf.tools.workspace.pdf.pages_hint')}
          data-ui="document-tools.office-pages"
          onChange={(event) => onChange({ ...value, pageRange: event.target.value })}
        />
      </label>
      <p className="text-[12px]">
        {t(
          printLayout
            ? 'workbench.feature.pdf.tools.workspace.office.print_order'
            : 'workbench.feature.pdf.tools.workspace.pdf.order_hint'
        )}
      </p>
      {image ? (
        <label className="flex flex-col gap-1.5">
          {t('workbench.feature.pdf.tools.workspace.pdf.format')}
          <SearchSelect
            searchable={false}
            allowClear={false}
            data-ui="document-tools.office-format"
            aria-label={t('workbench.feature.pdf.tools.workspace.pdf.format')}
            value={value.imageFormat}
            options={PDF_IMAGE_FORMATS.map((format) => ({ value: format, label: format === 'jpeg' ? 'JPEG' : 'PNG' }))}
            onChange={(next) => onChange({ ...value, imageFormat: parsePdfImageFormat(next) })}
          />
        </label>
      ) : null}
      <p className="text-[12px]">
        {t(
          printLayout
            ? 'workbench.feature.pdf.tools.workspace.office.print_scope'
            : 'workbench.feature.pdf.tools.workspace.office.scope_hint'
        )}
      </p>
      <p className="text-[12px]">{t('workbench.feature.pdf.tools.workspace.pdf.apply_hint')}</p>
    </fieldset>
  )
}
