import { PageLoaderRing } from '@renderer/components/layout/PageEntry'

/** Centered file-editor wait state. The ink ring matches the page loader; the bar appears once a ratio is known. */
export function FileEditorLoading({ label, progress }: { label: string; progress?: number | null }) {
  const ratio = progress == null ? null : Math.min(1, Math.max(0, progress))
  return (
    <div className="relative h-full min-h-[320px] w-full flex-1">
      <div
        className="page-loader page-loader--contain"
        role="status"
        aria-live="polite"
        aria-busy="true"
        aria-label={label}>
        <div className="flex flex-col items-center gap-3">
          <div className="page-loader-spinner-wrap">
            <PageLoaderRing />
          </div>
          <p className="m-0 text-[13px] leading-none text-[#8a8a8a]">{label}</p>
          {ratio != null ? (
            <div
              role="progressbar"
              aria-valuemin={0}
              aria-valuemax={100}
              aria-valuenow={Math.round(ratio * 100)}
              aria-label={label}
              className="h-0.5 w-40 overflow-hidden rounded-full bg-[#efefef]">
              <div className="h-full bg-[#171717]" style={{ width: `${Math.round(ratio * 100)}%` }} />
            </div>
          ) : null}
        </div>
      </div>
    </div>
  )
}
