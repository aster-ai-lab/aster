/* global clearInterval, clearTimeout, Event, fetch, performance, setInterval, setTimeout */
const LOOPBACK_ORIGIN = /^http:\/\/127\.0\.0\.1:\d{1,5}$/
const TOKEN = /^[a-f0-9]{64}$/

export function readStudioDescriptor(host = globalThis) {
  const value = host.__A3S_STUDIO_RUNTIME__
  if (value === undefined || value === null) {
    throw new Error('The studio runtime descriptor is absent.')
  }
  if (typeof value !== 'object' || Array.isArray(value)) {
    throw new Error('The studio runtime descriptor is invalid.')
  }
  const apiBaseUrl = value.apiBaseUrl
  const token = value.token
  if (typeof apiBaseUrl !== 'string' || !LOOPBACK_ORIGIN.test(apiBaseUrl)) {
    throw new Error('The studio runtime descriptor is invalid.')
  }
  if (typeof token !== 'string' || !TOKEN.test(token)) {
    throw new Error('The studio runtime descriptor is invalid.')
  }
  return { apiBaseUrl, token }
}

export async function readStudioBootstrap(host = globalThis, fetchImpl = globalThis.fetch) {
  const runtime = readStudioDescriptor(host)
  const response = await fetchImpl(`${runtime.apiBaseUrl}/api/bootstrap`, {
    method: 'GET',
    headers: { authorization: `Bearer ${runtime.token}` }
  })
  if (!response.ok) {
    throw new Error(`Studio bootstrap failed (${response.status}).`)
  }
  const body = await response.json()
  if (!body || typeof body.sessionId !== 'string' || typeof body.product !== 'string' || body.product.length === 0) {
    throw new Error('Studio bootstrap failed.')
  }
  return body
}

// Same geometry as src/renderer/components/ToWorkWordmark.tsx.
function toWorkWordmark(document) {
  const svg = document.createElementNS('http://www.w3.org/2000/svg', 'svg')
  svg.setAttribute('data-ui', 'workbench.wordmark.mark')
  svg.setAttribute('aria-hidden', 'true')
  svg.setAttribute('viewBox', '140 116 2164 512')
  svg.setAttribute('fill', 'none')
  svg.setAttribute('stroke', '#303030')
  svg.setAttribute('stroke-width', '76')
  svg.setAttribute('stroke-linecap', 'butt')
  svg.setAttribute('stroke-linejoin', 'miter')
  svg.setAttribute('stroke-miterlimit', '16')
  const parts = [
    ['path', { d: 'M156 170H532M344 170V612' }],
    ['circle', { cx: '606', cy: '432', r: '142' }],
    ['path', { d: 'M868 233.4L980 510.6L1091.9 233.4L1203.9 510.6L1315.9 233.4' }],
    ['circle', { cx: '1577.9', cy: '432', r: '142' }],
    ['path', { d: 'M1839.9 612V290H1957.7' }],
    ['path', { d: 'M2053.7 132V612M2053.7 396L2249.7 288M2053.7 396L2249.7 582' }]
  ]
  for (const [name, attrs] of parts) {
    const node = document.createElementNS('http://www.w3.org/2000/svg', name)
    for (const [key, value] of Object.entries(attrs)) node.setAttribute(key, value)
    svg.append(node)
  }
  return svg
}

export function paintStudioWelcome(document, bootstrap) {
  const product = document.getElementById('product')
  if (product) {
    product.replaceChildren()
    if (bootstrap.product === 'ToWork') {
      product.append(toWorkWordmark(document))
      product.setAttribute('aria-label', 'ToWork')
    } else {
      product.textContent = bootstrap.product
      product.removeAttribute('aria-label')
    }
  }
  document.title = bootstrap.product
}

function chinaPhone(raw) {
  let digits = String(raw).replace(/\D/g, '')
  if (digits.startsWith('86') && digits.length > 11 && digits.slice(2).startsWith('1')) {
    digits = digits.slice(2)
  }
  return digits.slice(0, 11)
}

function validPhone(digits) {
  return /^1[3-9]\d{9}$/.test(digits)
}

const MEMBER_TOKEN_KEY = 'a3s_org_member_token'
const MEMBER_PROFILE_KEY = 'a3s_org_member_profile'
const SMS_COOLDOWN_KEY = 'a3s.org.login.smsCooldown'

function unwrapOrg(payload, httpStatus) {
  if (payload && typeof payload === 'object' && 'code' in payload && 'message' in payload && 'data' in payload) {
    const code = Number(payload.code)
    const ok = code === 0 || (code >= 200 && code < 300)
    if (!ok || payload.success === false) {
      const error = new Error(
        typeof payload.message === 'string' && payload.message ? payload.message : `API error ${payload.code}`
      )
      error.code = code
      throw error
    }
    return payload.data
  }
  if (httpStatus >= 400) {
    const error = new Error(`HTTP ${httpStatus}`)
    error.code = httpStatus
    throw error
  }
  return payload
}

async function orgPost(host, fetchImpl, path, body) {
  let runtime
  try {
    runtime = readStudioDescriptor(host)
  } catch {
    const error = new Error('failed to fetch')
    error.code = 0
    throw error
  }
  let response
  try {
    response = await fetchImpl(`${runtime.apiBaseUrl}${path}`, {
      method: 'POST',
      headers: {
        accept: 'application/json',
        'content-type': 'application/json',
        authorization: `Bearer ${runtime.token}`
      },
      body: JSON.stringify(body)
    })
  } catch {
    const error = new Error('failed to fetch')
    error.code = 0
    throw error
  }
  const text = await response.text()
  let payload = null
  if (text) {
    try {
      payload = JSON.parse(text)
    } catch {
      const error = new Error(text || `HTTP ${response.status}`)
      error.code = response.status
      throw error
    }
  }
  return unwrapOrg(payload, response.status)
}

function failureClass(code) {
  const value = Number(code) || 0
  if (value >= 10000) return Math.floor(value / 100)
  return value
}

function loginErrorMessage(err) {
  const message = String(err && err.message ? err.message : '')
    .trim()
    .toLowerCase()
  const code = failureClass(err && err.code)
  if (message === 'account locked' || message === 'locked') return '账号已暂时锁定，请稍后再试'
  if (message === 'second factor required' || message === 'challenge_required') return '需要完成第二步验证'
  if (code === 403 || message.includes('not permitted to log in')) return '这个账号不能登录控制台'
  if (message === 'captcha required') return '请先完成滑块验证'
  if (message.includes('verification code')) return '验证码错误或已过期'
  if (
    code >= 500 ||
    message.startsWith('http ') ||
    message === 'storage' ||
    message.includes('failed to fetch') ||
    message.includes('networkerror') ||
    message.includes('network error')
  ) {
    return '登录服务暂时不可用，请稍后再试'
  }
  return '邮箱或密码不正确'
}

