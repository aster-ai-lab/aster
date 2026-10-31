// Vendor the Unitree G1 tabletop station into ui/workbench/vendor/g1.
//
//   node tools/vendor-g1.mjs <assets root>
//
// The assets root holds g1/ (MuJoCo Menagerie unitree_g1: g1_with_hands.xml,
// assets/*.STL, LICENSE) and panda/ (Menagerie franka_emika_panda: panda.xml,
// assets/, LICENSE), the layout of the EmbodiedJev G1 integration package.
// The scene follows that package's G1Spec.prepare and verify_g1_grasp.py:
//
// - The pelvis is welded at (0.12, 0.10, 0.05), yaw +0.35 rad. The freejoint
//   and the keyframe are removed.
// - The pelvis, legs, left arm and left Dex3-1 hand are hidden and never
//   collide. Upstream gives their geoms group 3 and contype/conaffinity 0; here
//   the geoms are removed. Every body keeps its explicit inertial, so the
//   dynamics do not change.
// - The right Dex3-1 hand (fingers, palm geoms, right_hand_* actuators) is
//   replaced by the Panda hand: defaults with a ph_ prefix, tendon split,
//   finger equality, actuator8 (ctrl 0 closed, 255 open), mounted on
//   right_wrist_yaw_link at 0.046 0 0 so hand +Z is wrist +X. A tcp site sits
//   0.1034 m along hand +Z, as on the Panda.
// - Right arm and waist_pitch position actuators get kp 1500.
// - Timestep 0.002. The page inserts the task scene at <!-- aster:world -->.
//
// The view policy has connect-src 'none', so g1.js carries everything as one
// gzip pack in base64:
//
// - PHYSICS_XML: the scene above without visual geoms.
// - MESHES and VISUALS: the visual meshes for three.js. Positions are
//   deduplicated and quantized to 16 bits over each mesh's bounding box.
//   VISUALS gives each drawn geom its body, mesh, colour and local pose.
// - G1 collision meshes: MuJoCo collides against a mesh geom's convex hull,
//   so each G1 collision mesh is written into PHYSICS_XML as the vertices of
//   the hull MuJoCo itself builds from the STL corners (original float32
//   coordinates, no faces). The script checks that MuJoCo rebuilds the same
//   hull from them. This keeps tens of megabytes of STL out of the page and
//   out of MuJoCo's virtual file system, whose copy is slow in WASM.
// - FILES: the Panda hand collision meshes, byte for byte.
//
// The script refuses an input whose structure differs from the one it expects.
import { createHash } from 'node:crypto'
import { copyFileSync, mkdirSync, readFileSync, writeFileSync } from 'node:fs'
import { basename, extname, join } from 'node:path'
import { gzipSync } from 'node:zlib'

import { loadMujoco } from '../ui/workbench/engine.js'

const mujoco = await loadMujoco()

const root = process.argv[2]
if (!root) {
  console.error('usage: node tools/vendor-g1.mjs <assets root with g1/ and panda/>')
  process.exit(2)
}
const target = new URL('../ui/workbench/vendor/g1/', import.meta.url)
mkdirSync(target, { recursive: true })
const digest = (bytes) => createHash('sha256').update(bytes).digest('hex')
const fail = (message) => {
  throw new Error(message)
}

