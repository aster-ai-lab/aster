// Franka Panda tabletop world on MuJoCo WASM. Ported from EmbodiedJev
// physics.py (MIT): same scene, IK, motion profile, contact rules and
// success test. The scene geometry is built from the vendored panda.xml.
import { inflate } from './engine.js'
import { FILES, PACK, PHYSICS_XML } from './vendor/panda/panda.js'

export const HOME = [0, -0.45, 0, -2.15, 0, 1.75, 0.7854]
export const DT = 0.002
export const TRAVEL_Z = 0.22
export const TASKS = {
  transfer: { name: '搬运入盘', goal: '把红色方块放进蓝色托盘，松开夹爪并向上撤离。', target_z: 0.026 },
  stack: { name: '方块堆叠', goal: '把红色方块叠在蓝色方块上，松开夹爪并向上撤离。', target_z: 0.06 },
  barrier: { name: '越障搬运', goal: '越过中间的障碍，把红色方块放进蓝色托盘，松开夹爪并撤离。', target_z: 0.026 }
}
const TARGET_XY = [0.43, 0.18]
const SOURCE = [0.43, -0.17, 0.021]
const SOURCE_JITTER = 0.025
const WORKSPACE = [
  [0.2, -0.32, 0.02],
  [0.65, 0.34, 0.42]
]
const FORBIDDEN_BODIES = ['hand', 'left_finger', 'right_finger', 'link5', 'link6', 'link7']
const OBJ = { BODY: 1, JOINT: 3, GEOM: 5, SITE: 6 }
const DOWN_QUAT = [0, 1, 0, 0]

let packBytes = null

/** The vendored Panda pack, gunzipped once per page. */
export function pandaPack() {
  packBytes ??= inflate(PACK)
  return packBytes
}

/** A small deterministic generator, so one seed always gives the same scene. */
function random(seed) {
  let state = (seed >>> 0) + 0x6d2b79f5
  return () => {
    state = (state + 0x6d2b79f5) >>> 0
    let value = state
    value = Math.imul(value ^ (value >>> 15), value | 1)
    value ^= value + Math.imul(value ^ (value >>> 7), value | 61)
    return ((value ^ (value >>> 14)) >>> 0) / 4294967296
  }
}

const vec = (values) => values.map((value) => Number(value.toFixed(6))).join(' ')

export function buildScene(task, seed) {
  if (!TASKS[task]) throw new Error('Unknown task')
  const next = random(seed)
  const source = [...SOURCE]
  source[0] += (next() * 2 - 1) * SOURCE_JITTER
  source[1] += (next() * 2 - 1) * SOURCE_JITTER
  const target = [...TARGET_XY, TASKS[task].target_z]
  const world = [
    '<geom name="table" type="box" pos=".32 0 -.025" size=".55 .43 .025" rgba=".81 .84 .85 1" friction="1 .01 .001"/>',
    `<body name="cube" pos="${vec(source)}"><freejoint name="cube_joint"/>` +
      '<geom name="cube_geom" type="box" size=".02 .02 .02" mass=".06" rgba=".88 .19 .19 1" friction="1.8 .02 .002" condim="6" solref=".006 1"/></body>'
  ]
  if (task === 'stack') {
    world.push('<geom name="support" type="box" pos=".43 .18 .02" size=".03 .03 .02" rgba=".10 .44 .72 1" friction="1.2 .01 .001"/>')
  } else {
    world.push('<geom name="support" type="box" pos=".43 .18 .003" size=".07 .07 .003" rgba=".10 .44 .72 1" friction="1.2 .01 .001"/>')
    const walls = [
      [0.354, 0.18, 0.006, 0.082],
      [0.506, 0.18, 0.006, 0.082],
      [0.43, 0.104, 0.07, 0.006],
      [0.43, 0.256, 0.07, 0.006]
    ]
    walls.forEach(([x, y, sx, sy], index) => {
      world.push(`<geom name="tray_wall_${index}" type="box" pos="${vec([x, y, 0.012])}" size="${vec([sx, sy, 0.012])}" rgba=".12 .47 .74 1"/>`)
    })
  }
  if (task === 'barrier') {
    world.push('<geom name="barrier" type="box" pos=".43 0 .055" size=".115 .018 .055" rgba=".93 .66 .15 1"/>')
  }
  const marker = '<!-- aster:world -->'
  if (PHYSICS_XML.split(marker).length !== 2) throw new Error('panda.xml has no world marker')
  return { xml: PHYSICS_XML.replace(marker, world.join('\n    ')), target, source }
}