function sendCodeError(err) {
  const message = String(err && err.message ? err.message : '')
  if (message.trim().toLowerCase() === 'captcha required') return '请先完成滑块验证'
  if (/wait a minute/i.test(message)) return '发送太频繁，请稍等一分钟'
  return '操作失败，请稍后再试'
}

function showDevCode(document, host, devCode) {
  if (!/^\d{6}$/.test(devCode || '')) return
  const root = document.getElementById('toasts')
  if (!root) return
  root.replaceChildren()
  const toast = document.createElement('div')
  toast.className = 'toast'
  toast.setAttribute('role', 'status')
  const mark = document.createElement('span')
  mark.className = 'toast-mark'
  mark.textContent = 'i'
  const title = document.createElement('p')
  title.textContent = `开发验证码 ${devCode}`
  const close = document.createElement('button')
  close.type = 'button'
  close.setAttribute('aria-label', '关闭')
  close.textContent = '×'
  close.addEventListener('click', () => toast.remove())
  toast.append(mark, title, close)
  root.append(toast)
  const timer = (host.setTimeout || setTimeout)(() => toast.remove(), 20_000)
  close.addEventListener('click', () => (host.clearTimeout || clearTimeout)(timer))
}

function readCooldown(host) {
  try {
    const raw = host.sessionStorage?.getItem(SMS_COOLDOWN_KEY)
    if (!raw) return null
    const parsed = JSON.parse(raw)
    if (typeof parsed.phone !== 'string' || typeof parsed.until !== 'number') return null
    if (!validPhone(parsed.phone) || parsed.until <= Date.now()) {
      host.sessionStorage.removeItem(SMS_COOLDOWN_KEY)
      return null
    }
    return parsed
  } catch {
    return null
  }
}

function memberAvatar(member) {
  const avatar = member && typeof member.avatar === 'string' ? member.avatar.trim() : ''
  if (!avatar.startsWith('data:image/') || !avatar.includes(';base64,') || avatar.length > 400000) return ''
  return avatar
}

function rememberMember(host, auth) {
  const token = auth && auth.token
  const member = auth && auth.member
  if (typeof token !== 'string' || !token.startsWith('a3s_ms_') || !member || typeof member !== 'object') {
    const error = new Error('HTTP 502')
    error.code = 502
    throw error
  }
  const profile = {
    display_name: typeof member.display_name === 'string' ? member.display_name : '',
    phone: typeof member.phone === 'string' ? member.phone : '',
    avatar: memberAvatar(member)
  }
  host.localStorage?.setItem(MEMBER_TOKEN_KEY, token)
  host.localStorage?.setItem(MEMBER_PROFILE_KEY, JSON.stringify(profile))
  return profile
}

async function saveMemberSession(host, token, profile) {
  try {
    await studioFetch(host, host.fetch || fetch, '/api/member-session', {
      method: 'POST',
      body: {
        token,
        display_name: profile.display_name,
        phone: profile.phone,
        avatar: profile.avatar || ''
      }
    })
  } catch {
    const error = new Error('storage')
    error.code = 500
    throw error
  }
}

const SESSION_RESTORE_MS = 4000

function withTimeout(promise, ms, host) {
  return new Promise((resolve, reject) => {
    let done = false
    const timer = (host.setTimeout || setTimeout)(() => {
      if (done) return
      done = true
      reject(new Error('timeout'))
    }, ms)
    promise.then(
      (value) => {
        if (done) return
        done = true
        ;(host.clearTimeout || clearTimeout)(timer)
        resolve(value)
      },
      (error) => {
        if (done) return
        done = true
        ;(host.clearTimeout || clearTimeout)(timer)
        reject(error)
      }
    )
  })
}

function forgetMember(host) {
  try {
    host.localStorage?.removeItem(MEMBER_TOKEN_KEY)
    host.localStorage?.removeItem(MEMBER_PROFILE_KEY)
  } catch {
    // 当次页面仍按未登录处理。
  }
}

async function restoreMember(host) {
  try {
    const body = await studioFetch(host, host.fetch || fetch, '/api/member-session')
    if (body && typeof body.token === 'string' && body.token.startsWith('a3s_ms_')) {
      return rememberMember(host, {
        token: body.token,
        member: {
          display_name: typeof body.display_name === 'string' ? body.display_name : '',
          phone: typeof body.phone === 'string' ? body.phone : '',
          avatar: typeof body.avatar === 'string' ? body.avatar : ''
        }
      })
    }
  } catch {
    return readProfile(host)
  }
  forgetMember(host)
  return null
}

function readProfile(host) {
  try {
    const token = host.localStorage?.getItem(MEMBER_TOKEN_KEY)
    if (!token || !token.startsWith('a3s_ms_')) return null
    const profile = JSON.parse(host.localStorage.getItem(MEMBER_PROFILE_KEY) || 'null')
    if (!profile || typeof profile !== 'object') return null
    return profile
  } catch {
    return null
  }
}

const CHALLENGE_ID = /^[0-9a-f-]{32,36}$/i

function captchaFile(value) {
  const name = String(value || '')
    .split('?')[0]
    .split('/')
    .pop()
  return /^(?:bg-[a-z0-9]+\.jpg|piece-[a-z0-9]+\.png)$/.test(name) ? `/captcha/${name}` : ''
}

