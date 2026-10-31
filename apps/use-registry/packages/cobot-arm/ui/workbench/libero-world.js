// LIBERO libero_90 scenes on MuJoCo WASM. The model, settled state and
// controller state are exported from LIBERO 8f1084e (MIT) on robosuite 1.4.1
// (MIT); the controller below is robosuite's OSC_POSE with the Panda gripper,
// stepped as robosuite steps it: 25 substeps of forward, forward, control, step.
const OBJ = { BODY: 1, JOINT: 3, SITE: 6, CAMERA: 7 }

const triple = (array, at) => [array[at * 3], array[at * 3 + 1], array[at * 3 + 2]]
const clamp = (value, low, high) => Math.min(high, Math.max(low, value))
const isZero = (value) => value === 0

function matMul(a, b) {
  const rows = a.length
  const inner = b.length
  const columns = b[0].length
  const out = Array.from({ length: rows }, () => new Array(columns).fill(0))
  for (let i = 0; i < rows; i += 1) {
    for (let k = 0; k < inner; k += 1) {
      const value = a[i][k]
      if (value === 0) continue
      for (let j = 0; j < columns; j += 1) out[i][j] += value * b[k][j]
    }
  }
  return out
}

const transpose = (a) => a[0].map((_, j) => a.map((row) => row[j]))
const matVec = (a, v) => a.map((row) => row.reduce((sum, value, k) => sum + value * v[k], 0))

/** Gauss-Jordan inverse with partial pivoting; the operational-space matrices here are well conditioned. */
export function inverse(a) {
  const n = a.length
  const m = a.map((row, i) => [...row, ...Array.from({ length: n }, (_, j) => (i === j ? 1 : 0))])
  for (let column = 0; column < n; column += 1) {
    let pivot = column
    for (let row = column + 1; row < n; row += 1) if (Math.abs(m[row][column]) > Math.abs(m[pivot][column])) pivot = row
    if (Math.abs(m[pivot][column]) < 1e-14) throw new Error('operational-space matrix is singular')
    ;[m[column], m[pivot]] = [m[pivot], m[column]]
    const scale = m[column][column]
    for (let j = 0; j < 2 * n; j += 1) m[column][j] /= scale
    for (let row = 0; row < n; row += 1) {
      if (row === column) continue
      const factor = m[row][column]
      if (factor === 0) continue
      for (let j = 0; j < 2 * n; j += 1) m[row][j] -= factor * m[column][j]
    }
  }
  return m.map((row) => row.slice(n))
}

/** Rotation matrix (row-major 3x3 arrays) of an axis-angle vector, as robosuite's axisangle2quat then quat2mat. */
export function axisAngleMatrix(v) {
  const angle = Math.hypot(...v)
  if (angle === 0) return [[1, 0, 0], [0, 1, 0], [0, 0, 1]]
  const [x, y, z] = v.map((value) => value / angle)
  const s = Math.sin(angle)
  const c = 1 - Math.cos(angle)
  return [
    [1 - c * (y * y + z * z), -s * z + c * x * y, s * y + c * x * z],
    [s * z + c * x * y, 1 - c * (x * x + z * z), -s * x + c * y * z],
    [-s * y + c * x * z, s * x + c * y * z, 1 - c * (x * x + y * y)]
  ]
}

/** robosuite orientation_error: half the summed column cross products, in the world frame. */
export function orientationError(desired, current) {
  const out = [0, 0, 0]
  for (let i = 0; i < 3; i += 1) {
    const a = [current[0][i], current[1][i], current[2][i]]
    const b = [desired[0][i], desired[1][i], desired[2][i]]
    out[0] += 0.5 * (a[1] * b[2] - a[2] * b[1])
    out[1] += 0.5 * (a[2] * b[0] - a[0] * b[2])
    out[2] += 0.5 * (a[0] * b[1] - a[1] * b[0])
  }
  return out
}

export class LiberoWorld {
  /**
   * scene: { physics: MJCF text, episode: the exported aster-libero-episode-v1
   * object, init: the index of one of its exported init states (default 0) }.
   */
  static create(mujoco, scene) {
    if (scene?.episode?.format !== 'aster-libero-episode-v1') throw new Error('不是 LIBERO 场景包')
    const init = scene.init ?? 0
    if (!Number.isInteger(init) || !scene.episode.inits?.[init]) throw new Error('LIBERO 初始状态无效')
    const model = mujoco.MjModel.from_xml_string(scene.physics)
    return new LiberoWorld(mujoco, model, new mujoco.MjData(model), scene.episode, init)
  }

