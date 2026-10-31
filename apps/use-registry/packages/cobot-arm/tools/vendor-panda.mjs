// Vendor the Franka Emika Panda model into ui/workbench/vendor/panda.
//
//   node tools/vendor-panda.mjs <mujoco_menagerie>/franka_emika_panda
//
// The source is mujoco_menagerie commit 822c2d8f877dd166c5b7d3c9f7e3c3b6589473b7
// (Apache-2.0). The view policy has connect-src 'none', so the page cannot
// fetch a mesh file. panda.js carries everything as one gzip pack in base64:
//
// - PHYSICS_XML: panda.xml without the visual geoms, the visual mesh assets and
//   the keyframe, with timestep 0.002, collision friction 1.8 .02 .002 and a
//   tcp site 0.1034 m below the hand. Every body keeps its explicit inertial, so
//   dropping the visual geoms does not change the dynamics. The page inserts the
//   task geometry at the <!-- aster:world --> marker.
// - FILES: the collision meshes, byte for byte, for MuJoCo's virtual file system.
// - MESHES and VISUALS: the visual OBJ meshes for three.js, drawn in their body
//   frame (the visual geoms have no pos or quat). Positions are deduplicated and
//   quantized to 16 bits over each mesh's bounding box.
//
// The script refuses an input whose structure differs from the one it expects.
import { createHash } from 'node:crypto'
import { copyFileSync, mkdirSync, readFileSync, writeFileSync } from 'node:fs'
import { join } from 'node:path'
import { gzipSync } from 'node:zlib'

const source = process.argv[2]
if (!source) {
  console.error('usage: node tools/vendor-panda.mjs <franka_emika_panda directory>')
  process.exit(2)
}
const target = new URL('../ui/workbench/vendor/panda/', import.meta.url)
mkdirSync(target, { recursive: true })
const digest = (bytes) => createHash('sha256').update(bytes).digest('hex')
const fail = (message) => {
  throw new Error(message)
}

const upstream = readFileSync(join(source, 'panda.xml'), 'utf8')
const attribute = (line, name) => new RegExp(`\\b${name}="([^"]*)"`).exec(line)?.[1]

const materials = {}
const visuals = []
const keptFiles = []
const bodies = []
const out = []
let collisionDepth = 0
let defaultDepth = 0
let inKeyframe = false
let tcp = false
let marker = false