function wireStudioCaptcha(document, host, onChange) {
  const root = document.getElementById('captcha')
  if (!root) {
    return {
      required: false,
      token: () => '',
      passed: () => false,
      clear() {},
      spend() {},
      ensure: () => Promise.resolve('')
    }
  }
  const dialog = document.getElementById('captcha-dialog')
  const stage = document.getElementById('captcha-stage')
  const background = document.getElementById('captcha-bg')
  const piece = document.getElementById('captcha-piece')
  const veil = document.getElementById('captcha-veil')
  const veilText = document.getElementById('captcha-veil-text')
  const track = document.getElementById('captcha-track')
  const hint = document.getElementById('captcha-hint')
  const handle = document.getElementById('captcha-handle')
  const caption = document.getElementById('captcha-status')
  const refresh = document.getElementById('captcha-refresh')
  const closer = document.getElementById('captcha-close')
  let token = ''
  let challenge = null
  let status = 'idle'
  let fraction = 0
  let attemptsLeft = 0
  let drag = null
  let generation = 0
  let retries = 0
  let waiter = null
  let pendingEnsure = null
  const login = document.getElementById('login')

  function bindImage(img) {
    if (!img || img.dataset.bound === 'true') return
    img.dataset.bound = 'true'
    img.addEventListener('error', () => {
      if (status !== 'loading' && status !== 'idle') return
      challenge = null
      status = 'error'
      paint()
    })
  }
  bindImage(background)
  bindImage(piece)

  function pieceX(value) {
    if (!challenge) return 0
    return Math.round(value * Math.max(1, challenge.width - challenge.pieceW))
  }

  function paint() {
    const width = challenge?.width || 600
    const height = challenge?.height || 340
    const pad = challenge?.pad ?? 12
    const pieceW = challenge?.pieceW || 56
    const box = pieceW + pad * 2
    if (stage) stage.style.aspectRatio = `${width} / ${height}`
    const bgSrc = captchaFile(challenge?.background)
    const pieceSrc = captchaFile(challenge?.piece)
    if (background) {
      background.hidden = !bgSrc
      if (bgSrc) background.src = bgSrc
    }
    if (piece) {
      piece.hidden = !pieceSrc
      if (pieceSrc) piece.src = pieceSrc
      const left = fraction * Math.max(1, width - pieceW) - pad
      piece.style.left = `${(left / width) * 100}%`
      piece.style.top = `${(((challenge?.y || 0) - pad) / height) * 100}%`
      piece.style.width = `${(box / width) * 100}%`
      piece.style.height = `${(box / height) * 100}%`
      piece.classList.toggle('is-fail', status === 'fail')
    }
    if (handle) {
      handle.style.left = `calc(${fraction * 100}% - ${fraction * 3.5}rem + 0.25rem)`
      handle.setAttribute('aria-valuenow', String(Math.round(fraction * 100)))
      handle.classList.toggle('is-success', status === 'success')
    }
    if (track) {
      track.classList.toggle('is-success', status === 'success')
      track.classList.toggle('is-busy', status === 'loading' || status === 'verifying' || status === 'error')
    }
    let text = ''
    if (status === 'loading') text = '正在准备拼图'
    else if (status === 'verifying') text = '校验中…'
    else if (status === 'error') text = '暂时无法签发挑战'
    else if (status === 'success') text = '验证通过'
    else if (status === 'fail') {
      text = '偏差过大，请重试'
      if (attemptsLeft > 0) text = `${text} · 剩余 ${attemptsLeft} 次尝试`
    }
    const busy = status === 'loading' || status === 'verifying'
    const veilShown = busy || status === 'success' || (status === 'error' && !challenge)
    if (caption) caption.textContent = veilShown && status !== 'success' ? '' : text
    if (refresh) {
      refresh.disabled = busy
      refresh.textContent = status === 'error' ? '重试' : '换一张'
    }
    if (hint) hint.hidden = fraction > 0.01 || status === 'success' || status === 'verifying'
    if (veil) {
      veil.hidden = !veilShown
      veil.classList.toggle('is-success', status === 'success')
      const spinner = veil.querySelector('.captcha-spinner')
      if (spinner) spinner.hidden = !busy
      if (veilText) veilText.textContent = status === 'success' ? '验证通过' : text
    }
  }

  function show() {
    if (dialog) dialog.hidden = false
  }

  function hide() {
    if (dialog) dialog.hidden = true
  }

  function finishWait(error, proof) {
    const current = waiter
    waiter = null
    pendingEnsure = null
    if (!current) return
    if (error) current.reject(error)
    else current.resolve(proof)
  }

  function load(manual) {
    if (manual) retries = 0
    const gen = ++generation
    token = ''
    challenge = null
    fraction = 0
    attemptsLeft = 0
    drag = null
    status = 'loading'
    paint()
    onChange()
    const pending = orgPost(host, host.fetch, '/v1/api/captcha/challenges', {})
      .then((data) => {
        if (gen !== generation) return
        const backgroundUrl = captchaFile(data && data.background)
        const pieceUrl = captchaFile(data && data.piece)
        if (!data || !CHALLENGE_ID.test(String(data.challengeId || '')) || !backgroundUrl || !pieceUrl) {
          status = 'error'
          paint()
          return
        }
        challenge = {
          challengeId: String(data.challengeId),
          background: String(data.background || ''),
          piece: String(data.piece || ''),
          width: Number(data.width) || 600,
          height: Number(data.height) || 340,
          pieceW: Number(data.pieceW) || 56,
          pieceH: Number(data.pieceH) || 56,
          pad: Number.isFinite(Number(data.pad)) ? Number(data.pad) : 12,
          y: Number(data.y) || 0,
          tolerance: Number(data.tolerance) || 0
        }
        retries = 0
        status = 'idle'
        paint()
        if (dialog && !dialog.hidden && handle) handle.focus()
      })
      .catch(() => {
        if (gen !== generation) return
        status = 'error'
        paint()
        if (retries < 1) {
          retries += 1
          ;(host.setTimeout || setTimeout)(() => {
            if (gen === generation && status === 'error') load()
          }, 600)
        }
      })
    if (login) login._captchaReady = pending
    return pending
  }

  function finish(x, durationMs, points) {
    if (!challenge || status !== 'idle') return Promise.resolve()
    const gen = generation
    status = 'verifying'
    paint()
    const pending = orgPost(host, host.fetch, `/v1/api/captcha/challenges/${challenge.challengeId}/verifications`, {
      x,
      durationMs,
      points
    })
      .then((result) => {
        if (gen !== generation) return
        if (result && result.verified && typeof result.token === 'string' && result.token) {
          token = result.token
          status = 'success'
          paint()
          onChange()
          hide()
          finishWait(null, token)
          return
        }
        attemptsLeft = Number(result && result.attemptsLeft) || 0
        status = 'fail'
        paint()
        ;(host.setTimeout || setTimeout)(() => {
          if (gen === generation && status === 'fail') load()
        }, 900)
      })
      .catch(() => {
        if (gen !== generation) return
        status = 'error'
        paint()
        ;(host.setTimeout || setTimeout)(() => {
          if (gen === generation && status === 'error') load()
        }, 1200)
      })
    if (login) login._captchaPending = pending
    return pending
  }

  function cancel() {
    if (status === 'verifying') return
    generation += 1
    hide()
    const error = new Error('closed')
    error.code = 'closed'
    finishWait(error)
    onChange()
  }

  function ensure() {
    if (token) return Promise.resolve(token)
    if (pendingEnsure) {
      show()
      return pendingEnsure
    }
    pendingEnsure = new Promise((resolve, reject) => {
      waiter = { resolve, reject }
    })
    show()
    load()
    return pendingEnsure
  }

  function moveTo(next) {
    fraction = Math.min(1, Math.max(0, next))
    paint()
  }

  if (handle) {
    handle.addEventListener('pointerdown', (event) => {
      if (status !== 'idle' || !challenge || !track) return
      try {
        event.currentTarget.setPointerCapture(event.pointerId)
      } catch {
        // 测试环境没有指针捕获。
      }
      drag = {
        startX: event.clientX,
        startFraction: fraction,
        startedAt: performance.now(),
        trajectory: [fraction],
        key: false
      }
    })
    handle.addEventListener('pointermove', (event) => {
      if (!drag || drag.key || status !== 'idle' || !track) return
      const total = track.clientWidth - event.currentTarget.clientWidth
      if (total <= 0) return
      moveTo(drag.startFraction + (event.clientX - drag.startX) / total)
      drag.trajectory.push(fraction)
    })
    const release = () => {
      const current = drag
      drag = null
      if (!current || current.key || !challenge || status !== 'idle') return
      const latest = current.trajectory[current.trajectory.length - 1] ?? fraction
      finish(pieceX(latest), Math.round(performance.now() - current.startedAt), current.trajectory.length)
    }
    handle.addEventListener('pointerup', release)
    handle.addEventListener('pointercancel', release)
    handle.addEventListener('keydown', (event) => {
      if (status !== 'idle' || !challenge) return
      if (event.key === 'ArrowLeft' || event.key === 'ArrowRight' || event.key === 'Home' || event.key === 'End') {
        event.preventDefault()
        if (!drag || !drag.key) {
          drag = { startedAt: performance.now(), trajectory: [], key: true }
        }
        drag.trajectory.push(fraction)
        const delta = event.key === 'ArrowRight' ? 0.025 : event.key === 'ArrowLeft' ? -0.025 : 0
        const next = event.key === 'Home' ? 0 : event.key === 'End' ? 1 : fraction + delta
        moveTo(next)
        return
      }
      if (event.key !== 'Enter' && event.key !== ' ') return
      event.preventDefault()
      const current = drag && drag.key ? drag : null
      drag = null
      const duration = current ? Math.round(performance.now() - current.startedAt) : 0
      finish(pieceX(fraction), duration, current ? current.trajectory.length : 1)
    })
  }
  if (refresh) {
    refresh.addEventListener('click', () => {
      if (refresh.disabled) return
      load(true)
    })
  }
  if (closer) closer.addEventListener('click', cancel)
  if (dialog) {
    dialog.addEventListener('click', (event) => {
      if (event.target === dialog) cancel()
    })
  }
  document.addEventListener('keydown', (event) => {
    if (event.key !== 'Escape' || !dialog || dialog.hidden) return
    event.preventDefault()
    cancel()
  })
  return {
    required: true,
    token: () => token,
    passed: () => status === 'success' && !token,
    ensure,
    clear() {
      token = ''
      if (dialog && !dialog.hidden) {
        load(true)
        return
      }
      generation += 1
      challenge = null
      status = 'idle'
      fraction = 0
      drag = null
      paint()
      onChange()
    },
    spend() {
      token = ''
      challenge = null
      status = 'idle'
      fraction = 0
      onChange()
    }
  }
}

