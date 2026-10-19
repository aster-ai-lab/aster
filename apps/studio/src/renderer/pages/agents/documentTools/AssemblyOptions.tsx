import { Checkbox, Input, SearchSelect } from '@a3s-lab/ui'
import { useTranslation } from 'react-i18next'

import {
  IMAGE_PAPERS,
  IMAGE_ORIENTATIONS,
  IMAGE_FITS,
  parseImagePaper,
  parseImageOrientation,
  parseImageFit,
  type ImagePdfOptions
} from './imagePdf'
import {
  LONG_ALIGNMENTS,
  LONG_BACKGROUNDS,
  parseLongAlignment,
  parseLongBackground,
  type LongImageOptions
} from './longImage'
import { PDF_IMAGE_FORMATS, parsePdfImageFormat } from './pdfExport'

const LABELS = {
  long: 'workbench.feature.pdf.tools.workspace.assembly.long',
  width: 'workbench.feature.pdf.tools.workspace.assembly.width',
  auto_width: 'workbench.feature.pdf.tools.workspace.assembly.auto_width',
  gap: 'workbench.feature.pdf.tools.workspace.assembly.gap',
  alignment: 'workbench.feature.pdf.tools.workspace.assembly.alignment',
  left: 'workbench.feature.pdf.tools.workspace.assembly.left',
  center: 'workbench.feature.pdf.tools.workspace.assembly.center',
  right: 'workbench.feature.pdf.tools.workspace.assembly.right',
  background: 'workbench.feature.pdf.tools.workspace.assembly.background',
  white: 'workbench.feature.pdf.tools.workspace.assembly.white',
  transparent: 'workbench.feature.pdf.tools.workspace.assembly.transparent',
  upscale: 'workbench.feature.pdf.tools.workspace.assembly.upscale',
  segment: 'workbench.feature.pdf.tools.workspace.assembly.segment',
  single: 'workbench.feature.pdf.tools.workspace.assembly.single',
  long_hint: 'workbench.feature.pdf.tools.workspace.assembly.long_hint',
  pdf: 'workbench.feature.pdf.tools.workspace.assembly.pdf',
  paper: 'workbench.feature.pdf.tools.workspace.assembly.paper',
  image: 'workbench.feature.pdf.tools.workspace.assembly.image',
  a4: 'workbench.feature.pdf.tools.workspace.assembly.a4',
  letter: 'workbench.feature.pdf.tools.workspace.assembly.letter',
  orientation: 'workbench.feature.pdf.tools.workspace.assembly.orientation',
  portrait: 'workbench.feature.pdf.tools.workspace.assembly.portrait',
  landscape: 'workbench.feature.pdf.tools.workspace.assembly.landscape',
  margin: 'workbench.feature.pdf.tools.workspace.assembly.margin',
  fit: 'workbench.feature.pdf.tools.workspace.assembly.fit',
  contain: 'workbench.feature.pdf.tools.workspace.assembly.contain',
  cover: 'workbench.feature.pdf.tools.workspace.assembly.cover',
  pdf_hint: 'workbench.feature.pdf.tools.workspace.assembly.pdf_hint'
} as const
const FIELD = 'h-9 min-w-0 rounded-[8px] border border-[#e5e5e5] px-2 focus-visible:outline-2'