// mju_mat2Quat, mju_mulQuat and mju_quat2Vel, as MuJoCo defines them.
function mat2Quat(m, at) {
  const [a, b, c, d, e, f, g, h, i] = [0, 1, 2, 3, 4, 5, 6, 7, 8].map((k) => m[at + k])
  let q
  if (a + e + i > 0) {
    const w = 0.5 * Math.sqrt(1 + a + e + i)
    q = [w, (0.25 * (h - f)) / w, (0.25 * (c - g)) / w, (0.25 * (d - b)) / w]
  } else if (a > e && a > i) {
    const x = 0.5 * Math.sqrt(1 + a - e - i)
    q = [(0.25 * (h - f)) / x, x, (0.25 * (b + d)) / x, (0.25 * (c + g)) / x]
  } else if (e > i) {
    const y = 0.5 * Math.sqrt(1 - a + e - i)
    q = [(0.25 * (c - g)) / y, (0.25 * (b + d)) / y, y, (0.25 * (f + h)) / y]
  } else {
    const z = 0.5 * Math.sqrt(1 - a - e + i)
    q = [(0.25 * (d - b)) / z, (0.25 * (c + g)) / z, (0.25 * (f + h)) / z, z]
  }
  const norm = Math.hypot(...q)
  return q.map((value) => value / norm)
}

function mulQuat(p, q) {
  return [
    p[0] * q[0] - p[1] * q[1] - p[2] * q[2] - p[3] * q[3],
    p[0] * q[1] + p[1] * q[0] + p[2] * q[3] - p[3] * q[2],
    p[0] * q[2] - p[1] * q[3] + p[2] * q[0] + p[3] * q[1],
    p[0] * q[3] + p[1] * q[2] - p[2] * q[1] + p[3] * q[0]
  ]
}

function quat2Vel(q) {
  const axis = [q[1], q[2], q[3]]
  const sin = Math.hypot(...axis)
  if (sin < 1e-15) return [0, 0, 0]
  let speed = 2 * Math.atan2(sin, q[0])
  if (speed > Math.PI) speed -= 2 * Math.PI
  return axis.map((value) => (value / sin) * speed)
}

/** Solve the 6x6 system a x = b by Gaussian elimination with partial pivoting. */
function solve6(a, b) {
  const n = 6
  const m = a.map((row, i) => [...row, b[i]])
  for (let col = 0; col < n; col += 1) {
    let pivot = col
    for (let row = col + 1; row < n; row += 1) if (Math.abs(m[row][col]) > Math.abs(m[pivot][col])) pivot = row
    ;[m[col], m[pivot]] = [m[pivot], m[col]]
    for (let row = col + 1; row < n; row += 1) {
      const factor = m[row][col] / m[col][col]
      for (let k = col; k <= n; k += 1) m[row][k] -= factor * m[col][k]
    }
  }
  const x = new Array(n).fill(0)
  for (let row = n - 1; row >= 0; row -= 1) {
    let sum = m[row][n]
    for (let k = row + 1; k < n; k += 1) sum -= m[row][k] * x[k]
    x[row] = sum / m[row][row]
  }
  return x
}

const distance = (a, b) => Math.hypot(...a.map((value, i) => value - b[i]))
const round = (value, digits) => Number(value.toFixed(digits))
const triple = (array, at) => [array[at * 3], array[at * 3 + 1], array[at * 3 + 2]]

export class RobotWorld {
  /** Build the task scene, settle the arm at its travel pose and run 250 ticks. */
  static async create(mujoco, task = 'transfer', seed = 0) {
    const pack = await pandaPack()
    const { xml, target, source } = buildScene(task, seed)
    const vfs = new mujoco.MjVFS()
    let model
    try {
      for (const [name, { offset, length }] of Object.entries(FILES)) vfs.addBuffer(name, pack.subarray(offset, offset + length))
      model = mujoco.MjModel.from_xml_string(xml, vfs)
    } finally {
      vfs.delete()
    }
    const world = new RobotWorld(mujoco, model, new mujoco.MjData(model), { task, seed, target, source })
    world.settle()
    return world
  }

  constructor(mujoco, model, data, { task, seed, target, source, shared }) {
    this.mujoco = mujoco
    this.model = model
    this.data = data
    this.task = task
    this.seed = seed
    this.target = target
    this.source = source
    this.shared = shared ?? this.index()
    this.closed = false
    this.stableSeconds = 0
    this.contactSeconds = 0
    this.maxLift = 0
    this.unsafeContacts = 0
    this.steps = 0
    this.startTime = 0
    this.scratch = null
  }