export const CHERRY_MAIN_URL = 'http://127.0.0.1:4318/windows/main/index.html'

function openCherryMain(host) {
  const target = host.__A3S_STUDIO_MAIN_URL__ || CHERRY_MAIN_URL
  const assign = host.location && host.location.assign
  if (typeof assign === 'function') assign.call(host.location, target)
}

export function wireStudioLogin(document, host = globalThis) {
  const root = document.getElementById('login')
  if (!root || root.dataset.wired === 'true') return
  root.dataset.wired = 'true'
  const sms = document.getElementById('sms-fields')
  const passwordFields = document.getElementById('password-fields')
  const agree = document.getElementById('agree')
  const submit = document.getElementById('submit')
  const phone = document.getElementById('phone')
  const passwordPhone = document.getElementById('password-phone')
  const code = document.getElementById('code')
  const password = document.getElementById('password')
  const send = document.getElementById('send-code')
  const eye = document.getElementById('eye')
  const indicator = document.getElementById('tab-indicator')
  const formError = document.getElementById('form-error')
  const tabs = [...root.querySelectorAll('[role="tab"]')]

  function placeIndicator(tab) {
    if (!indicator || !tab || !tab.offsetWidth) return
    const x = tab.offsetLeft + (tab.offsetWidth - indicator.offsetWidth) / 2
    indicator.style.transform = `translateX(${x}px)`
  }

  function currentMode() {
    const selected = tabs.find((tab) => tab.getAttribute('aria-selected') === 'true')
    return selected?.dataset.mode === 'password' ? 'password' : 'sms'
  }

  function clearErrors() {
    for (const id of ['phone-error', 'code-error', 'password-phone-error', 'password-error']) {
      const node = document.getElementById(id)
      if (!node) continue
      node.textContent = ''
      node.hidden = true
    }
    if (formError) {
      formError.textContent = ''
      formError.hidden = true
    }
  }

  function showError(id, message) {
    const node = document.getElementById(id)
    if (!node) return
    node.textContent = message
    node.hidden = false
  }

  let puzzle = {
    required: false,
    token: () => '',
    passed: () => false,
    clear() {},
    spend() {},
    ensure: () => Promise.resolve('')
  }
  let sendInflight = false

  function syncSend() {
    if (!send || !phone) return
    const dialog = document.getElementById('captcha-dialog')
    const dialogOpen = Boolean(dialog && !dialog.hidden)
    send.disabled = sendInflight || dialogOpen || !validPhone(phone.value) || secondsLeft(chinaPhone(phone.value)) > 0
  }

  function select(next) {
    for (const tab of tabs) {
      tab.setAttribute('aria-selected', tab.dataset.mode === next ? 'true' : 'false')
    }
    if (sms) sms.hidden = next !== 'sms'
    if (passwordFields) passwordFields.hidden = next !== 'password'
    if (agree) agree.hidden = next !== 'sms'
    if (submit) submit.textContent = next === 'sms' ? '登录 / 注册' : '登录'
    placeIndicator(tabs.find((tab) => tab.dataset.mode === next))
    clearErrors()
  }

  for (const tab of tabs) {
    tab.addEventListener('click', () => select(tab.dataset.mode === 'password' ? 'password' : 'sms'))
  }

  function bindDigits(input, map) {
    if (!input) return
    input.addEventListener('input', () => {
      const next = map(input.value)
      if (input.value !== next) input.value = next
      syncSend()
    })
  }
  bindDigits(phone, chinaPhone)
  bindDigits(passwordPhone, chinaPhone)
  bindDigits(code, (value) => value.replace(/\D/g, '').slice(0, 6))

  function bindClear(input) {
    if (!input) return
    const button = input.closest('.combo')?.querySelector('button.field-clear')
    if (!button) return
    const sync = () => {
      button.hidden = input.value.length === 0
    }
    input.addEventListener('input', sync)
    button.addEventListener('mousedown', (event) => {
      event.preventDefault()
    })
    button.addEventListener('click', () => {
      input.value = ''
      input.dispatchEvent(new Event('input', { bubbles: true }))
      input.focus()
    })
    sync()
  }
  bindClear(phone)
  bindClear(code)
  bindClear(passwordPhone)
  bindClear(password)
  function showLogin() {
    if (document.documentElement.dataset.presentationDeck === 'shown') {
      document.documentElement.dataset.shellReady = 'true'
      document.documentElement.style.visibility = 'visible'
      return
    }
    root.hidden = false
    document.documentElement.dataset.shellReady = 'true'
    document.documentElement.style.visibility = 'visible'
    placeIndicator(tabs.find((tab) => tab.getAttribute('aria-selected') === 'true'))
  }

  // The card stays hidden until this launch is known to have no session.
  // A new loopback port cannot see the previous localStorage entry.
  root.hidden = true
  placeIndicator(tabs.find((tab) => tab.getAttribute('aria-selected') === 'true'))

  if (eye && password) {
    eye.addEventListener('click', () => {
      const shown = password.type === 'text'
      password.type = shown ? 'password' : 'text'
      eye.setAttribute('aria-pressed', shown ? 'false' : 'true')
      eye.setAttribute('aria-label', shown ? '显示密码' : '隐藏密码')
    })
  }

  let cooldown = readCooldown(host)
  let tick = 0

  function secondsLeft(digits) {
    if (!cooldown || cooldown.phone !== digits || cooldown.until <= Date.now()) return 0
    return Math.max(0, Math.ceil((cooldown.until - Date.now()) / 1000))
  }

  function paintSend() {
    if (!send || !phone) return
    const left = secondsLeft(chinaPhone(phone.value))
    if (left > 0) {
      send.disabled = true
      send.textContent = `${left} 秒后重发`
      return
    }
    send.textContent = '发送验证码'
    syncSend()
  }

  function armCooldown(digits, seconds) {
    cooldown = { phone: digits, until: Date.now() + Math.min(seconds, 120) * 1000 }
    try {
      host.sessionStorage?.setItem(SMS_COOLDOWN_KEY, JSON.stringify(cooldown))
    } catch {
      // 当次倒计时仍留在内存里。
    }
    paintSend()
    if (tick) (host.clearInterval || clearInterval)(tick)
    tick = (host.setInterval || setInterval)(() => {
      if (!cooldown || cooldown.until <= Date.now()) {
        cooldown = null
        try {
          host.sessionStorage?.removeItem(SMS_COOLDOWN_KEY)
        } catch {
          // 忽略不可用的会话存储。
        }
        ;(host.clearInterval || clearInterval)(tick)
        tick = 0
      }
      paintSend()
    }, 1000)
  }

  puzzle = wireStudioCaptcha(document, host, () => paintSend())
  paintSend()
  if (cooldown) armCooldown(cooldown.phone, Math.max(1, Math.ceil((cooldown.until - Date.now()) / 1000)))

  if (send) {
    send.addEventListener('click', () => {
      const digits = chinaPhone(phone?.value ?? '')
      if (!validPhone(digits) || send.disabled) return
      clearErrors()
      sendInflight = true
      paintSend()
      const deliver = (proof) => {
        const body = puzzle.required ? { phone: digits, captchaToken: proof } : { phone: digits }
        return orgPost(host, host.fetch, '/v1/api/org/members/phone-codes', body).then((data) => {
          armCooldown(digits, Number(data && data.cooldown_seconds) || 60)
          showDevCode(document, host, data && data.dev_code)
          puzzle.spend()
          code?.focus()
        })
      }
      const pending = (puzzle.required ? puzzle.ensure() : Promise.resolve(''))
        .then(deliver)
        .catch((err) => {
          if (err && err.code === 'closed') return
          if (formError) {
            formError.textContent = sendCodeError(err)
            formError.hidden = false
          }
          if (!err || err.code !== 0) puzzle.clear()
        })
        .finally(() => {
          sendInflight = false
          paintSend()
        })
      send._send = pending
    })
  }

  const form = document.getElementById('login-form')
  if (form) {
    form.addEventListener('submit', (event) => {
      event.preventDefault()
      if (submit?.disabled) return
      clearErrors()
      const mode = currentMode()
      const box = document.getElementById('agree-box')
      if (box && !box.checked) {
        if (formError) {
          formError.textContent = '请先勾选同意用户协议与隐私政策'
          formError.hidden = false
        }
        return
      }
      const source = mode === 'sms' ? phone : passwordPhone
      const digits = chinaPhone(source?.value ?? '')
      let invalid = false
      if (!validPhone(digits)) {
        showError(mode === 'sms' ? 'phone-error' : 'password-phone-error', '请输入正确的 11 位中国大陆手机号')
        invalid = true
      }
      if (mode === 'sms' && !/^\d{6}$/.test(code?.value ?? '')) {
        showError('code-error', '请输入短信验证码')
        invalid = true
      }
      if (mode === 'password' && (password?.value.length ?? 0) < 8) {
        showError('password-error', '密码至少 8 位')
        invalid = true
      }
      if (invalid || !submit) return
      const postLogin = (proof) => {
        const previous = submit.textContent
        submit.disabled = true
        submit.textContent = '登录中…'
        form.setAttribute('aria-busy', 'true')
        const path = mode === 'sms' ? '/v1/api/org/members/code-sessions' : '/v1/api/org/members/sessions'
        const body =
          mode === 'sms'
            ? { phone: digits, code: code.value.trim() }
            : { identifier: digits, password: password.value, captchaToken: proof }
        const pending = orgPost(host, host.fetch, path, body)
          .then((auth) => {
            const profile = rememberMember(host, auth)
            return saveMemberSession(host, auth.token, profile).then(() => openCherryMain(host))
          })
          .catch((err) => {
            if (formError) {
              formError.textContent = loginErrorMessage(err)
              formError.hidden = false
            }
            if (mode === 'password' && (!err || err.code !== 0)) puzzle.clear()
            submit.disabled = false
            submit.textContent = previous
            form.setAttribute('aria-busy', 'false')
          })
        submit._open = pending
        return pending
      }
      if (mode === 'password' && puzzle.required && !puzzle.token()) {
        submit.disabled = true
        submit._open = puzzle
          .ensure()
          .then((proof) => {
            submit.disabled = false
            return postLogin(proof)
          })
          .catch((err) => {
            submit.disabled = false
            submit.textContent = '登录'
            form.setAttribute('aria-busy', 'false')
            if (err && err.code === 'closed') return
            if (formError) {
              formError.textContent = loginErrorMessage(err)
              formError.hidden = false
            }
          })
        return
      }
      postLogin(puzzle.token())
    })
  }

  root._restore = withTimeout(restoreMember(host), SESSION_RESTORE_MS, host).then(
    (profile) => {
      if (profile) {
        openCherryMain(host)
        return profile
      }
      showLogin()
      return null
    },
    () => {
      showLogin()
      return null
    }
  )
}

