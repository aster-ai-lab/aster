/**
 * Original spring-driven blob face for the A3S Bot.
 * Silhouette and eye marks are A3S geometry — not a third-party character.
 */

export type A3SBotAvatarActivity = 'idle' | 'working' | 'paused' | 'completed' | 'failed'

export interface A3SBotAvatarGaze {
  x: number
  y: number
}

export interface A3SBotAvatarIdleGlance extends A3SBotAvatarGaze {
  centerMs: number
  holdMs: number
}

export type A3SBotAvatarIdleDirection = -1 | 1
export type A3SBotAvatarIdlePose = 'left' | 'right' | 'thinking'

export interface A3SBotAvatarIdleSequence {
  direction: A3SBotAvatarIdleDirection
  sideGlances: number
}

export interface A3SBotAvatarIdleStep {
  pose: A3SBotAvatarIdlePose
  sequence: A3SBotAvatarIdleSequence
}

interface A3SBotAvatarBounds {
  height: number
  left: number
  top: number
  width: number
}

const BLINK_DELAY_MIN_MS = 3_200
const BLINK_DELAY_RANGE_MS = 4_000
/** Share of the avatar box the pupils may travel. Fixed pixels vanish on the home bot. */
const GAZE_TRAVEL_RATIO = 0.18

export function calculateA3SBotAvatarGaze(
  pointerX: number,
  pointerY: number,
  bounds: A3SBotAvatarBounds
): A3SBotAvatarGaze {
  const deltaX = pointerX - (bounds.left + bounds.width / 2)
  const deltaY = pointerY - (bounds.top + bounds.height / 2)
  const distance = Math.hypot(deltaX, deltaY)
  if (distance === 0) return { x: 0, y: 0 }

  const responseRadius = Math.max(bounds.width, bounds.height, 1) * 0.75
  const travel = Math.max(bounds.width, bounds.height, 1) * GAZE_TRAVEL_RATIO * Math.min(1, distance / responseRadius)
  return {
    x: (deltaX / distance) * travel,
    y: (deltaY / distance) * travel
  }
}

export function nextA3SBotAvatarBlinkDelay(random: () => number = Math.random): number {
  const sample = Math.min(1, Math.max(0, random()))
  return Math.round(BLINK_DELAY_MIN_MS + BLINK_DELAY_RANGE_MS * sample)
}

export function nextA3SBotAvatarIdleGlance(
  direction: A3SBotAvatarIdleDirection,
  random: () => number = Math.random
): A3SBotAvatarIdleGlance {
  const horizontalSample = Math.min(1, Math.max(0, random()))
  const verticalSample = Math.min(1, Math.max(0, random()))
  const holdSample = Math.min(1, Math.max(0, random()))
  const centerSample = Math.min(1, Math.max(0, random()))
  return {
    centerMs: Math.round(650 + centerSample * 650),
    holdMs: Math.round(650 + holdSample * 450),
    x: direction * (2.8 + horizontalSample * 1.2),
    y: (verticalSample - 0.5) * 1.5
  }
}

export function nextA3SBotAvatarThinkingGlance(random: () => number = Math.random): A3SBotAvatarIdleGlance {
  const horizontalSample = Math.min(1, Math.max(0, random()))
  const verticalSample = Math.min(1, Math.max(0, random()))
  const holdSample = Math.min(1, Math.max(0, random()))
  const centerSample = Math.min(1, Math.max(0, random()))
  return {
    centerMs: Math.round(700 + centerSample * 600),
    holdMs: Math.round(1_200 + holdSample * 600),
    x: (horizontalSample - 0.5) * 1.2,
    y: -(3.2 + verticalSample * 0.8)
  }
}

export function advanceA3SBotAvatarIdleSequence(
  sequence: A3SBotAvatarIdleSequence,
  thinkAfterSideGlances: number
): A3SBotAvatarIdleStep {
  const threshold = Math.max(1, Math.floor(thinkAfterSideGlances))
  if (sequence.sideGlances >= threshold) {
    return {
      pose: 'thinking',
      sequence: { ...sequence, sideGlances: 0 }
    }
  }

  return {
    pose: sequence.direction === -1 ? 'left' : 'right',
    sequence: {
      direction: sequence.direction === -1 ? 1 : -1,
      sideGlances: sequence.sideGlances + 1
    }
  }
}