  constructor(mujoco, model, data, episode, init) {
    this.mujoco = mujoco
    this.model = model
    this.data = data
    this.episode = episode
    this.init = init
    const c = episode.controller
    const id = (type, name) => {
      const found = mujoco.mj_name2id(model, type, name)
      if (found < 0) throw new Error(`LIBERO scene has no ${name}`)
      return found
    }
    this.site = id(OBJ.SITE, c.eef_site)
    this.hand = id(OBJ.BODY, c.eef_body)
    this.goalJoints = episode.success.joints.map((joint) => {
      const jointId = id(OBJ.JOINT, joint.joint)
      return { ...joint, id: jointId, address: model.jnt_qposadr[jointId] }
    })
    this.nv = model.nv
    this.jacp = new mujoco.DoubleBuffer(3 * model.nv)
    this.jacr = new mujoco.DoubleBuffer(3 * model.nv)
    this.fullM = new mujoco.DoubleBuffer(model.nv * model.nv)
    const scale = c.input_max.map((max, i) => Math.abs(c.output_max[i] - c.output_min[i]) / Math.abs(max - c.input_min[i]))
    this.action = {
      scale,
      inputShift: c.input_max.map((max, i) => (max + c.input_min[i]) / 2),
      outputShift: c.output_max.map((max, i) => (max + c.output_min[i]) / 2)
    }
    this.reset()
  }

  /** The settled LIBERO init state and the controller state robosuite held there. */
  reset() {
    const { data, model } = this
    const c = this.episode.inits[this.init]
    const s = c.state
    data.time = s.time
    data.qpos.set(s.qpos)
    data.qvel.set(s.qvel)
    data.ctrl.set(s.ctrl)
    if (model.na) data.act.set(s.act)
    data.qacc_warmstart.set(s.qacc_warmstart)
    this.goalPos = [...c.goal_pos]
    this.goalOri = c.goal_ori.map((row) => [...row])
    this.gripperAction = [...c.gripper_action]
    this.steps = 0
    this.mujoco.mj_forward(model, data)
    this.update()
  }

  update() {
    const { mujoco, model, data, site, nv } = this
    const c = this.episode.controller
    mujoco.mj_forward(model, data)
    this.eePos = triple(data.site_xpos, site)
    const xmat = data.site_xmat
    this.eeOri = [0, 1, 2].map((row) => [0, 1, 2].map((column) => xmat[site * 9 + row * 3 + column]))
    mujoco.mj_jacSite(model, data, this.jacp, this.jacr, site)
    const p = this.jacp.GetView()
    const r = this.jacr.GetView()
    const qvel = data.qvel
    const rows = (j) => [0, 1, 2].map((row) => c.qvel_index.map((dof) => j[row * nv + dof]))
    const velocity = (j) => [0, 1, 2].map((row) => {
      let sum = 0
      for (let k = 0; k < nv; k += 1) sum += j[row * nv + k] * qvel[k]
      return sum
    })
    this.velP = velocity(p)
    this.velR = velocity(r)
    this.jPos = rows(p)
    this.jOri = rows(r)
    this.q = c.qpos_index.map((i) => data.qpos[i])
    this.qd = c.qvel_index.map((i) => qvel[i])
    mujoco.mj_fullM(model, data, this.fullM)
    const full = this.fullM.GetView()
    this.mass = c.qvel_index.map((i) => c.qvel_index.map((j) => full[i * nv + j]))
  }

  setGoal(arm) {
    const { scale, inputShift, outputShift } = this.action
    const c = this.episode.controller
    const delta = arm.map((value, i) => (clamp(value, c.input_min[i], c.input_max[i]) - inputShift[i]) * scale[i] + outputShift[i])
    if (!delta.slice(3).every(isZero)) this.goalOri = matMul(axisAngleMatrix(delta.slice(3)), this.eeOri)
    this.goalPos = this.eePos.map((value, i) => value + delta[i])
  }

  torques() {
    const c = this.episode.controller
    const posErr = this.goalPos.map((value, i) => value - this.eePos[i])
    const oriErr = orientationError(this.goalOri, this.eeOri)
    const force = posErr.map((value, i) => c.kp[i] * value - c.kd[i] * this.velP[i])
    const torque = oriErr.map((value, i) => c.kp[i + 3] * value - c.kd[i + 3] * this.velR[i])
    const J = [...this.jPos, ...this.jOri]
    const JT = transpose(J)
    const Minv = inverse(this.mass)
    const lambda = (j) => inverse(matMul(matMul(j, Minv), transpose(j)))
    const lamFull = lambda(J)
    const wrench = [...matVec(lambda(this.jPos), force), ...matVec(lambda(this.jOri), torque)]
    const bias = this.data.qfrc_bias
    const tau = matVec(JT, wrench).map((value, i) => value + bias[c.qvel_index[i]])
    const jbar = matMul(matMul(Minv, JT), lamFull)
    const jbarJ = matMul(jbar, J)
    const n = this.q.length
    const kv = 2 * Math.sqrt(c.nullspace_kp)
    const pose = matVec(this.mass, this.q.map((value, i) => c.nullspace_kp * (c.initial_joint[i] - value) - kv * this.qd[i]))
    for (let i = 0; i < n; i += 1) {
      let sum = 0
      for (let k = 0; k < n; k += 1) sum += ((i === k ? 1 : 0) - jbarJ[k][i]) * pose[k]
      tau[i] = clamp(tau[i] + sum, c.torque_low[i], c.torque_high[i])
    }
    return tau
  }