export async function readPresentationDeck(host = globalThis, fetchImpl = host.fetch) {
  const runtime = readStudioDescriptor(host)
  const response = await fetchImpl(`${runtime.apiBaseUrl}/api/presentation-deck`, {
    method: 'GET',
    headers: { authorization: `Bearer ${runtime.token}` }
  })
  if (response.status === 404) return null
  if (!response.ok) return null
  const body = await response.json()
  if (!body || !Array.isArray(body.slides) || body.slides.length < 2) return null
  return body
}

/** Finished slides for this launch. A missing deck leaves the login card in place. */
export function paintPresentationDeck(document, deck) {
  const root = document.getElementById('presentation-deck')
  if (!root || !deck || !Array.isArray(deck.slides) || deck.slides.length < 2) return false
  const pages = []
  for (const slide of deck.slides) {
    const heading = typeof slide?.heading === 'string' ? slide.heading.trim() : ''
    const body = typeof slide?.body === 'string' ? slide.body.trim() : ''
    if (!heading || !body) return false
    pages.push({ heading, body })
  }
  root.replaceChildren()
  const title = document.createElement('h1')
  title.textContent = typeof deck.title === 'string' ? deck.title : ''
  const list = document.createElement('ol')
  for (const [index, page] of pages.entries()) {
    const item = document.createElement('li')
    item.setAttribute('data-presentation-slide', String(index + 1))
    const heading = document.createElement('h2')
    heading.textContent = page.heading
    const body = document.createElement('p')
    body.textContent = page.body
    item.append(heading, body)
    list.append(item)
  }
  root.append(title, list)
  root.hidden = false
  for (const id of ['login', 'provider-setup', 'model-setup']) {
    const node = document.getElementById(id)
    if (node) node.hidden = true
  }
  document.documentElement.dataset.presentationDeck = 'shown'
  document.documentElement.dataset.shellReady = 'true'
  document.documentElement.style.visibility = 'visible'
  return true
}