/** Fills the avatar box. A smaller path leaves a framed gap around the face. */
export const A3S_BOT_AVATAR_BLOB_UNIT_PATH =
  'M0.5 0.02C0.765 0.02 0.98 0.235 0.98 0.5C0.98 0.765 0.765 0.98 0.5 0.98C0.235 0.98 0.02 0.765 0.02 0.5C0.02 0.235 0.235 0.02 0.5 0.02Z'

/** Soft vertical marks in a 64×32 eye box. Not ellipses, and not mirrors. */
export const A3S_BOT_AVATAR_EYE_LEFT =
  'M9.2 8.4C10.4 3.6 17.2 3.2 18.6 8.8C19.8 13.6 18.4 22.2 14.6 24.4C10.4 26.8 7.2 22.4 7.4 16.2C7.5 12.8 8.2 10.4 9.2 8.4Z'

export const A3S_BOT_AVATAR_EYE_RIGHT =
  'M45.2 8.2C44.2 3.4 51.4 3 52.8 8.6C54 13.4 52.6 22.4 48.6 24.6C44.2 27 41.4 22.2 41.6 16C41.7 12.6 43.8 10.2 45.2 8.2Z'

/**
 * Status marks stay wide enough to read inside an 18px task row.
 * Shapes are original: alert ticks, sleepy slits, a settled smile, a failed frown.
 */
const A3S_BOT_AVATAR_STATUS_EYES: Record<A3SBotAvatarActivity, { left: string; right: string }> = {
  idle: { left: A3S_BOT_AVATAR_EYE_LEFT, right: A3S_BOT_AVATAR_EYE_RIGHT },
  working: { left: A3S_BOT_AVATAR_EYE_LEFT, right: A3S_BOT_AVATAR_EYE_RIGHT },
  paused: {
    left: 'M4.4 12.2C4.4 9.4 8.2 8.2 14.2 8.2C20.2 8.2 23.8 9.4 23.8 12.2C23.8 18.4 19.6 20.6 14.2 20.6C8.6 20.6 4.4 18.4 4.4 12.2Z',
    right:
      'M40.2 13C40.2 10.2 44.2 9 50.2 9C56.2 9 59.8 10.2 59.8 13C59.8 19.2 55.4 21.4 50.2 21.4C44.8 21.4 40.2 19.2 40.2 13Z'
  },
  completed: {
    left: 'M4.2 9.2C8.4 6.4 19.6 6.2 23.8 9C21.2 16.8 18.4 21.6 14 21.6C9.4 21.6 6.6 16.8 4.2 9.2Z',
    right: 'M40.2 8.6C44.6 5.8 55.8 5.6 60 8.4C57.2 16.4 54.2 21.4 49.8 21.4C45.2 21.4 42.4 16.4 40.2 8.6Z'
  },
  failed: {
    left: 'M4.2 20.8C8.6 23.6 19.8 23.8 23.8 21C21.2 13.2 18.4 8.4 14 8.4C9.4 8.4 6.6 13.2 4.2 20.8Z',
    right: 'M40.2 21.4C44.6 24.2 55.8 24.4 60 21.6C57.2 13.6 54.2 8.6 49.8 8.6C45.2 8.6 42.4 13.6 40.2 21.4Z'
  }
}

export function a3sBotAvatarEyePaths(activity: A3SBotAvatarActivity): { left: string; right: string } {
  return A3S_BOT_AVATAR_STATUS_EYES[activity]
}

const SVG_NS = 'http://www.w3.org/2000/svg'
const FRAME_DT = 1 / 60
const MAX_DT = 0.05
const BLINK_SHUT = 0.12
const BLINK_HOLD_MS = 78
const DOUBLE_BLINK_CHANCE = 0.15
const DOUBLE_BLINK_GAP_MS = 168