  /** One 20 Hz environment step of the 7-D normalised OSC_POSE action. */
  step(action) {
    if (!Array.isArray(action) || action.length !== 7 || !action.every(Number.isFinite)) throw new Error('LIBERO 动作必须是 7 个有限数值')
    const { mujoco, model, data } = this
    const c = this.episode.controller
    const sign = Math.sign(action[6])
    const [low, high] = [c.gripper_ctrlrange.map((range) => range[0]), c.gripper_ctrlrange.map((range) => range[1])]
    for (let substep = 0; substep < c.substeps; substep += 1) {
      mujoco.mj_forward(model, data)
      this.update()
      if (substep === 0) this.setGoal(action.slice(0, 6))
      const tau = this.torques()
      this.gripperAction = this.gripperAction.map((value, i) => clamp(value + (i === 0 ? -1 : 1) * c.gripper_speed * sign, -1, 1))
      c.gripper_actuators.forEach((actuator, i) => {
        data.ctrl[actuator] = 0.5 * (high[i] + low[i]) + 0.5 * (high[i] - low[i]) * this.gripperAction[i]
      })
      c.arm_actuators.forEach((actuator, i) => {
        data.ctrl[actuator] = tau[i]
      })
      mujoco.mj_step(model, data)
    }
    for (const value of data.qpos) if (!Number.isFinite(value)) throw new Error('MuJoCo state became non-finite')
    this.steps += 1
  }

  /** The tool centre point after the last step, as robosuite reports robot0_eef_pos. */
  get tcp() {
    return triple(this.data.site_xpos, this.site)
  }

  /** x, y, z, w like robot0_eef_quat: robosuite reads the hand body, not the grip site. */
  get quaternionXyzw() {
    const [w, x, y, z] = this.data.xquat.subarray(this.hand * 4, this.hand * 4 + 4)
    return [x, y, z, w]
  }

  get fingerQpos() {
    return this.episode.controller.finger_qpos_index.map((i) => this.data.qpos[i])
  }

  goalQpos() {
    return this.goalJoints.map((joint) => this.data.qpos[joint.address])
  }

  /** World anchor of each goal joint: privileged state, for the reference planner only. */
  goalAnchors() {
    return this.goalJoints.map((joint) => triple(this.data.xanchor, joint.id))
  }

  /** Body ids for body names, for posing the render pack. */
  bodyIds(names) {
    return names.map((name) => {
      const found = this.mujoco.mj_name2id(this.model, OBJ.BODY, name)
      if (found < 0) throw new Error(`LIBERO scene has no body ${name}`)
      return found
    })
  }

  /** Copies of the body poses: from the live state, or from a recorded qpos through forward kinematics. */
  poses(qpos = null) {
    let data = this.data
    if (qpos) {
      this.scratch ??= new this.mujoco.MjData(this.model)
      data = this.scratch
      data.qpos.set(qpos)
      this.mujoco.mj_kinematics(this.model, data)
    }
    return { xpos: Float64Array.from(data.xpos), xquat: Float64Array.from(data.xquat) }
  }

  /** A MuJoCo camera's live pose and vertical field of view, in degrees. */
  cameraPose(name) {
    const camera = this.mujoco.mj_name2id(this.model, OBJ.CAMERA, name)
    if (camera < 0) throw new Error(`LIBERO scene has no camera ${name}`)
    const xmat = Array.from(this.data.cam_xmat.subarray(camera * 9, camera * 9 + 9))
    return { position: triple(this.data.cam_xpos, camera), xmat, fovy: this.model.cam_fovy[camera] }
  }

  get qpos() {
    return Float64Array.from(this.data.qpos)
  }

  /** LIBERO's Close predicate: every goal joint above its default close range. */
  success() {
    return this.goalJoints.every((joint) => this.data.qpos[joint.address] > joint.closed_above)
  }

  /** The policy-facing robot state, the same fields upstream's worker sends. */
  observation() {
    return {
      task: this.episode.language,
      step: this.steps,
      tcp: this.tcp,
      quaternion_xyzw: this.quaternionXyzw,
      gripper_qpos: this.fingerQpos,
      frame: 'world; metres; TCP = robot0_eef_pos'
    }
  }

  dispose() {
    this.jacp.delete()
    this.jacr.delete()
    this.fullM.delete()
    this.scratch?.delete()
    this.data.delete()
    this.model.delete()
  }
}