  index() {
    const { mujoco, model } = this
    const id = (type, name) => {
      const found = mujoco.mj_name2id(model, type, name)
      if (found < 0) throw new Error(`scene has no ${name}`)
      return found
    }
    const joints = [1, 2, 3, 4, 5, 6, 7].map((i) => id(OBJ.JOINT, `joint${i}`))
    const geomBody = Array.from(model.geom_bodyid)
    const forbiddenBodies = new Set(FORBIDDEN_BODIES.map((name) => id(OBJ.BODY, name)))
    const obstacles = new Set(['table', 'barrier'].map((name) => mujoco.mj_name2id(model, OBJ.GEOM, name)).filter((g) => g >= 0))
    const cubeJoint = id(OBJ.JOINT, 'cube_joint')
    return {
      armQ: joints.map((j) => model.jnt_qposadr[j]),
      armDof: joints.map((j) => model.jnt_dofadr[j]),
      ranges: joints.map((j) => [model.jnt_range[j * 2], model.jnt_range[j * 2 + 1]]),
      fingerQ: ['finger_joint1', 'finger_joint2'].map((name) => model.jnt_qposadr[id(OBJ.JOINT, name)]),
      tcp: id(OBJ.SITE, 'tcp'),
      cubeBody: id(OBJ.BODY, 'cube'),
      cubeGeom: id(OBJ.GEOM, 'cube_geom'),
      cubeDof: model.jnt_dofadr[cubeJoint],
      support: id(OBJ.GEOM, 'support'),
      left: id(OBJ.BODY, 'left_finger'),
      right: id(OBJ.BODY, 'right_finger'),
      geomBody,
      forbiddenBodies,
      obstacles
    }
  }

  settle() {
    const { data, mujoco, model, shared } = this
    shared.armQ.forEach((address, i) => {
      data.qpos[address] = HOME[i]
      data.ctrl[i] = HOME[i]
    })
    data.ctrl[7] = 255
    for (const address of shared.fingerQ) data.qpos[address] = 0.04
    mujoco.mj_forward(model, data)
    const { q, error } = this.solveIk([0.38, -0.08, TRAVEL_Z])
    if (error > 0.004) throw new Error(`Initial IK failed: ${error}`)
    shared.armQ.forEach((address, i) => {
      data.qpos[address] = q[i]
      data.ctrl[i] = q[i]
    })
    mujoco.mj_forward(model, data)
    for (let i = 0; i < 250; i += 1) this.tick()
    this.steps = 0
    this.startTime = data.time
  }

  get position() {
    return triple(this.data.site_xpos, this.shared.tcp)
  }

  get cube() {
    return triple(this.data.xpos, this.shared.cubeBody)
  }

  contacts() {
    const { data, shared } = this
    const fingers = new Set()
    let support = false
    let forbidden = false
    const count = data.ncon
    if (count === 0) return { fingers, support, forbidden }
    const list = data.contact
    try {
      for (let i = 0; i < count; i += 1) {
        const contact = list.get(i)
        if (!contact) continue
        const dist = contact.dist
        const g1 = contact.geom1
        const g2 = contact.geom2
        contact.delete()
        if (dist > 0.0005) continue
        const b1 = shared.geomBody[g1]
        const b2 = shared.geomBody[g2]
        if (g1 === shared.cubeGeom || g2 === shared.cubeGeom) {
          const other = g1 === shared.cubeGeom ? g2 : g1
          const body = shared.geomBody[other]
          if (body === shared.left) fingers.add('left')
          if (body === shared.right) fingers.add('right')
          support = support || other === shared.support
        }
        if (
          (shared.forbiddenBodies.has(b1) && shared.obstacles.has(g2)) ||
          (shared.forbiddenBodies.has(b2) && shared.obstacles.has(g1))
        ) {
          forbidden = true
        }
      }
    } finally {
      list.delete()
    }
    return { fingers, support, forbidden }
  }

