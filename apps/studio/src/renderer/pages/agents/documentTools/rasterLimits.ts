export const RASTER_PIXEL_LIMIT = 33_554_432
export const RASTER_OUTPUT_BYTE_LIMIT = 25 * 1024 * 1024

export function validateRasterSize(width: number, height: number): void {
  if (
    !Number.isSafeInteger(width) ||
    !Number.isSafeInteger(height) ||
    width <= 0 ||
    height <= 0 ||
    !Number.isSafeInteger(width * height) ||
    width * height > RASTER_PIXEL_LIMIT
  )
    throw new Error('raster-limit')
}
