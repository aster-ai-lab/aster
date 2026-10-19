import { afterEach, describe, expect, it, vi } from 'vitest'

import html from './index.html?raw'
import {
  CHERRY_MAIN_URL,
  paintPresentationDeck,
  paintStudioWelcome,
  readPresentationDeck,
  readStudioBootstrap,
  renderPicker,
  wireStudioLogin
} from './studioClient.mjs'
import source from './studioClient.mjs?raw'

afterEach(() => {
  vi.unstubAllGlobals()
  document.body.innerHTML = ''
  window.localStorage.clear()
  window.sessionStorage.clear()
  delete document.documentElement.dataset.presentationDeck
  delete document.documentElement.dataset.shellReady
  document.documentElement.style.visibility = ''
})

describe('studio shell client', () => {
  it('fails closed when the frozen descriptor is absent or invalid', async () => {
    const fetchMock = vi.fn()
    await expect(readStudioBootstrap({} as typeof globalThis, fetchMock)).rejects.toThrow(/absent/)
    await expect(
      readStudioBootstrap(
        { __A3S_STUDIO_RUNTIME__: { apiBaseUrl: 'https://example.com', token: 'ab'.repeat(32) } } as typeof globalThis,
        fetchMock
      )
    ).rejects.toThrow(/invalid/)
    await expect(
      readStudioBootstrap(
        { __A3S_STUDIO_RUNTIME__: { apiBaseUrl: 'http://127.0.0.1:9', token: 'AB'.repeat(32) } } as typeof globalThis,
        fetchMock
      )
    ).rejects.toThrow(/invalid/)
    expect(fetchMock).not.toHaveBeenCalled()
  })

  it('reads bootstrap with the bearer and paints the returned product', async () => {
    const fetchMock = vi.fn(async () => ({
      ok: true,
      json: async () => ({
        sessionId: 'session-from-authority',
        name: 'Probe Tenant 7f3a',
        product: 'Probe Product 7f3a'
      })
    }))
    const body = await readStudioBootstrap(
      { __A3S_STUDIO_RUNTIME__: { apiBaseUrl: 'http://127.0.0.1:9', token: 'cd'.repeat(32) } } as typeof globalThis,
      fetchMock
    )
    expect(body).toMatchObject({
      sessionId: 'session-from-authority',
      name: 'Probe Tenant 7f3a',
      product: 'Probe Product 7f3a'
    })
    expect(fetchMock).toHaveBeenCalledWith(
      'http://127.0.0.1:9/api/bootstrap',
      expect.objectContaining({
        method: 'GET',
        headers: { authorization: `Bearer ${'cd'.repeat(32)}` }
      })
    )
    document.body.innerHTML = '<h1 id="product"></h1>'
    paintStudioWelcome(document, body)
    expect(document.getElementById('product')?.textContent).toBe('Probe Product 7f3a')
    expect(document.getElementById('product')?.querySelector('svg')).toBeNull()
    expect(document.title).toBe('Probe Product 7f3a')
  })

  it('paints every finished slide and leaves login hidden', () => {
    document.body.innerHTML = `
      <main id="presentation-deck" hidden></main>
      <main id="login"></main>
      <section id="provider-setup"></section>
      <section id="model-setup"></section>`
    const painted = paintPresentationDeck(document, {
      title: '量子计算入门',
      slides: [
        { heading: '量子计算入门', body: '叠加与纠缠可以同时表示多种可能。' },
        { heading: '第 2 部分', body: '测量会让叠加落到一个结果。' }
      ]
    })
    expect(painted).toBe(true)
    expect(document.documentElement.dataset.presentationDeck).toBe('shown')
    expect(document.getElementById('login')?.hidden).toBe(true)
    expect(document.querySelectorAll('[data-presentation-slide]')).toHaveLength(2)
    expect(document.querySelector('h2')?.textContent).toBe('量子计算入门')
    expect(document.querySelector('[data-presentation-slide="2"] p')?.textContent).toContain('测量')
  })

  it('does not replace login when the deck has an empty page', async () => {
    const fetchMock = vi.fn(async () => ({
      ok: true,
      status: 200,
      json: async () => ({ slides: [{ heading: '只有标题', body: ' ' }] })
    }))
    const deck = await readPresentationDeck(
      { __A3S_STUDIO_RUNTIME__: { apiBaseUrl: 'http://127.0.0.1:9', token: 'cd'.repeat(32) } } as typeof globalThis,
      fetchMock
    )
    expect(deck).toBeNull()
    document.body.innerHTML = '<main id="presentation-deck" hidden></main><main id="login"></main>'
    expect(paintPresentationDeck(document, { slides: [{ heading: '只有标题', body: '' }] })).toBe(false)
    expect(document.getElementById('login')?.hidden).toBe(false)
  })

  it('paints the ToWork geometric wordmark instead of the product name', () => {
    document.body.innerHTML = '<h1 id="product">ToWork</h1>'
    paintStudioWelcome(document, { product: 'ToWork' })
    const product = document.getElementById('product')
    const mark = product?.querySelector('[data-ui="workbench.wordmark.mark"]')
    expect(product?.getAttribute('aria-label')).toBe('ToWork')
    expect(product?.textContent).toBe('')
    expect(mark?.getAttribute('viewBox')).toBe('140 116 2164 512')
    expect(mark?.getAttribute('stroke')).toBe('#303030')
    expect(mark?.querySelector('circle')?.getAttribute('r')).toBe('142')
    expect(document.title).toBe('ToWork')
  })

  it('switches the login card between phone and password and keeps digits', () => {
    document.body.innerHTML = `
      <main id="login">
        <button type="button" role="tab" data-mode="sms" aria-selected="true">手机号登录</button>
        <button type="button" role="tab" data-mode="password" aria-selected="false">密码登录</button>
        <span id="tab-indicator"></span>
        <div id="sms-fields"><input id="phone" /><input id="code" /><button id="send-code" disabled></button></div>
        <div id="password-fields" hidden><input id="password-phone" /><input id="password" type="password" /></div>
        <button id="eye" type="button" aria-label="显示密码"></button>
        <button id="submit" type="submit">登录 / 注册</button>
        <div id="agree"><input id="agree-box" type="checkbox" checked /></div>
        <p id="form-error" hidden></p>
        <form id="login-form"></form>
      </main>`
    wireStudioLogin(document)
    const phone = document.getElementById('phone') as HTMLInputElement
    phone.value = '86 138-0013-8000abc'
    phone.dispatchEvent(new Event('input'))
    expect(phone.value).toBe('13800138000')
    expect((document.getElementById('send-code') as HTMLButtonElement).disabled).toBe(false)
    const passwordTab = document.querySelector('[data-mode="password"]') as HTMLButtonElement
    passwordTab.click()
    expect(document.getElementById('sms-fields')?.hidden).toBe(true)
    expect(document.getElementById('password-fields')?.hidden).toBe(false)
    expect(document.getElementById('agree')?.hidden).toBe(true)
    expect(document.getElementById('submit')?.textContent).toBe('登录')
    const eye = document.getElementById('eye') as HTMLButtonElement
    const password = document.getElementById('password') as HTMLInputElement
    eye.click()
    expect(password.type).toBe('text')
    expect(eye.getAttribute('aria-label')).toBe('隐藏密码')
  })

  it('shows a clear button only while a login field has text and empties that field', () => {
    document.body.innerHTML = `
      <main id="login">
        <div class="combo">
          <input id="phone" />
          <button type="button" class="field-clear" hidden aria-label="清空"></button>
        </div>
        <div class="combo">
          <input id="code" />
          <button type="button" class="field-clear" hidden aria-label="清空"></button>
          <button id="send-code" type="button" disabled></button>
        </div>
        <div id="password-fields" hidden>
          <div class="combo">
            <input id="password-phone" />
            <button type="button" class="field-clear" hidden aria-label="清空"></button>
          </div>
          <div class="combo">
            <input id="password" type="password" />
            <button type="button" class="field-clear" hidden aria-label="清空"></button>
            <button id="eye" type="button" aria-label="显示密码"></button>
          </div>
        </div>
        <button id="submit" type="submit">登录 / 注册</button>
      </main>`
    wireStudioLogin(document)
    const phone = document.getElementById('phone') as HTMLInputElement
    const clear = phone.closest('.combo')?.querySelector('button.field-clear') as HTMLButtonElement
    expect(clear.hidden).toBe(true)
    phone.value = '13800138000'
    phone.dispatchEvent(new Event('input'))
    expect(clear.hidden).toBe(false)
    clear.click()
    expect(phone.value).toBe('')
    expect(clear.hidden).toBe(true)
    expect(document.activeElement).toBe(phone)
    const code = document.getElementById('code') as HTMLInputElement
    const codeClear = code.closest('.combo')?.querySelector('button.field-clear') as HTMLButtonElement
    code.value = '123456'
    code.dispatchEvent(new Event('input'))
    expect(codeClear.hidden).toBe(false)
    codeClear.click()
    expect(code.value).toBe('')
    expect((document.getElementById('send-code') as HTMLButtonElement).disabled).toBe(true)
  })

  it('keeps the welcome page on the light theme without a preview control', () => {
    for (const text of [html, source]) {
      expect(text.toLowerCase()).not.toContain('<iframe')
      expect(text.toLowerCase()).not.toContain('<webview')
      expect(text).not.toContain('打开预览')
      expect(text).not.toContain('create_webview')
      expect(text).not.toContain('window.api')
      expect(text).not.toContain('electron')
    }
    const parsed = new DOMParser().parseFromString(html, 'text/html')
    expect(parsed.documentElement.getAttribute('data-theme')).toBe('light')
    expect(parsed.getElementById('embed')).toBeNull()
    expect(parsed.querySelector('header')).toBeNull()
    expect(parsed.getElementById('app-name')).toBeNull()
    expect(parsed.querySelector('#mark')).toBeNull()
    expect(parsed.querySelector('.brand img')).toBeNull()
    expect(parsed.querySelector('.qr img')?.getAttribute('src')).toBe('/wechat-qr.png')
    expect(parsed.getElementById('status')).toBeNull()
    expect(parsed.getElementById('welcome')).toBeNull()
    expect(parsed.getElementById('connect')).toBeNull()
    expect(parsed.getElementById('submit')?.textContent).toBe('登录 / 注册')
    expect(parsed.getElementById('local-use')).toBeNull()
    expect(html).not.toContain('本地使用')
    expect(parsed.getElementById('login')?.hidden).toBe(true)
    expect(parsed.getElementById('providers')).toBeNull()
    expect(html).not.toContain('配置其他服务商')
    expect(source).toContain('开发验证码')
    expect(parsed.getElementById('provider-setup')?.hidden).toBe(true)
    expect(parsed.getElementById('provider-title')?.textContent).toBe('选择服务商')
    expect(parsed.getElementById('provider-next')?.textContent).toBe('下一步')
    expect(parsed.getElementById('model-title')?.textContent).toBe('选择默认模型')
    expect(source).toContain('请先启用一个服务商')
    expect(source).toContain('请先启用该服务商下的至少一个模型')
    expect(html).toContain('微信扫码登录')
    expect(html).toContain('id="captcha"')
    expect(html).toContain('id="captcha-dialog"')
    expect(html).toContain('请完成安全验证')
    expect(html).toContain('向右拖动滑块完成拼图')
    expect(html).toContain('换一张')
    expect(parsed.getElementById('captcha-dialog')?.hidden).toBe(true)
    expect(parsed.getElementById('captcha-bg')?.getAttribute('alt')).toBe('验证背景')
    expect(parsed.getElementById('captcha-piece')?.getAttribute('alt')).toBe('验证拼图')
    expect(html).toContain('gap: 0.85rem')
    expect(html).toContain('class="combo-input"')
    expect(html).toContain("url('/login-bg.jpg')")
    expect(parsed.querySelectorAll('button.field-clear')).toHaveLength(4)
    expect(
      parsed.querySelector('#phone')?.closest('.combo')?.querySelector('button.field-clear')?.getAttribute('aria-label')
    ).toBe('清空')
    expect(parsed.querySelector('#code')?.closest('.combo')?.querySelector('button.field-clear')).not.toBeNull()
    expect(parsed.querySelector('#password')?.closest('.combo')?.querySelector('button.field-clear')).not.toBeNull()
    expect(
      parsed.querySelector('#password-phone')?.closest('.combo')?.querySelector('button.field-clear')
    ).not.toBeNull()
    expect(html).toContain('caret-color: #171717')
    expect(html).toContain('padding: 0 0.75rem')
    expect(html).toContain('border-radius: 0.5rem')
    expect(html).not.toContain('border-radius: 999px')
    expect(html).not.toContain('欢迎使用')
    expect(html).not.toContain('管理后台')
    expect(html).not.toContain('连接服务商仍由完整')
    const drag = parsed.getElementById('drag')
    expect(drag?.hasAttribute('data-tauri-drag-region')).toBe(true)
    expect(drag?.hasAttribute('data-window-drag-region')).toBe(true)
    expect(drag?.getAttribute('aria-hidden')).toBe('true')
    expect(drag?.textContent).toBe('')
    expect(html).toContain("html[data-overlay-titlebar='true'] #drag")
    expect(html).toContain('left: var(--a3s-traffic-light-inset, 0px)')
    expect(html).toContain('height: var(--a3s-titlebar-height, 56px)')
    expect(html).toContain('padding-top: var(--a3s-titlebar-height, 56px)')
    expect(html).toContain('user-select: none')
    expect(html).not.toContain('-webkit-app-region')
    expect(html).not.toContain('<header')
    expect(source).not.toContain('连接服务商仍由完整')
  })

  function mountShell() {
    const parsed = new DOMParser().parseFromString(html, 'text/html')
    parsed.querySelectorAll('script').forEach((node) => node.remove())
    document.body.innerHTML = parsed.body.innerHTML
  }

  function jsonResponse(body, ok = true) {
    return { ok, text: async () => JSON.stringify(body) }
  }

  function captchaEnvelope(url: string) {
    const target = String(url)
    if (target.endsWith('/captcha/challenges')) {
      return jsonResponse({
        code: 0,
        message: 'ok',
        data: {
          challengeId: '018f3b2a-7c31-7a3e-9b2a-6c1d0e5f4a11',
          background: '/captcha/bg-n060.jpg?v=3',
          piece: '/captcha/piece-n060.png?v=3',
          width: 600,
          height: 340,
          pieceW: 56,
          pieceH: 56,
          pad: 12,
          y: 40,
          tolerance: 6,
          maxAttempts: 3,
          expiresIn: 180
        }
      })
    }
    if (target.endsWith('/verifications')) {
      return jsonResponse({
        code: 0,
        message: 'ok',
        data: { verified: true, token: 'captcha_v1.test', expiresIn: 300 }
      })
    }
    return null
  }

  async function passCaptcha() {
    const login = document.getElementById('login') as HTMLElement & {
      _captchaReady?: Promise<void>
      _captchaPending?: Promise<void>
    }
    await login._captchaReady
    const handle = document.getElementById('captcha-handle') as HTMLElement
    for (const key of ['ArrowRight', 'ArrowRight', 'ArrowRight', 'ArrowRight', 'Enter']) {
      handle.dispatchEvent(new KeyboardEvent('keydown', { key, bubbles: true }))
    }
    await login._captchaPending
  }

  function loginHost(fetchMock: typeof fetch) {
    return {
      __A3S_STUDIO_RUNTIME__: { apiBaseUrl: 'http://127.0.0.1:9', token: 'ab'.repeat(32) },
      document,
      fetch: fetchMock,
      localStorage: window.localStorage,
      sessionStorage: window.sessionStorage,
      setInterval: () => 0,
      clearInterval: () => undefined,
      setTimeout: () => 0,
      clearTimeout: () => undefined
    }
  }

  it('shows the development code and signs in with the member code-login response', async () => {
    mountShell()
    const calls: Array<{ url: string; body?: string; authorization?: string }> = []
    const fetchMock = vi.fn(async (url: string, init?: { body?: string; headers?: { authorization?: string } }) => {
      calls.push({ url: String(url), body: init?.body, authorization: init?.headers?.authorization })
      const captcha = captchaEnvelope(String(url))
      if (captcha) return captcha
      if (String(url).endsWith('/phone-codes')) {
        return jsonResponse({
          code: 0,
          message: 'ok',
          data: { ok: true, cooldown_seconds: 60, dev_code: '654321' }
        })
      }
      return jsonResponse({
        code: 0,
        message: 'ok',
        data: {
          token: 'a3s_ms_session',
          expires_at_epoch: 1,
          member: { id: 'm1', email: '', phone: '13800138000', display_name: '林', status: 'active' }
        }
      })
    })
    const assigned: string[] = []
    const host = loginHost(fetchMock)
    ;(host as { location?: { assign: (url: string) => void } }).location = {
      assign: (url) => assigned.push(url)
    }
    wireStudioLogin(document, host as unknown as typeof globalThis)
    const phone = document.getElementById('phone') as HTMLInputElement
    phone.value = '13800138000'
    phone.dispatchEvent(new Event('input'))
    const send = document.getElementById('send-code') as HTMLButtonElement & { _send?: Promise<void> }
    expect(send.disabled).toBe(false)
    expect(calls.some((call) => call.url.endsWith('/captcha/challenges'))).toBe(false)
    expect(document.getElementById('captcha-dialog')?.hidden).toBe(true)
    send.click()
    expect(document.getElementById('captcha-dialog')?.hidden).toBe(false)
    await passCaptcha()
    expect(document.getElementById('captcha-bg')?.getAttribute('src')).toBe('/captcha/bg-n060.jpg')
    expect(document.getElementById('captcha-status')?.textContent).toBe('验证通过')
    expect(document.getElementById('captcha-dialog')?.hidden).toBe(true)
    await send._send
    expect(document.getElementById('toasts')?.textContent).toContain('开发验证码 654321')
    expect(send.textContent).toBe('60 秒后重发')
    const phoneCall = calls.find((call) => call.url.endsWith('/phone-codes'))
    expect(phoneCall?.url).toBe('http://127.0.0.1:9/v1/api/org/members/phone-codes')
    expect(phoneCall?.body).toBe('{"phone":"13800138000","captchaToken":"captcha_v1.test"}')
    expect(phoneCall?.authorization).toBe(`Bearer ${'ab'.repeat(32)}`)
    const code = document.getElementById('code') as HTMLInputElement
    code.value = '654321'
    const submit = document.getElementById('submit') as HTMLButtonElement & { _open?: Promise<void> }
    submit.click()
    await submit._open
    const loginCall = calls.find((call) => call.url.endsWith('/code-sessions'))
    expect(loginCall?.url).toBe('http://127.0.0.1:9/v1/api/org/members/code-sessions')
    expect(loginCall?.body).toContain('"code":"654321"')
    expect(JSON.parse(loginCall?.body ?? '{}')).toMatchObject({ phone: '13800138000', code: '654321' })
    expect(loginCall?.body).not.toContain('captchaToken')
    expect(assigned).toEqual([CHERRY_MAIN_URL])
    expect(document.getElementById('signed-in')?.hidden).toBe(true)
    expect(window.localStorage.getItem('a3s_org_member_token')).toBe('a3s_ms_session')
    expect(
      calls.some((call) => call.url.endsWith('/api/member-session') && call.body?.includes('"token":"a3s_ms_session"'))
    ).toBe(true)
    expect(document.querySelector('.wechat')?.hasAttribute('hidden')).toBe(false)
  })

  it('opens the main window from the saved member session without asking again', async () => {
    mountShell()
    const fetchMock = vi.fn(async (url: string) => {
      if (String(url).endsWith('/api/member-session')) {
        return jsonResponse({
          token: 'a3s_ms_saved.session',
          display_name: '林',
          phone: '13800138000'
        })
      }
      return jsonResponse({ code: 0, message: 'ok', data: {} })
    })
    const assigned: string[] = []
    const host = loginHost(fetchMock)
    ;(host as { location?: { assign: (url: string) => void } }).location = {
      assign: (url) => assigned.push(url)
    }
    wireStudioLogin(document, host as unknown as typeof globalThis)
    const login = document.getElementById('login') as HTMLElement & { _restore?: Promise<unknown> }
    expect(login.hidden).toBe(true)
    await login._restore
    expect(login.hidden).toBe(true)
    expect(assigned).toEqual([CHERRY_MAIN_URL])
    expect(window.localStorage.getItem('a3s_org_member_token')).toBe('a3s_ms_saved.session')
    expect(document.getElementById('submit')?.textContent).toBe('登录 / 注册')
  })

  it('shows the login card when the saved session check does not return', async () => {
    vi.useFakeTimers()
    try {
      mountShell()
      const fetchMock = vi.fn(() => new Promise(() => {}))
      const host = {
        ...loginHost(fetchMock),
        setTimeout,
        clearTimeout
      }
      wireStudioLogin(document, host as unknown as typeof globalThis)
      const login = document.getElementById('login') as HTMLElement
      expect(login.hidden).toBe(true)
      await vi.advanceTimersByTimeAsync(3999)
      expect(login.hidden).toBe(true)
      await vi.advanceTimersByTimeAsync(1)
      expect(login.hidden).toBe(false)
      expect(document.documentElement.style.visibility).toBe('visible')
    } finally {
      vi.useRealTimers()
    }
  })

  it('shows the login card only after this launch has no saved session', async () => {
    mountShell()
    const fetchMock = vi.fn(async () => jsonResponse({ token: null }))
    const assigned: string[] = []
    const host = loginHost(fetchMock)
    ;(host as { location?: { assign: (url: string) => void } }).location = {
      assign: (url) => assigned.push(url)
    }
    wireStudioLogin(document, host as unknown as typeof globalThis)
    const login = document.getElementById('login') as HTMLElement & { _restore?: Promise<unknown> }
    expect(login.hidden).toBe(true)
    await login._restore
    expect(login.hidden).toBe(false)
    expect(document.documentElement.dataset.shellReady).toBe('true')
    expect(document.documentElement.style.visibility).toBe('visible')
    expect(assigned).toEqual([])
  })

  it('shows the login card when this launch cleared the saved session', async () => {
    mountShell()
    window.localStorage.setItem('a3s_org_member_token', 'a3s_ms_saved.session')
    window.localStorage.setItem('a3s_org_member_profile', JSON.stringify({ display_name: '林', phone: '13800138000' }))
    const fetchMock = vi.fn(async () => jsonResponse({ token: null }))
    const assigned: string[] = []
    const host = loginHost(fetchMock)
    ;(host as { location?: { assign: (url: string) => void } }).location = {
      assign: (url) => assigned.push(url)
    }
    wireStudioLogin(document, host as unknown as typeof globalThis)
    const login = document.getElementById('login') as HTMLElement & { _restore?: Promise<unknown> }
    await login._restore
    expect(login.hidden).toBe(false)
    expect(assigned).toEqual([])
    expect(window.localStorage.getItem('a3s_org_member_token')).toBeNull()
    expect(window.localStorage.getItem('a3s_org_member_profile')).toBeNull()
  })

  it('keeps a bad password on the login card with the console error', async () => {
    mountShell()
    const fetchMock = vi.fn(async (url: string) => {
      const captcha = captchaEnvelope(String(url))
      if (captcha) return captcha
      return jsonResponse({ code: 40100, message: 'invalid credentials', data: null }, false)
    })
    const host = loginHost(fetchMock)
    wireStudioLogin(document, host as unknown as typeof globalThis)
    ;(document.querySelector('[data-mode="password"]') as HTMLButtonElement).click()
    const phone = document.getElementById('password-phone') as HTMLInputElement
    phone.value = '13800138000'
    const password = document.getElementById('password') as HTMLInputElement
    password.value = 'secret-secret'
    const submit = document.getElementById('submit') as HTMLButtonElement & { _open?: Promise<void> }
    submit.click()
    expect(document.getElementById('captcha-dialog')?.hidden).toBe(false)
    await passCaptcha()
    await submit._open
    expect(document.getElementById('form-error')?.textContent).toBe('邮箱或密码不正确')
    expect(document.getElementById('signed-in')?.hidden).toBe(true)
    expect(submit.disabled).toBe(false)
    expect(submit.textContent).toBe('登录')
    const loginCall = fetchMock.mock.calls.find((call) => String(call[0]).endsWith('/v1/api/org/members/sessions'))
    const body = JSON.parse(String(loginCall?.[1]?.body))
    expect(body).toEqual({
      identifier: '13800138000',
      password: 'secret-secret',
      captchaToken: 'captcha_v1.test'
    })
  })

  it('picks default models from a drawn listbox instead of an OS menu', () => {
    expect(html).not.toMatch(/<select|<datalist/)
    document.body.innerHTML = new DOMParser().parseFromString(html, 'text/html').body.innerHTML
    const picker = document.getElementById('model-default')!
    renderPicker(
      document,
      picker,
      [
        { value: 'openai\tgpt-4o', label: 'OpenAI / GPT-4o' },
        { value: 'deepseek\tchat', label: 'DeepSeek / Chat' }
      ],
      'openai\tgpt-4o'
    )
    const trigger = picker.querySelector<HTMLButtonElement>('.picker-trigger')!
    const listbox = picker.querySelector<HTMLElement>('[role="listbox"]')!
    expect(trigger.getAttribute('aria-haspopup')).toBe('listbox')
    expect(trigger.textContent).toBe('OpenAI / GPT-4o')
    expect(listbox.hidden).toBe(true)

    trigger.click()
    expect(trigger.getAttribute('aria-expanded')).toBe('true')
    expect(listbox.hidden).toBe(false)
    const options = [...listbox.querySelectorAll<HTMLButtonElement>('[role="option"]')]
    expect(options.map((option) => option.getAttribute('aria-selected'))).toEqual(['true', 'false'])
    expect(document.activeElement).toBe(options[0])
    listbox.dispatchEvent(new KeyboardEvent('keydown', { key: 'ArrowDown', bubbles: true }))
    expect(document.activeElement).toBe(options[1])

    options[1].click()
    expect(picker.dataset.value).toBe('deepseek\tchat')
    expect(trigger.textContent).toBe('DeepSeek / Chat')
    expect(listbox.hidden).toBe(true)
    expect(document.activeElement).toBe(trigger)

    trigger.click()
    listbox.dispatchEvent(new KeyboardEvent('keydown', { key: 'Escape', bubbles: true }))
    expect(listbox.hidden).toBe(true)
    trigger.click()
    document.body.dispatchEvent(new Event('pointerdown', { bubbles: true }))
    expect(listbox.hidden).toBe(true)
  })
})
