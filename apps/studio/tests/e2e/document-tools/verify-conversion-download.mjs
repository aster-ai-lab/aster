import { readFile, writeFile } from 'node:fs/promises'

import sharp from 'sharp'

const report = JSON.parse(await readFile(process.argv[2], 'utf8'))
if (report.status !== 'passed') throw new Error('Browser conversion campaign did not pass')
const matches = report.scenarios.flatMap((scenario) =>
  scenario.steps.flatMap((step) =>
    (step.output?.evidence ?? []).filter((artifact) => artifact.name === 'downloads/converted-page-1.png')
  )
)
if (matches.length !== 1) throw new Error('Expected one real conversion download')
const artifact = matches[0]
const metadata = await sharp(artifact.path, { limitInputPixels: 16_777_216 }).metadata()
if (metadata.format !== 'png' || metadata.width !== 200 || metadata.height !== 120) {
  throw new Error('Unexpected PDF conversion dimensions or format')
}
const { data, info } = await sharp(artifact.path).ensureAlpha().raw().toBuffer({ resolveWithObject: true })
if (info.channels !== 4) throw new Error('Unexpected output channels')
for (let pixel = 0; pixel < data.length; pixel += 4) {
  if (data[pixel] !== 255 || data[pixel + 1] !== 0 || data[pixel + 2] !== 0 || data[pixel + 3] !== 255) {
    throw new Error('Downloaded PDF page pixels differ from the source page')
  }
}
await writeFile(
  process.argv[3],
  JSON.stringify(
    {
      artifact: artifact.path,
      width: info.width,
      height: info.height,
      expected_rgba: [255, 0, 0, 255],
      all_pixels: 'passed'
    },
    null,
    2
  ) + '\n'
)