export interface A3SBotAvatarSpring {
  target: number
  value: number
  velocity: number
}

export interface A3SBotAvatarPose {
  bodyX: number
  bodyY: number
  gazeBiasX: number
  gazeBiasY: number
  lid: number
  spin: number
  squashX: number
  squashY: number
}

export interface A3SBotAvatarMotionInput {
  activity: A3SBotAvatarActivity
  gaze: A3SBotAvatarGaze | null
  idleGaze: boolean
  tracking: boolean
}

export function createA3SBotAvatarSpring(value: number): A3SBotAvatarSpring {
  return { target: value, value, velocity: 0 }
}

export function stepA3SBotAvatarSpring(
  spring: A3SBotAvatarSpring,
  stiffness: number,
  damping: number,
  dt: number
): void {
  const steps = Math.max(1, Math.min(4, Math.round(dt / FRAME_DT)))
  const stepDt = dt / steps
  for (let index = 0; index < steps; index += 1) {
    const accel = -stiffness * (spring.value - spring.target) - damping * spring.velocity
    spring.velocity += accel * stepDt
    spring.value += spring.velocity * stepDt
    if (!Number.isFinite(spring.value) || !Number.isFinite(spring.velocity)) {
      spring.value = spring.target
      spring.velocity = 0
      return
    }
  }
}

export function poseA3SBotAvatar(activity: A3SBotAvatarActivity, timeSec: number, phaseSec: number): A3SBotAvatarPose {
  const breath = Math.sin(timeSec * 1.15)
  if (activity === 'working') {
    const hop = Math.sin(timeSec * 6.4)
    const lift = Math.max(0, hop)
    return {
      bodyX: Math.sin(timeSec * 2.1) * 0.35,
      bodyY: -lift * 1.35,
      gazeBiasX: Math.sin(timeSec * 1.7) * 0.45,
      gazeBiasY: 0.55,
      lid: 1,
      spin: Math.sin(timeSec * 2.3) * 2.2,
      squashX: 1 + lift * 0.035,
      squashY: 1 - lift * 0.045
    }
  }
  if (activity === 'paused') {
    return {
      bodyX: Math.sin(timeSec * 0.35) * 0.2,
      bodyY: 0.85 + breath * 0.2,
      gazeBiasX: 0,
      gazeBiasY: 1.15,
      lid: 0.34,
      spin: breath * 0.6,
      squashX: 1.02,
      squashY: 0.95
    }
  }
  if (activity === 'failed') {
    const shake = Math.exp(-((phaseSec % 2.6) - 0.15) * ((phaseSec % 2.6) - 0.15) * 18)
    return {
      bodyX: Math.sin(timeSec * 28) * shake * 1.1,
      bodyY: 0.45,
      gazeBiasX: 0,
      gazeBiasY: 1.35,
      lid: 0.62,
      spin: Math.sin(timeSec * 26) * shake * 4,
      squashX: 1.03,
      squashY: 0.94
    }
  }
  if (activity === 'completed') {
    const pop = Math.exp(-phaseSec * 3.2) * Math.sin(Math.min(phaseSec, 0.7) * 13)
    return {
      bodyX: 0,
      bodyY: pop * -1.5 + breath * -0.35,
      gazeBiasX: 0,
      gazeBiasY: -0.35,
      lid: 1,
      spin: pop * 2.4,
      squashX: 1 - pop * 0.02,
      squashY: 1 + pop * 0.035 + breath * 0.008
    }
  }
  return {
    bodyX: Math.sin(timeSec * 0.37) * 0.28,
    bodyY: breath * -0.72,
    gazeBiasX: 0,
    gazeBiasY: 0,
    lid: 1,
    spin: Math.sin(timeSec * 0.42) * 1.15,
    squashX: 1 - breath * 0.01,
    squashY: 1 + breath * 0.014
  }
}

