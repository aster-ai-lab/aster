import { readFile, writeFile } from 'node:fs/promises'

import sharp from 'sharp'

const report = JSON.parse(await readFile(process.argv[2], 'utf8'))
if (report.status !== 'passed') throw new Error('Browser filter campaign did not pass')
const evidence = report.scenarios.flatMap((scenario) => scenario.steps.flatMap((step) => step.output?.evidence ?? []))
const checks = [
  ['filter-half', 4, 4, [41, 76, 116, 255]],
  ['filter-pdf-original', 40, 24, [255, 0, 0, 255]],
  ['filter-pdf-gray', 40, 24, [182, 182, 182, 255]],
  ['filter-pdf-selected', 40, 24, [54, 54, 54, 255]]
]
const receipts = []
for (const [name, width, height, expected] of checks) {
  const matches = evidence.filter((artifact) => artifact.name === `downloads/${name}.png`)
  if (matches.length !== 1) throw new Error(`Expected exactly one real ${name} download`)
  const artifact = matches[0]
  const image = sharp(artifact.path, { limitInputPixels: 16_777_216 })
  const metadata = await image.metadata()
  const { data, info } = await image.ensureAlpha().raw().toBuffer({ resolveWithObject: true })
  if (metadata.format !== 'png' || info.width !== width || info.height !== height || info.channels !== 4) {
    throw new Error(`Unexpected filter output dimensions or format: ${name}`)
  }
  // Independent luminance: source10/80/160 -> gray71 -> half41/76/116;
  // PDF green -> gray182, red -> gray54. Check every reopened pixel.
  for (let index = 0; index < data.length; index++) {
    if (data[index] !== expected[index % 4]) throw new Error(`Wrong ${name} pixel at byte ${index}`)
  }
  receipts.push({ artifact: artifact.path, width, height, expected_rgba: expected, all_pixels: 'passed' })
}
await writeFile(process.argv[3], JSON.stringify(receipts, null, 2) + '\n')
