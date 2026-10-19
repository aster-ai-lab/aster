import { execFileSync } from 'node:child_process'
import { mkdtempSync, rmSync, writeFileSync } from 'node:fs'
import { tmpdir } from 'node:os'
import { join } from 'node:path'

export interface ReviewFixtures {
  dir: string
  docx: string
  pdf: string
  empty: string
  large: string
}

const DOCX_SCRIPT = `
import zipfile, sys
path, sentence = sys.argv[1], sys.argv[2]
document = """<?xml version="1.0" encoding="UTF-8" standalone="yes"?>
<w:document xmlns:w="http://schemas.openxmlformats.org/wordprocessingml/2006/main">
  <w:body><w:p><w:r><w:t>%s</w:t></w:r></w:p></w:body>
</w:document>""" % sentence
content_types = """<?xml version="1.0" encoding="UTF-8" standalone="yes"?>
<Types xmlns="http://schemas.openxmlformats.org/package/2006/content-types">
  <Default Extension="rels" ContentType="application/vnd.openxmlformats-package.relationships+xml"/>
  <Default Extension="xml" ContentType="application/xml"/>
  <Override PartName="/word/document.xml" ContentType="application/vnd.openxmlformats-officedocument.wordprocessingml.document.main+xml"/>
</Types>"""
rels = """<?xml version="1.0" encoding="UTF-8" standalone="yes"?>
<Relationships xmlns="http://schemas.openxmlformats.org/package/2006/relationships">
  <Relationship Id="rId1" Type="http://schemas.openxmlformats.org/officeDocument/2006/relationships/officeDocument" Target="word/document.xml"/>
</Relationships>"""
with zipfile.ZipFile(path, "w") as archive:
    archive.writestr("[Content_Types].xml", content_types)
    archive.writestr("_rels/.rels", rels)
    archive.writestr("word/document.xml", document)
`

/** Tiny Word file plus the reject fixtures. The 8 MB file stays in the temp dir. */
export function createReviewFixtures(sentence: string): ReviewFixtures {
  const dir = mkdtempSync(join(tmpdir(), 'a3s-workbench-office-'))
  const docx = join(dir, 'notice.docx')
  execFileSync('python3', ['-c', DOCX_SCRIPT, docx, sentence], { stdio: 'ignore' })
  const pdf = join(dir, 'scan.pdf')
  const empty = join(dir, 'empty.docx')
  const large = join(dir, 'large.docx')
  writeFileSync(pdf, '%PDF-1.4\n')
  writeFileSync(empty, Buffer.alloc(0))
  writeFileSync(large, Buffer.alloc(8 * 1024 * 1024 + 1))
  return { dir, docx, pdf, empty, large }
}

export function removeReviewFixtures(dir: string) {
  rmSync(dir, { recursive: true, force: true })
}