export function createA3SBotAvatarEyeMark(): SVGSVGElement {
  const svg = document.createElementNS(SVG_NS, 'svg')
  svg.setAttribute('viewBox', '0 0 64 32')
  svg.setAttribute('class', 'a3s-bot-avatar__eye-mark')
  svg.setAttribute('aria-hidden', 'true')
  svg.setAttribute('focusable', 'false')
  for (const [side, pathData] of [
    ['left', A3S_BOT_AVATAR_EYE_LEFT],
    ['right', A3S_BOT_AVATAR_EYE_RIGHT]
  ] as const) {
    const path = document.createElementNS(SVG_NS, 'path')
    path.setAttribute('class', 'a3s-bot-avatar__eye')
    path.setAttribute('d', pathData)
    path.setAttribute('data-a3s-bot-avatar-eye', side)
    svg.append(path)
  }
  return svg
}

/** Blob fill plus polygon eyes. Returns the eyes host so callers can add glasses. */
export function mountA3SBotAvatarMark(face: HTMLElement): HTMLElement {
  const blob = document.createElement('span')
  blob.className = 'a3s-bot-avatar__blob'
  blob.setAttribute('aria-hidden', 'true')
  const eyes = document.createElement('span')
  eyes.className = 'a3s-bot-avatar__eyes'
  eyes.append(createA3SBotAvatarEyeMark())
  face.append(blob, eyes)
  return eyes
}

interface MotionBinding {
  activity: A3SBotAvatarActivity
  activityAt: number
  blinkAt: number
  blinkShutUntil: number
  centerUntil: number
  el: HTMLElement
  glance: A3SBotAvatarGaze
  glanceUntil: number
  hold: boolean
  pendingDoubleBlink: boolean
  pose: {
    bodyX: A3SBotAvatarSpring
    bodyY: A3SBotAvatarSpring
    gazeX: A3SBotAvatarSpring
    gazeY: A3SBotAvatarSpring
    lid: A3SBotAvatarSpring
    spin: A3SBotAvatarSpring
    squashX: A3SBotAvatarSpring
    squashY: A3SBotAvatarSpring
  }
  read: () => A3SBotAvatarMotionInput
  sequence: A3SBotAvatarIdleSequence
  thinkAfter: number
}

const bindings = new Set<MotionBinding>()
let frame = 0
let lastFrame = 0
let pointerBound = false
let pointerX = 0
let pointerY = 0

function gazeDirection(gaze: A3SBotAvatarGaze): 'center' | 'left' | 'right' | 'up' {
  if (gaze.y < -1.25) return 'up'
  if (gaze.x < -0.5) return 'left'
  if (gaze.x > 0.5) return 'right'
  return 'center'
}

function scheduleGlance(binding: MotionBinding, now: number): void {
  const step = advanceA3SBotAvatarIdleSequence(binding.sequence, binding.thinkAfter)
  binding.sequence = step.sequence
  const glance =
    step.pose === 'thinking'
      ? nextA3SBotAvatarThinkingGlance()
      : nextA3SBotAvatarIdleGlance(step.pose === 'left' ? -1 : 1)
  if (step.pose === 'thinking') {
    binding.thinkAfter = 2 + Math.floor(Math.random() * 3)
  }
  binding.glance = glance
  binding.hold = true
  binding.glanceUntil = now + glance.holdMs
  binding.centerUntil = glance.centerMs
}