export function LongImageOptionsPanel({
  value,
  busy,
  onChange,
  format = true
}: {
  value: LongImageOptions
  busy: boolean
  format?: boolean
  onChange: (value: LongImageOptions) => void
}) {
  const { t } = useTranslation()
  return (
    <fieldset className="mt-4 flex flex-col gap-3 text-[13px]" disabled={busy}>
      <legend>{t(LABELS.long)}</legend>
      <label className="flex flex-col gap-1">
        {t(LABELS.width)}
        <Input
          className={FIELD}
          type="number"
          min={1}
          max={8192}
          placeholder={t(LABELS.auto_width)}
          value={value.width ?? ''}
          data-ui="document-tools.long-width"
          onChange={(event) =>
            onChange({ ...value, width: event.target.value === '' ? null : Number(event.target.value) })
          }
        />
      </label>
      <label className="flex flex-col gap-1">
        {t(LABELS.gap)}
        <Input
          className={FIELD}
          type="number"
          min={0}
          max={1024}
          value={value.gap}
          data-ui="document-tools.long-gap"
          onChange={(event) => onChange({ ...value, gap: Number(event.target.value) })}
        />
      </label>
      <label className="flex flex-col gap-1">
        {t(LABELS.alignment)}
        <SearchSelect
          searchable={false}
          allowClear={false}
          aria-label={t(LABELS.alignment)}
          value={value.alignment}
          options={LONG_ALIGNMENTS.map((item) => ({ value: item, label: t(LABELS[item]) }))}
          onChange={(next) => onChange({ ...value, alignment: parseLongAlignment(next) })}
        />
      </label>
      <label className="flex flex-col gap-1">
        {t(LABELS.background)}
        <SearchSelect
          searchable={false}
          allowClear={false}
          aria-label={t(LABELS.background)}
          value={value.background}
          options={LONG_BACKGROUNDS.map((item) => ({ value: item, label: t(LABELS[item]) }))}
          onChange={(next) => onChange({ ...value, background: parseLongBackground(next) })}
        />
      </label>
      <label className="flex min-h-9 items-center gap-2">
        <Checkbox
          checked={value.upscale}
          disabled={busy}
          aria-label={t(LABELS.upscale)}
          onCheckedChange={(checked) => onChange({ ...value, upscale: checked === true })}
        />
        {t(LABELS.upscale)}
      </label>
      <label className="flex flex-col gap-1">
        {t(LABELS.segment)}
        <Input
          className={FIELD}
          type="number"
          min={1}
          max={32768}
          value={value.segmentHeight ?? ''}
          placeholder={t(LABELS.single)}
          data-ui="document-tools.long-segment"
          onChange={(event) =>
            onChange({ ...value, segmentHeight: event.target.value === '' ? null : Number(event.target.value) })
          }
        />
      </label>
      {format ? (
        <label className="flex flex-col gap-1">
          {t('workbench.feature.pdf.tools.workspace.pdf.format')}
          <SearchSelect
            searchable={false}
            allowClear={false}
            aria-label={t('workbench.feature.pdf.tools.workspace.pdf.format')}
            value={value.imageFormat}
            options={PDF_IMAGE_FORMATS.map((item) => ({ value: item, label: item === 'png' ? 'PNG' : 'JPEG' }))}
            onChange={(next) => onChange({ ...value, imageFormat: parsePdfImageFormat(next) })}
          />
        </label>
      ) : null}
      <p className="text-[12px]">{t(LABELS.long_hint)}</p>
    </fieldset>
  )
}

export function ImagePdfOptionsPanel({
  value,
  busy,
  onChange
}: {
  value: ImagePdfOptions
  busy: boolean
  onChange: (value: ImagePdfOptions) => void
}) {
  const { t } = useTranslation()
  return (
    <fieldset className="mt-4 flex flex-col gap-3 text-[13px]" disabled={busy}>
      <legend>{t(LABELS.pdf)}</legend>
      <label className="flex flex-col gap-1">
        {t(LABELS.paper)}
        <SearchSelect
          searchable={false}
          allowClear={false}
          data-ui="document-tools.image-paper"
          aria-label={t(LABELS.paper)}
          value={value.paper}
          options={IMAGE_PAPERS.map((item) => ({ value: item, label: t(LABELS[item]) }))}
          onChange={(next) => onChange({ ...value, paper: parseImagePaper(next) })}
        />
      </label>
      <label className="flex flex-col gap-1">
        {t(LABELS.orientation)}
        <SearchSelect
          searchable={false}
          allowClear={false}
          disabled={busy || value.paper === 'image'}
          aria-label={t(LABELS.orientation)}
          value={value.orientation}
          options={IMAGE_ORIENTATIONS.map((item) => ({ value: item, label: t(LABELS[item]) }))}
          onChange={(next) => onChange({ ...value, orientation: parseImageOrientation(next) })}
        />
      </label>
      <label className="flex flex-col gap-1">
        {t(LABELS.margin)}
        <Input
          className={FIELD}
          type="number"
          min={0}
          max={144}
          step="any"
          value={value.margin}
          data-ui="document-tools.image-margin"
          onChange={(event) => onChange({ ...value, margin: Number(event.target.value) })}
        />
      </label>
      <label className="flex flex-col gap-1">
        {t(LABELS.fit)}
        <SearchSelect
          searchable={false}
          allowClear={false}
          disabled={busy || value.paper === 'image'}
          aria-label={t(LABELS.fit)}
          value={value.fit}
          options={IMAGE_FITS.map((item) => ({ value: item, label: t(LABELS[item]) }))}
          onChange={(next) => onChange({ ...value, fit: parseImageFit(next) })}
        />
      </label>
      <p className="text-[12px]">{t(LABELS.pdf_hint)}</p>
    </fieldset>
  )
}
