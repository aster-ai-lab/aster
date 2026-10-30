import { describe, expect, it } from 'bun:test';
import {
  clampDocumentZoom,
  documentZoomForFit,
  type DocumentZoomFitMetrics,
} from '../src/internal/features/work/editors/document-zoom';

function metrics(viewportWidth: number, viewportHeight = 1_000): DocumentZoomFitMetrics {
  return {
    pageWidth: 794,
    pageHeight: 1_123,
    viewportWidth,
    viewportHeight,
    viewportPadding: { top: 0, right: 0, bottom: 0, left: 0 },
  };
}

describe('document zoom fitting', () => {
  it('shrinks below the manual 50 percent floor instead of overflowing', () => {
    expect(documentZoomForFit('width', metrics(344))).toBe(43);
  });

  it('rounds down so fractional zoom never exceeds the available width', () => {
    expect(documentZoomForFit('width', metrics(345.6))).toBe(43);
  });

  it('keeps a positive zoom for extremely narrow viewports and caps enlargement', () => {
    expect(documentZoomForFit('width', metrics(1))).toBe(1);
    expect(documentZoomForFit('width', metrics(4_000))).toBe(200);
  });

  it('uses the smaller width and height fit for whole-page mode', () => {
    expect(documentZoomForFit('page', metrics(900, 500))).toBe(44);
  });

  it('keeps the independent manual zoom floor unchanged', () => {
    expect(clampDocumentZoom(20)).toBe(50);
    expect(clampDocumentZoom(300)).toBe(200);
  });
});