function stepBinding(binding: MotionBinding, now: number, dt: number): void {
  const input = binding.read()
  if (input.activity !== binding.activity) {
    binding.activity = input.activity
    binding.activityAt = now
  }
  const timeSec = now / 1000
  const phaseSec = (now - binding.activityAt) / 1000
  const pose = poseA3SBotAvatar(input.activity, timeSec, phaseSec)

  if (!input.gaze && !input.tracking && input.idleGaze && now >= binding.glanceUntil) {
    if (binding.hold) {
      binding.glance = { x: 0, y: 0 }
      binding.hold = false
      binding.glanceUntil = now + binding.centerUntil
    } else {
      scheduleGlance(binding, now)
    }
  }

  let gazeTarget = input.gaze ?? { x: pose.gazeBiasX, y: pose.gazeBiasY }
  if (!input.gaze && input.tracking) {
    const tracked = calculateA3SBotAvatarGaze(pointerX, pointerY, binding.el.getBoundingClientRect())
    gazeTarget = {
      x: tracked.x + pose.gazeBiasX,
      y: tracked.y + pose.gazeBiasY
    }
  } else if (!input.gaze && input.idleGaze) {
    gazeTarget = {
      x: binding.glance.x + pose.gazeBiasX,
      y: binding.glance.y + pose.gazeBiasY
    }
  }

  if (now >= binding.blinkAt && input.activity !== 'paused') {
    binding.pose.lid.target = BLINK_SHUT
    binding.blinkShutUntil = now + BLINK_HOLD_MS
    binding.pendingDoubleBlink = !binding.pendingDoubleBlink && Math.random() < DOUBLE_BLINK_CHANCE
    binding.blinkAt =
      now + (binding.pendingDoubleBlink ? BLINK_HOLD_MS + DOUBLE_BLINK_GAP_MS : nextA3SBotAvatarBlinkDelay())
  }
  if (now >= binding.blinkShutUntil && binding.pose.lid.target === BLINK_SHUT) {
    binding.pose.lid.target = pose.lid
  } else if (binding.pose.lid.target !== BLINK_SHUT) {
    binding.pose.lid.target = pose.lid
  }

  binding.pose.bodyX.target = pose.bodyX
  binding.pose.bodyY.target = pose.bodyY + (gazeDirection(gazeTarget) === 'up' ? -1.1 : 0)
  binding.pose.gazeX.target = gazeTarget.x
  binding.pose.gazeY.target = gazeTarget.y
  binding.pose.spin.target = pose.spin
  binding.pose.squashX.target = pose.squashX
  binding.pose.squashY.target = pose.squashY

  stepA3SBotAvatarSpring(binding.pose.bodyX, 180, 22, dt)
  stepA3SBotAvatarSpring(binding.pose.bodyY, 180, 22, dt)
  stepA3SBotAvatarSpring(binding.pose.gazeX, 520, 34, dt)
  stepA3SBotAvatarSpring(binding.pose.gazeY, 520, 34, dt)
  stepA3SBotAvatarSpring(binding.pose.lid, 1400, 48, dt)
  stepA3SBotAvatarSpring(binding.pose.spin, 160, 20, dt)
  stepA3SBotAvatarSpring(binding.pose.squashX, 220, 24, dt)
  stepA3SBotAvatarSpring(binding.pose.squashY, 220, 24, dt)

  const gaze = { x: binding.pose.gazeX.value, y: binding.pose.gazeY.value }
  const direction = gazeDirection(gaze)
  const root = binding.el.parentElement ?? binding.el
  root.dataset.characterMotion = 'spring'
  root.dataset.gazeDirection = direction
  binding.el.style.setProperty('--a3s-bot-avatar-body-x', `${binding.pose.bodyX.value.toFixed(2)}px`)
  binding.el.style.setProperty('--a3s-bot-avatar-body-y', `${binding.pose.bodyY.value.toFixed(2)}px`)
  binding.el.style.setProperty('--a3s-bot-avatar-gaze-x', `${gaze.x.toFixed(2)}px`)
  binding.el.style.setProperty('--a3s-bot-avatar-gaze-y', `${gaze.y.toFixed(2)}px`)
  binding.el.style.setProperty('--a3s-bot-avatar-head-rotate-x', `${(-gaze.y * 2).toFixed(2)}deg`)
  binding.el.style.setProperty('--a3s-bot-avatar-head-rotate-y', `${(gaze.x * 2.5).toFixed(2)}deg`)
  binding.el.style.setProperty('--a3s-bot-avatar-lid', binding.pose.lid.value.toFixed(3))
  binding.el.style.setProperty('--a3s-bot-avatar-spin', `${binding.pose.spin.value.toFixed(2)}deg`)
  binding.el.style.setProperty('--a3s-bot-avatar-squash-x', binding.pose.squashX.value.toFixed(3))
  binding.el.style.setProperty('--a3s-bot-avatar-squash-y', binding.pose.squashY.value.toFixed(3))
  binding.el.dataset.blinking = binding.pose.lid.value < 0.45 ? 'true' : 'false'
}