// A small MJCF reader: elements and attributes only. MJCF carries no text content.
function parse(text, label) {
  const top = { name: '#document', attrs: new Map(), children: [] }
  const stack = [top]
  const token = /<!--[\s\S]*?-->|<\?[\s\S]*?\?>|<(\/?)([A-Za-z_][\w.-]*)((?:\s+[\w:.-]+\s*=\s*"[^"]*")*)\s*(\/?)>/g
  let at = 0
  for (let match = token.exec(text); match; match = token.exec(text)) {
    if (text.slice(at, match.index).trim()) fail(`${label}: text content before ${match[0].slice(0, 40)}`)
    at = token.lastIndex
    if (match[0].startsWith('<!--') || match[0].startsWith('<?')) continue
    const [, close, name, attributes, selfClosing] = match
    if (close) {
      if (stack.pop().name !== name) fail(`${label}: mismatched </${name}>`)
      continue
    }
    const attrs = new Map()
    for (const [, key, value] of attributes.matchAll(/([\w:.-]+)\s*=\s*"([^"]*)"/g)) attrs.set(key, value)
    const element = { name, attrs, children: [] }
    stack.at(-1).children.push(element)
    if (!selfClosing) stack.push(element)
  }
  if (stack.length !== 1 || text.slice(at).trim()) fail(`${label}: unbalanced document`)
  if (top.children.length !== 1) fail(`${label}: expected one root element`)
  return top.children[0]
}

function serialize(element, depth = 0) {
  const pad = '  '.repeat(depth)
  if (element.comment) return `${pad}<!-- ${element.comment} -->`
  const attrs = [...element.attrs].map(([key, value]) => ` ${key}="${value}"`).join('')
  if (!element.children.length) return `${pad}<${element.name}${attrs}/>`
  return [`${pad}<${element.name}${attrs}>`, ...element.children.map((child) => serialize(child, depth + 1)), `${pad}</${element.name}>`].join('\n')
}

const node = (name, attrs = {}, children = []) => ({ name, attrs: new Map(Object.entries(attrs)), children })
const child = (element, name) => element.children.find((item) => item.name === name)
const children = (element, name) => element.children.filter((item) => item.name === name)
function* walk(element) {
  yield element
  for (const item of element.children) yield* walk(item)
}
const bodiesOf = (element) => new Map([...walk(element)].filter((item) => item.name === 'body').map((item) => [item.attrs.get('name'), item]))
const remove = (parent, element) => {
  const index = parent.children.indexOf(element)
  if (index < 0) fail(`no ${element.name} under ${parent.name}`)
  parent.children.splice(index, 1)
}

const ARM = [
  'right_shoulder_pitch_joint', 'right_shoulder_roll_joint', 'right_shoulder_yaw_joint', 'right_elbow_joint',
  'right_wrist_roll_joint', 'right_wrist_pitch_joint', 'right_wrist_yaw_joint'
]
const HIDDEN = [
  'pelvis',
  'left_hip_pitch_link', 'left_hip_roll_link', 'left_hip_yaw_link', 'left_knee_link', 'left_ankle_pitch_link', 'left_ankle_roll_link',
  'right_hip_pitch_link', 'right_hip_roll_link', 'right_hip_yaw_link', 'right_knee_link', 'right_ankle_pitch_link', 'right_ankle_roll_link',
  'left_shoulder_pitch_link', 'left_shoulder_roll_link', 'left_shoulder_yaw_link', 'left_elbow_link',
  'left_wrist_roll_link', 'left_wrist_pitch_link', 'left_wrist_yaw_link',
  'left_hand_thumb_0_link', 'left_hand_thumb_1_link', 'left_hand_thumb_2_link',
  'left_hand_index_0_link', 'left_hand_index_1_link', 'left_hand_middle_0_link', 'left_hand_middle_1_link'
]
const DEX3_RIGHT = ['right_hand_thumb_0_link', 'right_hand_index_0_link', 'right_hand_middle_0_link']
const PANDA_DEFAULTS = `<default class="ph_root">
  <default class="ph_visual"><geom type="mesh" contype="0" conaffinity="0" group="2"/></default>
  <default class="ph_collision"><geom type="mesh" group="3" friction="1.8 .02 .002"/></default>
  <default class="ph_finger"><joint axis="0 1 0" type="slide" range="0 0.04" armature="0.1" damping="1"/></default>
  <default class="ph_pad1"><geom type="box" group="3" size="0.0085 0.004 0.0085" pos="0 0.0055 0.0445" friction="1.8 .02 .002"/></default>
  <default class="ph_pad2"><geom type="box" group="3" size="0.003 0.002 0.003" pos="0.0055 0.002 0.05" friction="1.8 .02 .002"/></default>
  <default class="ph_pad3"><geom type="box" group="3" size="0.003 0.002 0.003" pos="-0.0055 0.002 0.05" friction="1.8 .02 .002"/></default>
  <default class="ph_pad4"><geom type="box" group="3" size="0.003 0.002 0.0035" pos="0.0055 0.002 0.0395" friction="1.8 .02 .002"/></default>
  <default class="ph_pad5"><geom type="box" group="3" size="0.003 0.002 0.0035" pos="-0.0055 0.002 0.0395" friction="1.8 .02 .002"/></default>
  <default class="ph_act"><general dyntype="none" biastype="affine"/></default>
</default>`
const PANDA_CLASSES = {
  visual: 'ph_visual', collision: 'ph_collision', finger: 'ph_finger',
  fingertip_pad_collision_1: 'ph_pad1', fingertip_pad_collision_2: 'ph_pad2', fingertip_pad_collision_3: 'ph_pad3',
  fingertip_pad_collision_4: 'ph_pad4', fingertip_pad_collision_5: 'ph_pad5'
}
// Panda hand meshes the scene needs: collision files, then the visual OBJ meshes three.js draws.
const PANDA_COLLISION = { hand_c: 'hand.stl', finger_0: 'finger_0.obj' }

const g1Text = readFileSync(join(root, 'g1', 'g1_with_hands.xml'), 'utf8')
const pandaText = readFileSync(join(root, 'panda', 'panda.xml'), 'utf8')
const g1 = parse(g1Text, 'g1_with_hands.xml')
const panda = parse(pandaText, 'panda.xml')

const rgbaOf = (materials, name) => materials.get(name) ?? fail(`unknown material ${name}`)
const materialsOf = (document) =>
  new Map(children(child(document, 'asset'), 'material').map((item) => [item.attrs.get('name'), item.attrs.get('rgba').split(' ').map(Number)]))
const g1Materials = materialsOf(g1)
const pandaMaterials = materialsOf(panda)

// The G1 default visual class sets material="metal"; a geom may override it.
const g1VisualDefault = [...walk(child(g1, 'default'))].find((item) => item.name === 'default' && item.attrs.get('class') === 'visual')
const g1VisualMaterial = child(g1VisualDefault ?? fail('g1 has no visual default'), 'geom').attrs.get('material')

g1.attrs.set('model', 'aster g1-arm')
const compiler = child(g1, 'compiler')
if (!compiler.attrs.has('meshdir')) fail('compiler has no meshdir')
compiler.attrs.delete('meshdir')
child(g1, 'option').attrs.set('timestep', '0.002')
remove(g1, child(g1, 'keyframe') ?? fail('g1 has no keyframe'))

const world = child(g1, 'worldbody')
const bodies = bodiesOf(g1)
for (const name of [...HIDDEN, ...DEX3_RIGHT, 'right_wrist_yaw_link', 'torso_link']) if (!bodies.has(name)) fail(`g1 has no body ${name}`)
for (const body of bodies.values()) if (!child(body, 'inertial')) fail(`${body.attrs.get('name')} has no explicit inertial`)
const pelvis = bodies.get('pelvis')
remove(pelvis, child(pelvis, 'freejoint') ?? fail('pelvis has no freejoint'))
pelvis.attrs.set('pos', '0.12 0.1 0.05')
pelvis.attrs.set('quat', '0.9847296 0 0 0.1741106')
for (const name of HIDDEN) {
  const body = bodies.get(name)
  body.children = body.children.filter((item) => item.name !== 'geom')
}
const wrist = bodies.get('right_wrist_yaw_link')
for (const name of DEX3_RIGHT) remove(wrist, bodies.get(name))
const palms = children(wrist, 'geom').filter((item) => (item.attrs.get('mesh') ?? '').startsWith('right_hand_palm'))
if (palms.length !== 2) fail(`expected two right palm geoms, found ${palms.length}`)
for (const geom of palms) remove(wrist, geom)

const actuators = child(g1, 'actuator')
actuators.children = actuators.children.filter((item) => !(item.attrs.get('name') ?? '').startsWith('right_hand_'))
const stiff = new Set([...ARM, 'waist_pitch_joint'])
let stiffened = 0
for (const item of children(actuators, 'position')) {
  if (stiff.has(item.attrs.get('joint'))) {
    item.attrs.set('kp', '1500')
    stiffened += 1
  }
}
if (stiffened !== 8) fail(`expected 8 stiffened actuators, found ${stiffened}`)

// Visible G1 visual geoms go to three.js; their collision twins stay in physics.
const visuals = []
const pose = (geom) => {
  const out = {}
  if (geom.attrs.has('pos')) out.pos = geom.attrs.get('pos').split(/\s+/).map(Number)
  if (geom.attrs.has('quat')) out.quat = geom.attrs.get('quat').split(/\s+/).map(Number)
  return out
}
for (const [name, body] of bodiesOf(g1)) {
  for (const geom of children(body, 'geom')) {
    if (geom.attrs.get('class') !== 'visual') continue
    const material = geom.attrs.get('material') ?? g1VisualMaterial
    visuals.push({ body: name, mesh: geom.attrs.get('mesh'), rgba: rgbaOf(g1Materials, material), ...pose(geom) })
    remove(body, geom)
  }
}

// Mesh assets: keep only meshes a collision geom still uses, as their hull vertices.
const asset = child(g1, 'asset')
const meshName = (item) => item.attrs.get('name') ?? basename(item.attrs.get('file'), extname(item.attrs.get('file')))
const g1MeshFiles = new Map(children(asset, 'mesh').map((item) => [meshName(item), item.attrs.get('file')]))
const collisionMeshes = new Set(
  [...walk(world)].filter((item) => item.name === 'geom' && item.attrs.has('mesh')).map((item) => item.attrs.get('mesh'))
)
asset.children = asset.children.filter((item) => item.name !== 'mesh' || collisionMeshes.has(meshName(item)))
const hulls = {}
for (const item of children(asset, 'mesh')) {
  const name = meshName(item)
  const file = item.attrs.get('file')
  const { raw } = readStl(readFileSync(join(root, 'g1', 'assets', file)), file)
  const hull = hullVertices(raw, file)
  hulls[name] = { points: raw.length, vertices: hull.length }
  item.attrs = new Map([['name', name], ['vertex', hull.join('  ')]])
}
for (const [name, file] of Object.entries(PANDA_COLLISION)) asset.children.push(node('mesh', { name, file }))

child(g1, 'default').children.push(parse(PANDA_DEFAULTS, 'Panda hand defaults'))
const tendon = child(g1, 'tendon') ?? (g1.children.push(node('tendon')), child(g1, 'tendon'))
tendon.children.push(
  node('fixed', { name: 'split' }, [
    node('joint', { joint: 'finger_joint1', coef: '0.5' }),
    node('joint', { joint: 'finger_joint2', coef: '0.5' })
  ])
)
const equality = child(g1, 'equality') ?? (g1.children.push(node('equality')), child(g1, 'equality'))
equality.children.push(node('joint', { joint1: 'finger_joint1', joint2: 'finger_joint2', solimp: '0.95 0.99 0.001', solref: '0.005 1' }))
actuators.children.push(
  node('general', {
    class: 'ph_act', name: 'actuator8', tendon: 'split', forcerange: '-100 100', ctrlrange: '0 255',
    gainprm: '0.01568627451 0 0', biasprm: '0 -100 -10'
  })
)

// The Panda hand, mounted so hand +Z (finger direction) is wrist +X.
const hand = bodiesOf(panda).get('hand') ?? fail('panda.xml has no hand')
hand.attrs.set('pos', '0.046 0 0')
hand.attrs.set('quat', '0.70710678 0 0.70710678 0')
const pandaVisualMeshes = new Set()
for (const [name, body] of bodiesOf(hand)) {
  for (const element of [...body.children]) {
    const klass = element.attrs.get('class')
    if (element.name === 'geom' && klass === 'visual') {
      if (element.attrs.has('pos') || element.attrs.has('quat')) fail(`Panda visual geom has a pose in ${name}`)
      visuals.push({ body: name, mesh: element.attrs.get('mesh'), rgba: rgbaOf(pandaMaterials, element.attrs.get('material')) })
      pandaVisualMeshes.add(element.attrs.get('mesh'))
      remove(body, element)
      continue
    }
    if (klass !== undefined) element.attrs.set('class', PANDA_CLASSES[klass] ?? fail(`unmapped Panda class ${klass}`))
    if (element.name === 'geom' && element.attrs.has('mesh') && !(element.attrs.get('mesh') in PANDA_COLLISION)) {
      fail(`unexpected Panda collision mesh ${element.attrs.get('mesh')}`)
    }
  }
}
hand.children.push(node('site', { name: 'tcp', pos: '0 0 0.1034', size: '.004', rgba: '0 0 0 0' }))
wrist.children.push(hand)

const light = world.children.findIndex((item) => item.name === 'light')
if (light < 0) fail('g1 worldbody has no light')
world.children.splice(light + 1, 0, { comment: 'aster:world', attrs: new Map(), children: [] })

const physics = serialize(g1)
const leftover = /<geom [^>]*class="(?:ph_)?visual"|<keyframe|meshdir|freejoint|right_hand_/.exec(physics)
if (leftover) fail(`physics XML still has removed content: ${leftover[0]} at ${physics.slice(leftover.index - 80, leftover.index + 80)}`)

// Pack: dedup positions, quantize, append indices.
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

function readStl(bytes, label) {
  const count = bytes.readUInt32LE(80)
  if (bytes.length !== 84 + count * 50) fail(`${label}: not a binary STL`)
  const raw = []
  for (let t = 0; t < count; t += 1) {
    for (let v = 0; v < 3; v += 1) {
      const at = 84 + t * 50 + 12 + v * 12
      raw.push([bytes.readFloatLE(at), bytes.readFloatLE(at + 4), bytes.readFloatLE(at + 8)])
    }
  }
  return { raw, faces: raw.map((_, i) => i) }
}

/** The hull MuJoCo builds for a vertex-only mesh: vertex count, face count and geom bounds. */
function compileHull(vertices) {
  const xml = `<mujoco><asset><mesh name="m" vertex="${vertices.join('  ')}"/></asset><worldbody><body><geom type="mesh" mesh="m"/></body></worldbody></mujoco>`
  const model = mujoco.MjModel.from_xml_string(xml)
  try {
    if (model.nmeshvert !== vertices.length) fail('MuJoCo changed the vertex list')
    const at = model.mesh_graphadr[0]
    const graph = model.mesh_graph
    const count = graph[at]
    return {
      ids: Array.from(graph.subarray(at + 2 + count, at + 2 + 2 * count)),
      faces: graph[at + 1],
      aabb: Array.from(model.geom_aabb.subarray(0, 6)),
      rbound: model.geom_rbound[0]
    }
  } finally {
    model.delete()
  }
}

/**
 * The vertices of MuJoCo's convex hull of an STL's points, as decimal text.
 * Recompiling only these vertices must give the same hull and bounds.
 */
function hullVertices(raw, label) {
  const unique = [...new Set(raw.map((point) => point.map(coordinate).join(' ')))]
  const full = compileHull(unique)
  const kept = [...new Set(full.ids)].sort((a, b) => a - b).map((id) => unique[id])
  const reduced = compileHull(kept)
  const same = (a, b) => Math.abs(a - b) <= 1e-9
  if (reduced.ids.length !== full.ids.length || reduced.faces !== full.faces) fail(`${label}: the reduced hull differs`)
  if (!same(reduced.rbound, full.rbound) || !reduced.aabb.every((value, i) => same(value, full.aabb[i]))) fail(`${label}: the reduced hull bounds differ`)
  return kept
}

/** The shortest decimal that round-trips the float32 STL coordinate. */
function coordinate(value) {
  for (let digits = 6; digits < 10; digits += 1) {
    const text = String(Number(value.toPrecision(digits)))
    if (Math.fround(Number(text)) === value) return text
  }
  return String(value)
}

function readObj(text) {
  const raw = []
  const faces = []
  for (const line of text.split('\n')) {
    if (line.startsWith('v ')) raw.push(line.slice(2).trim().split(/\s+/).map(Number))
    else if (line.startsWith('f ')) {
      const corners = line.slice(2).trim().split(/\s+/).map((part) => Number(part.split('/')[0]) - 1)
      for (let i = 1; i + 1 < corners.length; i += 1) faces.push(corners[0], corners[i], corners[i + 1])
    }
  }
  return { raw, faces }
}

function pack(name, { raw, faces }) {
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
  const min = [0, 1, 2].map((axis) => unique.reduce((low, point) => Math.min(low, point[axis]), Infinity))
  const max = [0, 1, 2].map((axis) => unique.reduce((high, point) => Math.max(high, point[axis]), -Infinity))
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
  return { vertices: unique.length, indices: faces.length, wide, min, step, positions: append(positions), faces: append(indices) }
}

const meshes = {}
const g1Needed = new Set(visuals.filter((item) => !pandaVisualMeshes.has(item.mesh)).map((item) => item.mesh))
for (const name of [...g1Needed].sort()) {
  const file = g1MeshFiles.get(name) ?? fail(`g1 has no mesh ${name}`)
  meshes[name] = pack(name, readStl(readFileSync(join(root, 'g1', 'assets', file)), file))
}
for (const name of [...pandaVisualMeshes].sort()) {
  meshes[name] = pack(name, readObj(readFileSync(join(root, 'panda', 'assets', `${name}.obj`), 'utf8')))
}
const files = {}
for (const file of Object.values(PANDA_COLLISION)) {
  const bytes = readFileSync(join(root, 'panda', 'assets', file))
  files[file] = { offset: append(bytes), length: bytes.length }
}

const blob = Buffer.concat(chunks.map((chunk) => Buffer.from(chunk)))
const packed = gzipSync(blob, { level: 9 }).toString('base64')
const module = [
  '// Generated by tools/vendor-g1.mjs from MuJoCo Menagerie unitree_g1 (BSD-3-Clause, Unitree Robotics)',
  '// and franka_emika_panda (Apache-2.0).',
  `export const PHYSICS_XML = ${JSON.stringify(physics)}`,
  `export const VISUALS = ${JSON.stringify(visuals)}`,
  `export const MESHES = ${JSON.stringify(meshes)}`,
  `export const FILES = ${JSON.stringify(files)}`,
  `export const PACK = ${JSON.stringify(packed)}`,
  ''
].join('\n')
writeFileSync(new URL('g1.js', target), module)
copyFileSync(join(root, 'g1', 'LICENSE'), new URL('LICENSE', target))
copyFileSync(join(root, 'panda', 'LICENSE'), new URL('LICENSE-panda', target))

console.log(`upstream g1_with_hands.xml ${digest(g1Text)}`)
console.log(`upstream panda.xml ${digest(pandaText)}`)
console.log(`visual meshes ${Object.keys(meshes).length}, geoms ${visuals.length}`)
for (const [name, { points, vertices }] of Object.entries(hulls)) console.log(`collision hull ${name}: ${vertices} of ${points} STL corners`)
console.log(`pack ${blob.length} bytes raw, g1.js ${module.length} bytes`)