export async function bootStudioShell(host = globalThis, document = host.document, fetchImpl = host.fetch) {
  try {
    const bootstrap = await readStudioBootstrap(host, fetchImpl)
    paintStudioWelcome(document, bootstrap)
    const deck = await readPresentationDeck(host, fetchImpl)
    if (deck) paintPresentationDeck(document, deck)
  } catch {
    return
  }
}

const MANAGED_PROVIDER_IDS = new Set(['cherryai', 'cherryai-subscription'])

function providerState(document) {
  const node = document.getElementById('provider-setup')
  if (!node) return null
  if (!node._state) {
    node._state = { providers: [], selected: '', defaults: {}, detail: null }
  }
  return node._state
}

function showStudioStep(document, step) {
  const login = document.getElementById('login')
  const provider = document.getElementById('provider-setup')
  const model = document.getElementById('model-setup')
  if (login) login.hidden = step !== 'login'
  if (provider) provider.hidden = step !== 'provider'
  if (model) model.hidden = step !== 'model'
}

function showNote(document, id, message) {
  const node = document.getElementById(id)
  if (!node) return
  node.textContent = message || ''
  node.hidden = !message
}

function providerMessage(code) {
  if (code === 'missing_api_key') return '请先填写 API 密钥'
  if (code === 'missing_base_url' || code === 'bad_base_url') return 'API 地址不合法'
  if (code === 'sync_failed' || code === 'sync_unavailable') return '同步模型失败'
  if (code === 'missing_model') return '请先启用该服务商下的至少一个模型'
  if (code === 'not_found') return '找不到这个服务商'
  return '无法保存服务商设置'
}

function gateMessage(providers) {
  const enabled = providers.filter((provider) => provider.enabled && !MANAGED_PROVIDER_IDS.has(provider.id))
  if (enabled.length === 0) return '请先启用一个服务商'
  if (!enabled.some((provider) => (provider.chatModels || []).length > 0 || provider.enabledChatModels > 0)) {
    return '请先启用该服务商下的至少一个模型'
  }
  return ''
}

function eligibleModels(providers) {
  const models = []
  for (const provider of providers) {
    if (!provider.enabled || MANAGED_PROVIDER_IDS.has(provider.id)) continue
    for (const model of provider.chatModels || []) {
      models.push({
        providerId: provider.id,
        providerName: provider.name,
        id: model.id,
        name: model.name || model.id
      })
    }
  }
  return models
}

async function studioFetch(host, fetchImpl, path, options = {}) {
  const runtime = readStudioDescriptor(host)
  const response = await fetchImpl(`${runtime.apiBaseUrl}${path}`, {
    method: options.method || 'GET',
    headers: {
      authorization: `Bearer ${runtime.token}`,
      'content-type': 'application/json'
    },
    body: options.body === undefined ? undefined : JSON.stringify(options.body)
  })
  const text = await response.text()
  let data = {}
  if (text) {
    try {
      data = JSON.parse(text)
    } catch {
      data = {}
    }
  }
  if (!response.ok) {
    const error = new Error(data.error || 'request_failed')
    error.code = data.error || 'request_failed'
    throw error
  }
  return data
}

function rememberProvider(state, detail) {
  const row = state.providers.find((provider) => provider.id === detail.id)
  if (!row) return
  row.enabled = Boolean(detail.enabled)
  row.hasApiKey = Boolean(detail.apiKey)
  row.baseUrl = detail.baseUrl || row.baseUrl
  row.chatModels = (detail.models || [])
    .filter((model) => model.enabled && model.chat)
    .map((model) => ({ id: model.id, name: model.name || model.id }))
  row.enabledChatModels = row.chatModels.length
}

function renderProviderList(document) {
  const state = providerState(document)
  const items = document.getElementById('provider-items')
  const search = document.getElementById('provider-search')
  if (!state || !items) return
  const query = (search?.value || '').trim().toLowerCase()
  items.replaceChildren()
  for (const provider of state.providers) {
    const haystack = `${provider.name} ${provider.id}`.toLowerCase()
    if (query && !haystack.includes(query)) continue
    const button = document.createElement('button')
    button.type = 'button'
    button.className = 'provider-item'
    button.setAttribute('role', 'option')
    button.setAttribute('aria-selected', provider.id === state.selected ? 'true' : 'false')
    const name = document.createElement('span')
    name.textContent = provider.name
    button.append(name)
    if (provider.enabled) {
      const mark = document.createElement('span')
      mark.className = 'provider-on'
      mark.textContent = '已启用'
      button.append(mark)
    }
    button.addEventListener('click', () => {
      void selectProvider(document, provider.id)
    })
    items.append(button)
  }
}

function renderProviderModels(document, models) {
  const list = document.getElementById('provider-models')
  if (!list) return
  list.replaceChildren()
  if (!models || models.length === 0) {
    const empty = document.createElement('p')
    empty.className = 'note'
    empty.textContent = '点击上方的“同步模型”添加模型'
    list.append(empty)
    return
  }
  for (const model of models) {
    const row = document.createElement('label')
    row.className = 'model-row'
    const box = document.createElement('input')
    box.type = 'checkbox'
    box.checked = Boolean(model.enabled)
    box.addEventListener('change', () => {
      void updateProviderModel(document, model.id, box.checked).catch(() => {
        box.checked = Boolean(model.enabled)
      })
    })
    const name = document.createElement('span')
    name.textContent = model.name || model.id
    row.append(box, name)
    if (!model.chat) {
      const tag = document.createElement('span')
      tag.className = 'note'
      tag.textContent = '非对话'
      row.append(tag)
    }
    list.append(row)
  }
}