function onPointerMove(event: PointerEvent): void {
  pointerX = event.clientX
  pointerY = event.clientY
}

function syncPointerListener(): void {
  // Keep coordinates warm before tracking flips on. Home starts idle, then
  // follows the pointer only after the composer focuses — attaching only at
  // mount misses that transition.
  const needsPointer = bindings.size > 0
  if (needsPointer && !pointerBound) {
    window.addEventListener('pointermove', onPointerMove, { passive: true })
    window.addEventListener('pointerdown', onPointerMove, { passive: true })
    pointerBound = true
    return
  }
  if (!needsPointer && pointerBound) {
    window.removeEventListener('pointermove', onPointerMove)
    window.removeEventListener('pointerdown', onPointerMove)
    pointerBound = false
  }
}

function tick(now: number): void {
  syncPointerListener()
  const dt = Math.min(MAX_DT, lastFrame === 0 ? FRAME_DT : (now - lastFrame) / 1000)
  lastFrame = now
  for (const binding of bindings) stepBinding(binding, now, dt)
  frame = bindings.size > 0 ? window.requestAnimationFrame(tick) : 0
}

export function attachA3SBotAvatarMotion(el: HTMLElement, read: () => A3SBotAvatarMotionInput): () => void {
  const now = performance.now()
  const binding: MotionBinding = {
    activity: read().activity,
    activityAt: now,
    blinkAt: now + nextA3SBotAvatarBlinkDelay(),
    blinkShutUntil: 0,
    centerUntil: 800,
    el,
    glance: { x: 0, y: 0 },
    glanceUntil: now + 700 + Math.round(Math.random() * 650),
    hold: false,
    pendingDoubleBlink: false,
    pose: {
      bodyX: createA3SBotAvatarSpring(0),
      bodyY: createA3SBotAvatarSpring(0),
      gazeX: createA3SBotAvatarSpring(0),
      gazeY: createA3SBotAvatarSpring(0),
      lid: createA3SBotAvatarSpring(1),
      spin: createA3SBotAvatarSpring(0),
      squashX: createA3SBotAvatarSpring(1),
      squashY: createA3SBotAvatarSpring(1)
    },
    read,
    sequence: {
      direction: Math.random() < 0.5 ? -1 : 1,
      sideGlances: 0
    },
    thinkAfter: 2 + Math.floor(Math.random() * 3)
  }
  bindings.add(binding)
  syncPointerListener()
  if (frame === 0) {
    lastFrame = 0
    frame = window.requestAnimationFrame(tick)
  }
  return () => {
    bindings.delete(binding)
    el.dataset.blinking = 'false'
    for (const name of [
      '--a3s-bot-avatar-body-x',
      '--a3s-bot-avatar-body-y',
      '--a3s-bot-avatar-gaze-x',
      '--a3s-bot-avatar-gaze-y',
      '--a3s-bot-avatar-head-rotate-x',
      '--a3s-bot-avatar-head-rotate-y',
      '--a3s-bot-avatar-lid',
      '--a3s-bot-avatar-spin',
      '--a3s-bot-avatar-squash-x',
      '--a3s-bot-avatar-squash-y'
    ]) {
      el.style.removeProperty(name)
    }
    const root = el.parentElement
    if (root?.dataset.characterMotion === 'spring' && ![...bindings].some((item) => item.el.parentElement === root)) {
      delete root.dataset.characterMotion
      delete root.dataset.gazeDirection
    }
    syncPointerListener()
    if (bindings.size === 0 && frame !== 0) {
      window.cancelAnimationFrame(frame)
      frame = 0
      lastFrame = 0
    }
  }
}
