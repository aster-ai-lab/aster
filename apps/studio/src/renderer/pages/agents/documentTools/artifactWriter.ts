import type { DocumentWriter } from './application'
import type { DocumentArtifact } from './toolEngine'

export type DocumentSaveDialog = (name: string, bytes: Uint8Array) => Promise<string | null | undefined>

/** The owning native bridge chooses a destination and writes bytes inside its granted root. */
export function documentArtifactWriter(saveFile: DocumentSaveDialog): DocumentWriter<DocumentArtifact> {
  return async (artifact) => {
    const destinations: string[] = []
    if (artifact.output.files.length === 0) throw new Error('workbench.feature.pdf.tools.job.empty')
    for (const file of artifact.output.files) {
      const destination = await saveFile(file.name, file.bytes)
      if (typeof destination !== 'string' || !destination.trim()) {
        throw new Error('workbench.feature.pdf.tools.workspace.save_cancelled')
      }
      destinations.push(destination)
    }
    return { destination: destinations.join('\n'), savedAt: new globalThis.Date().toISOString() }
  }
}