  tick() {
    const { data, mujoco, model, shared } = this
    const applied = data.qfrc_applied
    const bias = data.qfrc_bias
    for (const dof of shared.armDof) applied[dof] = bias[dof]
    mujoco.mj_step(model, data)
    this.steps += 1
    const { fingers, support, forbidden } = this.contacts()
    this.unsafeContacts += forbidden ? 1 : 0
    this.contactSeconds = fingers.size === 2 && this.closed ? this.contactSeconds + DT : 0
    const qvel = data.qvel
    const speed = Math.hypot(qvel[shared.cubeDof], qvel[shared.cubeDof + 1], qvel[shared.cubeDof + 2])
    const cube = this.cube
    const onTarget = Math.hypot(cube[0] - this.target[0], cube[1] - this.target[1]) < 0.025
    this.stableSeconds = support && onTarget && speed < 0.025 && !this.closed ? this.stableSeconds + DT : 0
    this.maxLift = Math.max(this.maxLift, cube[2] - this.source[2])
    for (const value of data.qpos) if (!Number.isFinite(value)) throw new Error('MuJoCo state became non-finite')
  }

  success() {
    return this.stableSeconds >= 0.4 && !this.closed && this.position[2] >= 0.17
  }

  ensureScratch() {
    if (!this.scratch) {
      const { mujoco, model } = this
      this.scratch = {
        data: new mujoco.MjData(model),
        jacp: new mujoco.DoubleBuffer(3 * model.nv),
        jacr: new mujoco.DoubleBuffer(3 * model.nv)
      }
    }
    return this.scratch
  }

  solveIk(goal) {
    const { mujoco, model, shared } = this
    const { data: d, jacp, jacr } = this.ensureScratch()
    d.qpos.set(this.data.qpos)
    const nv = model.nv
    let error = [0, 0, 0]
    for (let iteration = 0; iteration < 180; iteration += 1) {
      mujoco.mj_kinematics(model, d)
      mujoco.mj_comPos(model, d)
      const site = triple(d.site_xpos, shared.tcp)
      error = goal.map((value, i) => value - site[i])
      const current = mat2Quat(d.site_xmat, shared.tcp * 9)
      const inverse = [current[0], -current[1], -current[2], -current[3]]
      const rotation = quat2Vel(mulQuat(DOWN_QUAT, inverse))
      if (Math.hypot(...error) < 0.0004 && Math.hypot(...rotation) < 0.004) break
      mujoco.mj_jacSite(model, d, jacp, jacr, shared.tcp)
      const p = jacp.GetView()
      const r = jacr.GetView()
      const jac = [0, 1, 2]
        .map((row) => shared.armDof.map((dof) => p[row * nv + dof]))
        .concat([0, 1, 2].map((row) => shared.armDof.map((dof) => r[row * nv + dof])))
      const a = jac.map((rowI, i) => jac.map((rowJ, j) => rowI.reduce((sum, value, k) => sum + value * rowJ[k], 0) + (i === j ? 0.002 : 0)))
      const x = solve6(a, [...error, ...rotation])
      const qpos = d.qpos
      shared.armQ.forEach((address, k) => {
        const delta = Math.max(-0.15, Math.min(0.15, jac.reduce((sum, row, i) => sum + row[k] * x[i], 0)))
        const [low, high] = shared.ranges[k]
        qpos[address] = Math.max(low, Math.min(high, qpos[address] + delta))
      })
    }
    return { q: shared.armQ.map((address) => d.qpos[address]), error: Math.hypot(...error) }
  }

  /** A shadow world with copied state for preview rollouts. Call dispose() after use. */
  clone() {
    const other = new RobotWorld(this.mujoco, this.model, new this.mujoco.MjData(this.model, this.data), {
      task: this.task,
      seed: this.seed,
      target: [...this.target],
      source: [...this.source],
      shared: this.shared
    })
    Object.assign(other, {
      closed: this.closed,
      stableSeconds: this.stableSeconds,
      contactSeconds: this.contactSeconds,
      maxLift: this.maxLift,
      unsafeContacts: this.unsafeContacts,
      steps: this.steps,
      startTime: this.startTime
    })
    other.ownsModel = false
    return other
  }

  /** Move to target under the quintic profile; yields a frame every 20 steps. */
  *motion(target, gripper, seconds = 0.6, emit = true) {
    const goal = target ?? this.position
    if (goal.length !== 3 || !goal.every(Number.isFinite)) throw new Error('Target must contain three finite coordinates')
    if (![null, undefined, 'open', 'close'].includes(gripper) || !Number.isFinite(seconds) || !(seconds > 0 && seconds <= 10)) {
      throw new Error('Invalid gripper command or motion duration')
    }
    if (goal.some((value, i) => value < WORKSPACE[0][i] || value > WORKSPACE[1][i])) throw new Error('Target outside workspace')
    const { q, error } = this.solveIk(goal)
    if (error > 0.004) throw new Error(`Unreachable target (${error.toFixed(4)} m)`)
    const start = Array.from(this.data.ctrl.subarray(0, 7))
    if (gripper) {
      this.closed = gripper === 'close'
      this.data.ctrl[7] = this.closed ? 0 : 255
    }
    const count = Math.max(1, Math.floor(seconds / DT))
    for (let i = 1; i <= count; i += 1) {
      const t = Math.min(1, i / (count * 0.75))
      const blend = t ** 3 * (10 - 15 * t + 6 * t * t)
      const ctrl = this.data.ctrl
      for (let k = 0; k < 7; k += 1) ctrl[k] = start[k] + (q[k] - start[k]) * blend
      this.tick()
      if (i % 20 === 0 || i === count) yield emit ? this.frame() : null
    }
  }