for (const line of upstream.split('\n')) {
  const trimmed = line.trim()
  if (trimmed.startsWith('<keyframe>')) {
    inKeyframe = true
    continue
  }
  if (inKeyframe) {
    if (trimmed.startsWith('</keyframe>')) inKeyframe = false
    continue
  }
  if (trimmed.startsWith('<mujoco ')) {
    out.push('<mujoco model="aster cobot-arm">')
    continue
  }
  if (trimmed.startsWith('<compiler ')) {
    if (!attribute(trimmed, 'meshdir')) fail('compiler has no meshdir')
    out.push(line.replace(/\s+meshdir="[^"]*"/, ''))
    continue
  }
  if (trimmed.startsWith('<option ')) {
    out.push(line.replace('<option ', '<option timestep="0.002" '))
    continue
  }
  if (trimmed.startsWith('<default')) {
    if (!trimmed.endsWith('/>')) defaultDepth += 1
    if (trimmed.startsWith('<default class="collision">')) collisionDepth = defaultDepth
  } else if (trimmed.startsWith('</default>')) {
    if (defaultDepth === collisionDepth) collisionDepth = 0
    defaultDepth -= 1
  }
  if (collisionDepth && trimmed.startsWith('<geom ')) {
    out.push(line.replace('<geom ', '<geom friction="1.8 .02 .002" '))
    continue
  }
  if (trimmed.startsWith('<material ') && attribute(trimmed, 'name')) {
    const rgba = attribute(trimmed, 'rgba') ?? fail(`material without rgba: ${trimmed}`)
    materials[attribute(trimmed, 'name')] = rgba.split(' ').map(Number)
  }
  if (trimmed.startsWith('<mesh ')) {
    const file = attribute(trimmed, 'file')
    const visual = !attribute(trimmed, 'name')
    if (visual && file !== 'finger_0.obj') continue
    keptFiles.push(file)
  }
  if (trimmed.startsWith('<body ')) bodies.push(attribute(trimmed, 'name'))
  if (trimmed.startsWith('</body>')) bodies.pop()
  if (trimmed.includes('class="visual"') && trimmed.startsWith('<geom ')) {
    if (attribute(trimmed, 'pos') || attribute(trimmed, 'quat')) fail(`visual geom has a pose: ${trimmed}`)
    const material = attribute(trimmed, 'material')
    if (!materials[material]) fail(`unknown material ${material}`)
    visuals.push({ body: bodies.at(-1), mesh: attribute(trimmed, 'mesh'), rgba: materials[material] })
    continue
  }
  out.push(line)
  if (bodies.at(-1) === 'hand' && trimmed.startsWith('<inertial ')) {
    const indent = line.slice(0, line.length - line.trimStart().length)
    out.push(`${indent}<site name="tcp" pos="0 0 0.1034" size=".004" rgba="0 0 0 0"/>`)
    tcp = true
  }
  if (trimmed.startsWith('<light ') && bodies.length === 0) {
    out.push('    <!-- aster:world -->')
    marker = true
  }
}
if (!tcp || !marker || inKeyframe || visuals.length !== 58) fail(`unexpected panda.xml (${visuals.length} visual geoms)`)
const physics = out.join('\n')
const leftover = /<geom [^>]*class="visual"|<keyframe|meshdir/.exec(physics)
if (leftover) fail(`physics XML still has visual or keyframe content: ${physics.slice(leftover.index - 80, leftover.index + 80)}`)

// Visual meshes: dedup positions, quantize, pack indices.
const chunks = []
let offset = 0
const append = (bytes) => {
  const pad = (4 - (offset % 4)) % 4
  if (pad) {
    chunks.push(new Uint8Array(pad))
    offset += pad
  }
  const at = offset
  chunks.push(new Uint8Array(bytes.buffer, bytes.byteOffset, bytes.byteLength))
  offset += bytes.byteLength
  return at
}

const meshes = {}
for (const name of new Set(visuals.map((item) => item.mesh))) {
  const text = readFileSync(join(source, 'assets', `${name}.obj`), 'utf8')
  const raw = []
  const faces = []
  for (const line of text.split('\n')) {
    if (line.startsWith('v ')) raw.push(line.slice(2).trim().split(/\s+/).map(Number))
    else if (line.startsWith('f ')) {
      const corners = line.slice(2).trim().split(/\s+/).map((part) => Number(part.split('/')[0]) - 1)
      for (let i = 1; i + 1 < corners.length; i += 1) faces.push(corners[0], corners[i], corners[i + 1])
    }
  }
  const index = new Map()
  const unique = []
  const remap = raw.map((point) => {
    const key = point.join(' ')
    let at = index.get(key)
    if (at === undefined) {
      at = unique.length
      index.set(key, at)
      unique.push(point)
    }
    return at
  })
  const min = [0, 1, 2].map((axis) => Math.min(...unique.map((point) => point[axis])))
  const max = [0, 1, 2].map((axis) => Math.max(...unique.map((point) => point[axis])))
  const step = min.map((low, axis) => (max[axis] - low) / 65535 || 1)
  const positions = new Uint16Array(unique.length * 3)
  unique.forEach((point, at) => {
    for (let axis = 0; axis < 3; axis += 1) positions[at * 3 + axis] = Math.round((point[axis] - min[axis]) / step[axis])
  })
  const wide = unique.length > 65535
  const indices = wide ? new Uint32Array(faces.length) : new Uint16Array(faces.length)
  faces.forEach((corner, at) => {
    if (!(corner >= 0 && corner < remap.length)) fail(`${name}: face index out of range`)
    indices[at] = remap[corner]
  })
  meshes[name] = {
    vertices: unique.length,
    indices: faces.length,
    wide,
    min,
    step,
    positions: append(positions),
    faces: append(indices)
  }
}

const files = {}
for (const file of keptFiles) {
  const bytes = readFileSync(join(source, 'assets', file))
  files[file] = { offset: append(bytes), length: bytes.length }
}

const blob = Buffer.concat(chunks.map((chunk) => Buffer.from(chunk)))
const packed = gzipSync(blob, { level: 9 }).toString('base64')
const module = [
  '// Generated by tools/vendor-panda.mjs from mujoco_menagerie franka_emika_panda (Apache-2.0).',
  `export const PHYSICS_XML = ${JSON.stringify(physics)}`,
  `export const VISUALS = ${JSON.stringify(visuals)}`,
  `export const MESHES = ${JSON.stringify(meshes)}`,
  `export const FILES = ${JSON.stringify(files)}`,
  `export const PACK = ${JSON.stringify(packed)}`,
  ''
].join('\n')
writeFileSync(new URL('panda.js', target), module)
copyFileSync(join(source, 'LICENSE'), new URL('LICENSE', target))

console.log(`upstream panda.xml ${digest(upstream)}`)
console.log(`visual meshes ${Object.keys(meshes).length}, geoms ${visuals.length}, collision files ${keptFiles.length}`)
console.log(`pack ${blob.length} bytes raw, panda.js ${module.length} bytes`)
