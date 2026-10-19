/** Native result transfer carries source bytes; destination authority stays in the main window. */
export interface DocumentResultRequest {
  id: string
  file: { name: string; mediaType: string; bytesBase64: string }
}
