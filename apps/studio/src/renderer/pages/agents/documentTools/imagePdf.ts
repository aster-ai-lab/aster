export const IMAGE_PAPERS = ['image', 'a4', 'letter'] as const
export const IMAGE_ORIENTATIONS = ['portrait', 'landscape'] as const
export const IMAGE_FITS = ['contain', 'cover'] as const
export type ImagePaper = (typeof IMAGE_PAPERS)[number]
export type ImageOrientation = (typeof IMAGE_ORIENTATIONS)[number]
export type ImageFit = (typeof IMAGE_FITS)[number]
export type ImagePdfOptions = Readonly<{
  paper: ImagePaper
  orientation: ImageOrientation
  fit: ImageFit
  margin: number
}>
export const DEFAULT_IMAGE_PDF: ImagePdfOptions = Object.freeze({
  paper: 'image',
  orientation: 'portrait',
  fit: 'contain',
  margin: 0
})
export function parseImagePaper(value: unknown): ImagePaper {
  if (!IMAGE_PAPERS.includes(value as ImagePaper)) throw new Error('Invalid image-PDF paper')
  return value as ImagePaper
}
export function imagePaperAsStr(value: ImagePaper) {
  return parseImagePaper(value)
}
export function parseImageOrientation(value: unknown): ImageOrientation {
  if (!IMAGE_ORIENTATIONS.includes(value as ImageOrientation)) throw new Error('Invalid image-PDF orientation')
  return value as ImageOrientation
}
export function imageOrientationAsStr(value: ImageOrientation) {
  return parseImageOrientation(value)
}
export function parseImageFit(value: unknown): ImageFit {
  if (!IMAGE_FITS.includes(value as ImageFit)) throw new Error('Invalid image-PDF fit')
  return value as ImageFit
}
export function imageFitAsStr(value: ImageFit) {
  return parseImageFit(value)
}
export function validateImagePdf(options: ImagePdfOptions): ImagePdfOptions {
  if (!Number.isFinite(options.margin) || options.margin < 0 || options.margin > 144)
    throw new Error('image-pdf-options')
  return Object.freeze({
    ...options,
    paper: parseImagePaper(options.paper),
    orientation: parseImageOrientation(options.orientation),
    fit: parseImageFit(options.fit)
  })
}
export function imagePdfPlacement(width: number, height: number, settings = DEFAULT_IMAGE_PDF) {
  const options = validateImagePdf(settings)
  if (!Number.isFinite(width) || !Number.isFinite(height) || width <= 0 || height <= 0) throw new Error('unreadable')
  const natural = options.paper === 'image'
  const paper = options.paper === 'a4' ? [595.28, 841.89] : [612, 792]
  const pageWidth = natural ? width + options.margin * 2 : options.orientation === 'portrait' ? paper[0] : paper[1]
  const pageHeight = natural ? height + options.margin * 2 : options.orientation === 'portrait' ? paper[1] : paper[0]
  const contentWidth = pageWidth - options.margin * 2
  const contentHeight = pageHeight - options.margin * 2
  if (contentWidth <= 0 || contentHeight <= 0) throw new Error('image-pdf-options')
  const scale = natural
    ? 1
    : options.fit === 'contain'
      ? Math.min(contentWidth / width, contentHeight / height)
      : Math.max(contentWidth / width, contentHeight / height)
  const drawWidth = width * scale
  const drawHeight = height * scale
  return {
    pageWidth,
    pageHeight,
    x: (pageWidth - drawWidth) / 2,
    y: (pageHeight - drawHeight) / 2,
    width: drawWidth,
    height: drawHeight,
    clip: { x: options.margin, y: options.margin, width: contentWidth, height: contentHeight }
  }
}