  observe() {
    const { fingers, support, forbidden } = this.contacts()
    const tcp = this.position
    const cube = this.cube
    return {
      task: TASKS[this.task].goal,
      source: 'MuJoCo geometry and contacts',
      units: 'metres',
      scene_name: TASKS[this.task].name,
      tcp: tcp.map((value) => round(value, 5)),
      object: cube.map((value) => round(value, 5)),
      relative_geometry: {
        tcp_object_xy_distance_m: round(Math.hypot(tcp[0] - cube[0], tcp[1] - cube[1]), 4),
        tcp_above_object_m: round(tcp[2] - cube[2], 4),
        object_destination_xy_distance_m: round(Math.hypot(cube[0] - this.target[0], cube[1] - this.target[1]), 4),
        object_above_destination_m: round(cube[2] - this.target[2], 4),
        travel_tcp_height_m: TRAVEL_Z
      },
      destination: [...this.target],
      gripper: this.closed ? 'closed' : 'open',
      finger_contacts: [...fingers].sort(),
      held: fingers.size === 2 && this.closed,
      grasp_secured: this.contactSeconds >= 0.16,
      support_contact: support,
      stable_seconds: round(this.stableSeconds, 3),
      success: this.success(),
      forbidden_contact: forbidden,
      max_lift_m: round(this.maxLift, 4),
      sim_seconds: round(this.data.time - this.startTime, 3)
    }
  }

  /** Body and geometry poses for the renderer, plus the observation. */
  frame(observation) {
    const { data, mujoco, model } = this
    mujoco.mj_forward(model, data)
    return {
      time: data.time,
      qpos: Array.from(data.qpos),
      bodies: poses(data.xpos, data.xmat, model.nbody),
      geoms: poses(data.geom_xpos, data.geom_xmat, model.ngeom),
      observation: observation ?? this.observe()
    }
  }

  /** The frame of a recorded qpos, computed on the scratch data. */
  replayFrame(qpos) {
    const { mujoco, model } = this
    const { data: d } = this.ensureScratch()
    d.qpos.set(qpos)
    mujoco.mj_kinematics(model, d)
    return {
      bodies: poses(d.xpos, d.xmat, model.nbody),
      geoms: poses(d.geom_xpos, d.geom_xmat, model.ngeom)
    }
  }

  /** Geometry the renderer draws itself: everything except collision group 3. */
  primitives() {
    const { mujoco, model } = this
    const items = []
    for (let i = 0; i < model.ngeom; i += 1) {
      if (model.geom_group[i] === 3) continue
      items.push({
        id: i,
        name: mujoco.mj_id2name(model, OBJ.GEOM, i) ?? '',
        type: model.geom_type[i],
        size: Array.from(model.geom_size.subarray(i * 3, i * 3 + 3)),
        color: Array.from(model.geom_rgba.subarray(i * 4, i * 4 + 4))
      })
    }
    return items
  }

  bodyId(name) {
    return this.mujoco.mj_name2id(this.model, OBJ.BODY, name)
  }

  dispose() {
    if (this.scratch) {
      this.scratch.data.delete()
      this.scratch.jacp.delete()
      this.scratch.jacr.delete()
      this.scratch = null
    }
    this.data.delete()
    if (this.ownsModel !== false) this.model.delete()
  }
}

function poses(xpos, xmat, count) {
  const out = new Array(count)
  for (let i = 0; i < count; i += 1) {
    out[i] = [
      xpos[i * 3], xpos[i * 3 + 1], xpos[i * 3 + 2],
      xmat[i * 9], xmat[i * 9 + 1], xmat[i * 9 + 2],
      xmat[i * 9 + 3], xmat[i * 9 + 4], xmat[i * 9 + 5],
      xmat[i * 9 + 6], xmat[i * 9 + 7], xmat[i * 9 + 8]
    ]
  }
  return out
}

export { distance }