function refreshProviderGate(document) {
  const state = providerState(document)
  const next = document.getElementById('provider-next')
  const message = gateMessage(state?.providers || [])
  showNote(document, 'provider-hint', message)
  if (next) next.setAttribute('aria-disabled', message ? 'true' : 'false')
}

function applyProviderDetail(document, detail, keepForm) {
  const state = providerState(document)
  if (!state || !detail) return
  state.detail = detail
  state.selected = detail.id
  rememberProvider(state, detail)
  renderProviderList(document)
  const title = document.getElementById('provider-name')
  if (title) title.textContent = detail.name || ''
  const link = document.getElementById('provider-key-link')
  if (link) {
    const url = typeof detail.apiKeyUrl === 'string' ? detail.apiKeyUrl : ''
    link.textContent = url ? `密钥页面 ${url}` : ''
    link.hidden = !url
  }
  if (!keepForm) {
    const host = document.getElementById('provider-host')
    const key = document.getElementById('provider-key')
    const enabled = document.getElementById('provider-enabled')
    if (host) host.value = detail.baseUrl || ''
    if (key) key.value = detail.apiKey || ''
    if (enabled) enabled.checked = Boolean(detail.enabled)
  }
  renderProviderModels(document, detail.models || [])
  refreshProviderGate(document)
}

async function selectProvider(document, id, host, fetchImpl) {
  const state = providerState(document)
  if (!state || (state.selected === id && state.detail?.id === id)) {
    renderProviderList(document)
    return
  }
  host = host || state.host || document.defaultView || globalThis
  fetchImpl = fetchImpl || state.fetchImpl || host.fetch
  state.selected = id
  renderProviderList(document)
  const detail = await studioFetch(host, fetchImpl, `/api/providers/${id}`)
  if (providerState(document)?.selected !== id) return
  applyProviderDetail(document, detail, false)
}

function providerFormBody(document) {
  return {
    enabled: Boolean(document.getElementById('provider-enabled')?.checked),
    apiKey: document.getElementById('provider-key')?.value ?? '',
    baseUrl: (document.getElementById('provider-host')?.value || '').trim()
  }
}

async function saveProvider(document, host, fetchImpl) {
  const state = providerState(document)
  if (!state?.selected) return null
  const body = providerFormBody(document)
  if (body.baseUrl && !/^https?:\/\/\S+$/.test(body.baseUrl)) {
    const error = new Error('bad_base_url')
    error.code = 'bad_base_url'
    throw error
  }
  const detail = await studioFetch(host, fetchImpl, `/api/providers/${state.selected}`, {
    method: 'POST',
    body
  })
  applyProviderDetail(document, detail, false)
  return detail
}

async function updateProviderModel(document, id, enabled) {
  const state = providerState(document)
  const host = state?.host || document.defaultView || globalThis
  const fetchImpl = state?.fetchImpl || host.fetch
  if (!state?.selected) return
  const detail = await studioFetch(host, fetchImpl, `/api/providers/${state.selected}/models`, {
    method: 'POST',
    body: { id, enabled }
  })
  applyProviderDetail(document, detail, true)
}

function fillModelStep(document) {
  const state = providerState(document)
  const models = eligibleModels(state?.providers || [])
  const defaults = state?.defaults || {}
  const fields = [
    ['model-default', defaults.defaultModel],
    ['model-quick', defaults.quickModel],
    ['model-translate', defaults.translateModel]
  ]
  const allEmpty = fields.every(([, value]) => !value || !value.providerId || !value.modelId)
  const options = models.map((model) => ({
    value: `${model.providerId}\t${model.id}`,
    label: `${model.providerName} / ${model.name}`
  }))
  for (const [id, value] of fields) {
    const picker = document.getElementById(id)
    if (!picker) continue
    const chosen = value && models.some((model) => model.providerId === value.providerId && model.id === value.modelId)
    const initial = chosen ? `${value.providerId}\t${value.modelId}` : allEmpty && options[0] ? options[0].value : ''
    renderPicker(document, picker, options, initial)
  }
}

const PICKER_CHEVRON =
  '<svg class="picker-chevron" width="12" height="12" viewBox="0 0 12 12" fill="none" aria-hidden="true"><path d="M3 4.5 6 7.5l3-3" stroke="currentColor" stroke-width="1.5" stroke-linecap="round" stroke-linejoin="round"/></svg>'
const PICKER_CHECK =
  '<svg class="picker-check" width="14" height="14" viewBox="0 0 14 14" fill="none" aria-hidden="true"><path d="m3 7.5 2.5 2.5L11 4.5" stroke="currentColor" stroke-width="1.6" stroke-linecap="round" stroke-linejoin="round"/></svg>'

export function renderPicker(document, root, options, value) {
  if (!root.dataset.mounted) mountPicker(document, root)
  root._options = options
  root.dataset.value = value
  const panel = root.querySelector('.picker-panel')
  panel.replaceChildren()
  for (const option of options) {
    const row = document.createElement('button')
    row.type = 'button'
    row.className = 'picker-option'
    row.setAttribute('role', 'option')
    row.dataset.value = option.value
    row.innerHTML = PICKER_CHECK
    const text = document.createElement('span')
    text.textContent = option.label
    row.append(text)
    panel.append(row)
  }
  syncPicker(root)
}

function syncPicker(root) {
  const value = root.dataset.value || ''
  const chosen = (root._options || []).find((option) => option.value === value)
  root.querySelector('.picker-text').textContent = chosen ? chosen.label : ''
  for (const row of root.querySelectorAll('.picker-option')) {
    row.setAttribute('aria-selected', row.dataset.value === value ? 'true' : 'false')
  }
}

function setPickerOpen(root, open) {
  const trigger = root.querySelector('.picker-trigger')
  const panel = root.querySelector('.picker-panel')
  trigger.setAttribute('aria-expanded', open ? 'true' : 'false')
  panel.hidden = !open
  if (!open) return
  const rows = [...panel.querySelectorAll('.picker-option')]
  ;(rows.find((row) => row.getAttribute('aria-selected') === 'true') || rows[0])?.focus()
}

