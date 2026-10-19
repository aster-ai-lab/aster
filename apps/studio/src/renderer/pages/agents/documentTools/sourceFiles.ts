import type { ToolBytes } from '../pdfToolsRun'
import type { DocumentSource } from './task'

export type DocumentInputFile = InstanceType<typeof globalThis.File>

/** Window-owned bytes. IDs survive list edits and are never reassigned to another file. */
export class DocumentSourceFiles {
  private sequence = 0
  private readonly files = new Map<string, DocumentInputFile>()

  constructor(private readonly read: (file: DocumentInputFile) => Promise<ToolBytes>) {}

  add(files: readonly DocumentInputFile[]): readonly DocumentSource[] {
    return files.map((file) => {
      const id = `source-${this.sequence++}`
      this.files.set(id, file)
      return Object.freeze({ id, name: file.name, type: file.type, size: file.size })
    })
  }

  retain(sourceIds: readonly string[]): void {
    const remaining = new Set(sourceIds)
    for (const id of this.files.keys()) if (!remaining.has(id)) this.files.delete(id)
  }

  load = async (source: DocumentSource): Promise<ToolBytes> => {
    const file = this.files.get(source.id)
    if (!file || file.name !== source.name || file.type !== source.type || file.size !== source.size) {
      throw new Error('workbench.feature.pdf.tools.job.unreadable')
    }
    return this.read(file)
  }
}
