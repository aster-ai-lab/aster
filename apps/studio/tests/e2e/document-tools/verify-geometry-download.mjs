import { readFile, writeFile } from 'node:fs/promises'

import sharp from 'sharp'

const report = JSON.parse(await readFile(process.argv[2], 'utf8'))
if (report.status !== 'passed') throw new Error('Browser geometry campaign did not pass')
const evidence = report.scenarios.flatMap((scenario) => scenario.steps.flatMap((step) => step.output?.evidence ?? []))
const receipts = []
for (const name of ['crop', 'perspective']) {
  const matches = evidence.filter((artifact) => artifact.name === `downloads/geometry-${name}.png`)
  if (matches.length !== 1) throw new Error(`Expected one real ${name} download`)
  const artifact = matches[0]
  const image = sharp(artifact.path, { limitInputPixels: 16_777_216 })
  const metadata = await image.metadata()
  const { data, info } = await image.ensureAlpha().raw().toBuffer({ resolveWithObject: true })
  const expectedSize = name === 'crop' ? [6, 4] : [12, 8]
  if (
    metadata.format !== 'png' ||
    info.channels !== 4 ||
    info.width !== expectedSize[0] ||
    info.height !== expectedSize[1]
  ) {
    throw new Error(`Unexpected ${name} dimensions or format`)
  }
  if (name === 'crop') {
    for (let y = 0; y < 4; y++)
      for (let x = 0; x < 6; x++) {
        const expected = [(x + 2) * 16, (y + 1) * 24, 100, 255]
        if (expected.some((value, channel) => data[(y * 6 + x) * 4 + channel] !== value)) {
          throw new Error(`Crop changed original source pixels at ${x},${y}`)
        }
      }
  } else {
    // Independent homography for TL(1,0), TR(12,0), BR(12,8), BL(0,8):
    // at u=13/24,v=9/16, source=(6.21038,3.82787) after half-pixel offset.
    // Original gradient channels are x*16,y*24,100,255; bilinear rounds to these.
    const actual = [...data.subarray((4 * 12 + 6) * 4, (4 * 12 + 6) * 4 + 4)]
    if (actual.join() !== '99,92,100,255') throw new Error(`Wrong projective source sample: ${actual}`)
  }
  receipts.push({
    artifact: artifact.path,
    width: info.width,
    height: info.height,
    verification:
      name === 'crop' ? 'every original-coordinate crop pixel passed' : 'independent projective sample passed'
  })
}
await writeFile(process.argv[3], JSON.stringify(receipts, null, 2) + '\n')