function mountPicker(document, root) {
  root.dataset.mounted = 'true'
  const labelledBy = root.getAttribute('aria-labelledby') || ''
  const panelId = `${root.id}-listbox`
  const textId = `${root.id}-text`
  root.innerHTML =
    `<button type="button" class="picker-trigger" aria-haspopup="listbox" aria-expanded="false" aria-controls="${panelId}" aria-labelledby="${labelledBy} ${textId}">` +
    `<span class="picker-text" id="${textId}" data-placeholder="请选择"></span>${PICKER_CHEVRON}</button>` +
    `<div class="picker-panel" id="${panelId}" role="listbox" aria-labelledby="${labelledBy}" hidden></div>`
  const trigger = root.querySelector('.picker-trigger')
  const panel = root.querySelector('.picker-panel')
  trigger.addEventListener('click', () => setPickerOpen(root, panel.hidden))
  trigger.addEventListener('keydown', (event) => {
    if (event.key === 'ArrowDown' || event.key === 'ArrowUp') {
      event.preventDefault()
      setPickerOpen(root, true)
    }
  })
  panel.addEventListener('click', (event) => {
    const row = event.target.closest?.('.picker-option')
    if (!row) return
    root.dataset.value = row.dataset.value
    syncPicker(root)
    setPickerOpen(root, false)
    trigger.focus()
  })
  panel.addEventListener('keydown', (event) => {
    const rows = [...panel.querySelectorAll('.picker-option')]
    const at = rows.indexOf(document.activeElement)
    if (event.key === 'ArrowDown' || event.key === 'ArrowUp') {
      event.preventDefault()
      const step = event.key === 'ArrowDown' ? 1 : -1
      rows[Math.min(rows.length - 1, Math.max(0, at + step))]?.focus()
    } else if (event.key === 'Home' || event.key === 'End') {
      event.preventDefault()
      rows[event.key === 'Home' ? 0 : rows.length - 1]?.focus()
    } else if (event.key === 'Escape' || event.key === 'Tab') {
      if (event.key === 'Escape') event.preventDefault()
      setPickerOpen(root, false)
      if (event.key === 'Escape') trigger.focus()
    }
  })
  document.addEventListener('pointerdown', (event) => {
    if (!panel.hidden && !root.contains(event.target)) setPickerOpen(root, false)
  })
}

function readModelChoice(picker) {
  const value = picker?.dataset?.value || ''
  const splitAt = value.indexOf('\t')
  if (splitAt <= 0) return null
  return { providerId: value.slice(0, splitAt), modelId: value.slice(splitAt + 1) }
}

export async function openProviderSetup(document, host = globalThis, fetchImpl = host.fetch) {
  if (!document.getElementById('provider-setup')) return
  showStudioStep(document, 'provider')
  showNote(document, 'provider-error', '')
  const state = providerState(document)
  try {
    const list = await studioFetch(host, fetchImpl, '/api/providers')
    state.host = host
    state.fetchImpl = fetchImpl
    state.providers = Array.isArray(list.providers) ? list.providers : []
    state.defaults = list.defaults || {}
    state.selected = ''
    state.detail = null
    renderProviderList(document)
    refreshProviderGate(document)
    if (state.providers[0]) await selectProvider(document, state.providers[0].id, host, fetchImpl)
    document.getElementById('provider-search')?.focus()
  } catch (error) {
    showNote(document, 'provider-error', error?.code ? providerMessage(error.code) : '无法读取服务商，请重试')
  }
}

async function completeModelSetup(document, host, fetchImpl) {
  const body = {
    defaultModel: readModelChoice(document.getElementById('model-default')),
    quickModel: readModelChoice(document.getElementById('model-quick')),
    translateModel: readModelChoice(document.getElementById('model-translate'))
  }
  if (!body.defaultModel || !body.quickModel || !body.translateModel) {
    showNote(document, 'model-error', '请先启用该服务商下的至少一个模型')
    return
  }
  try {
    const saved = await studioFetch(host, fetchImpl, '/api/providers/defaults', { method: 'POST', body })
    const state = providerState(document)
    if (state) state.defaults = saved
    showNote(document, 'model-error', '')
    showStudioStep(document, 'login')
  } catch {
    showNote(document, 'model-error', '无法完成引导，请重试')
  }
}

export function wireStudioProviders(document, host = globalThis, fetchImpl = host.fetch) {
  const button = document.getElementById('providers')
  const setup = document.getElementById('provider-setup')
  if (!button || !setup || button.dataset.wired === 'providers') return
  button.dataset.wired = 'providers'
  const runtimeHost = host
  const fetch = fetchImpl
  button.addEventListener('click', () => {
    button._open = openProviderSetup(document, runtimeHost, fetch)
  })
  document.getElementById('provider-search')?.addEventListener('input', () => renderProviderList(document))
  for (const id of ['provider-back', 'provider-cancel']) {
    document.getElementById(id)?.addEventListener('click', () => showStudioStep(document, 'login'))
  }
  document.getElementById('provider-save')?.addEventListener('click', () => {
    void saveProvider(document, runtimeHost, fetch).catch((error) => {
      showNote(document, 'provider-error', providerMessage(error.code))
    })
  })
  document.getElementById('provider-enabled')?.addEventListener('change', () => {
    void saveProvider(document, runtimeHost, fetch).catch((error) => {
      showNote(document, 'provider-error', providerMessage(error.code))
    })
  })
  document.getElementById('provider-sync')?.addEventListener('click', () => {
    const sync = document.getElementById('provider-sync')
    sync.disabled = true
    sync.textContent = '正在同步'
    sync._open = saveProvider(document, runtimeHost, fetch)
      .then(() => {
        const state = providerState(document)
        return studioFetch(runtimeHost, fetch, `/api/providers/${state.selected}/sync`, { method: 'POST', body: {} })
      })
      .then((detail) => {
        applyProviderDetail(document, detail, true)
        if (!detail.models || detail.models.length === 0) showNote(document, 'provider-error', '没有同步到模型')
        else showNote(document, 'provider-error', '')
      })
      .catch((error) => {
        showNote(document, 'provider-error', providerMessage(error.code))
      })
      .finally(() => {
        sync.disabled = false
        sync.textContent = '同步模型'
      })
  })
  const add = () => {
    const input = document.getElementById('provider-model-id')
    const id = input?.value.trim()
    const state = providerState(document)
    if (!id || !state?.selected) return
    void studioFetch(runtimeHost, fetch, `/api/providers/${state.selected}/models`, {
      method: 'POST',
      body: { id }
    })
      .then((detail) => {
        if (input) input.value = ''
        applyProviderDetail(document, detail, true)
        showNote(document, 'provider-error', '')
      })
      .catch((error) => showNote(document, 'provider-error', providerMessage(error.code)))
  }
  document.getElementById('provider-add-model')?.addEventListener('click', add)
  document.getElementById('provider-model-id')?.addEventListener('keydown', (event) => {
    if (event.key === 'Enter') {
      event.preventDefault()
      add()
    }
  })
  document.getElementById('provider-next')?.addEventListener('click', () => {
    const message = gateMessage(providerState(document)?.providers || [])
    refreshProviderGate(document)
    if (message) return
    showNote(document, 'model-error', '')
    fillModelStep(document)
    showStudioStep(document, 'model')
  })
  document.getElementById('model-back')?.addEventListener('click', () => showStudioStep(document, 'provider'))
  const start = document.getElementById('model-start')
  start?.addEventListener('click', () => {
    start._open = completeModelSetup(document, runtimeHost, fetch)
  })
}

export function wireShell(host = globalThis) {
  const document = host.document
  if (!document || typeof document.getElementById !== 'function') return
  if (!document.getElementById('login')) return
  wireStudioLogin(document, host)
  wireStudioProviders(document, host, host.fetch)
  void bootStudioShell(host, document, host.fetch)
}

wireShell()
