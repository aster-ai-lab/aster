export const MIN_DOCUMENT_ZOOM = 50;
export const MAX_DOCUMENT_ZOOM = 200;

export type DocumentZoomFit = 'page' | 'width';

export interface DocumentZoomFitMetrics {
  pageHeight: number;
  pageWidth: number;
  viewportHeight: number;
  viewportWidth: number;
  viewportPadding: {
    top: number;
    right: number;
    bottom: number;
    left: number;
  };
}

export function clampDocumentZoom(zoom: number): number {
  return Math.min(
    MAX_DOCUMENT_ZOOM,
    Math.max(MIN_DOCUMENT_ZOOM, Math.round(zoom)),
  );
}

export function documentZoomForFit(
  fit: DocumentZoomFit,
  metrics: DocumentZoomFitMetrics,
): number {
  const availableWidth = Math.max(
    1,
    metrics.viewportWidth -
      metrics.viewportPadding.left -
      metrics.viewportPadding.right,
  );
  const widthZoom = (availableWidth / Math.max(1, metrics.pageWidth)) * 100;
  const availableHeight = Math.max(
    1,
    metrics.viewportHeight -
      metrics.viewportPadding.top -
      metrics.viewportPadding.bottom,
  );
  const heightZoom = (availableHeight / Math.max(1, metrics.pageHeight)) * 100;
  const fittedZoom = fit === 'width' ? widthZoom : Math.min(widthZoom, heightZoom);
  // Fit is a containment guarantee, not a readability preference. The manual 50%
  // floor must not force a page wider than its viewport, and rounding up would
  // reintroduce the same overflow by a few pixels.
  return Math.min(MAX_DOCUMENT_ZOOM, Math.max(1, Math.floor(fittedZoom)));
}
