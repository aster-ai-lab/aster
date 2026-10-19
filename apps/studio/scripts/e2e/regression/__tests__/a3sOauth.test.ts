import { describe, expect, it, vi } from 'vitest'

import { completeA3sOauth } from '../a3sOauth'

const STATE = 'state-31415'
const AUTHORIZATION_URL = `http://127.0.0.1:8080/oauth2/auth?state=${STATE}&client_id=cherry-studio`
const LOGIN_URL = 'http://127.0.0.1:8080/register?login_challenge=login-27182'
const CONSENT_URL = 'http://127.0.0.1:8080/oauth/consent?consent_challenge=consent-16180'
const LOGIN_CONTINUATION_URL = 'http://127.0.0.1:8080/oauth2/auth?login_verifier=login-verifier-14142'
const CONSENT_CONTINUATION_URL = 'http://127.0.0.1:8080/oauth2/auth?consent_verifier=consent-verifier-17320'

function jsonResponse(data: Record<string, unknown>): Response {
  return Response.json({ data, success: true })
}

function createOauthFetch(
  options: {
    authorizationRedirect?: string
    callbackUrl?: string
    callbackState?: string
    loginData?: Record<string, unknown>
  } = {}
) {
  return vi.fn(async (input: string | URL, init: RequestInit = {}): Promise<Response> => {
    const url = new URL(input.toString())
    const method = init.method ?? 'GET'
    if (url.pathname === '/oauth2/auth') {
      if (url.searchParams.has('login_verifier')) {
        return new Response(null, { headers: { location: CONSENT_URL }, status: 302 })
      }
      if (url.searchParams.has('consent_verifier')) {
        return new Response(null, {
          headers: {
            location:
              options.callbackUrl ??
              `http://127.0.0.1:29873/oauth/callback?code=code-14142&state=${options.callbackState ?? STATE}`
          },
          status: 302
        })
      }
      return new Response(null, {
        headers: {
          location: options.authorizationRedirect ?? LOGIN_URL,
          'set-cookie': 'oauth_session=session-31415; Path=/; HttpOnly; Secure'
        },
        status: 302
      })
    }
    if (['/oauth/login', '/register'].includes(url.pathname)) return new Response('<main>Login</main>')
    if (url.pathname === '/api/oauth/login' && method === 'GET') return jsonResponse({ client_name: 'Cherry Studio' })
    if (url.pathname === '/api/oauth/login' && method === 'POST') {
      return jsonResponse(options.loginData ?? { redirect_to: LOGIN_CONTINUATION_URL })
    }
    if (url.pathname === '/oauth/consent') return new Response('<main>Consent</main>')
    if (url.pathname === '/api/oauth/consent' && method === 'GET') {
      return jsonResponse({ requested_scope: ['openid', 'tokens:read'] })
    }
    if (url.pathname === '/api/oauth/consent' && method === 'POST') {
      return jsonResponse({ redirect_to: CONSENT_CONTINUATION_URL })
    }
    throw new Error('Unexpected OAuth request')
  })
}

describe('A3S OAuth automation', () => {
  it('completes the application flow while preserving the server session cookie', async () => {
    const fetchImplementation = createOauthFetch()

    const callback = await completeA3sOauth(
      AUTHORIZATION_URL,
      { account: 'automation@example.test', password: 'account-secret' },
      { fetchImplementation }
    )

    expect(callback).toBe(`http://127.0.0.1:29873/oauth/callback?code=code-14142&state=${STATE}`)
    const loginRequest = fetchImplementation.mock.calls.find(([input, init]) => {
      const url = new URL(input.toString())
      return url.pathname === '/api/oauth/login' && init?.method === 'POST'
    })
    expect(loginRequest).toBeDefined()
    expect(new Headers(loginRequest?.[1]?.headers).get('cookie')).toBe('oauth_session=session-31415')
    expect(JSON.parse(String(loginRequest?.[1]?.body))).toEqual({
      login_challenge: 'login-27182',
      password: 'account-secret',
      username: 'automation@example.test'
    })
  })

  it('rejects a login that requires an unavailable second factor', async () => {
    await expect(
      completeA3sOauth(
        AUTHORIZATION_URL,
        { account: 'automation@example.test', password: 'account-secret' },
        { fetchImplementation: createOauthFetch({ loginData: { require_2fa: true } }) }
      )
    ).rejects.toThrow('A3S regression account requires 2FA')
  })

  it('rejects redirects outside A3S before forwarding credentials', async () => {
    const fetchImplementation = createOauthFetch({
      authorizationRedirect: 'https://untrusted.example.test/oauth/login?login_challenge=stolen'
    })

    await expect(
      completeA3sOauth(
        AUTHORIZATION_URL,
        { account: 'automation@example.test', password: 'account-secret' },
        { fetchImplementation }
      )
    ).rejects.toThrow('A3S OAuth authorization redirect was rejected')
    expect(fetchImplementation).toHaveBeenCalledOnce()
  })

  it('rejects callbacks whose state differs from the application request', async () => {
    await expect(
      completeA3sOauth(
        AUTHORIZATION_URL,
        { account: 'automation@example.test', password: 'account-secret' },
        { fetchImplementation: createOauthFetch({ callbackState: 'unexpected-state' }) }
      )
    ).rejects.toThrow('A3S OAuth callback state did not match the application request')
  })
  it.each([
    'cherrystudio://oauth/callback',
    'http://127.0.0.1:29874/oauth/callback',
    'http://localhost:29873/oauth/callback',
    'http://127.0.0.1:29873/other',
    'http://user:password@127.0.0.1:29873/oauth/callback',
    'https://untrusted.example.test/oauth/callback'
  ])('rejects an unexpected callback destination: %s', async (destination) => {
    await expect(
      completeA3sOauth(
        AUTHORIZATION_URL,
        { account: 'automation@example.test', password: 'account-secret' },
        { fetchImplementation: createOauthFetch({ callbackUrl: `${destination}?code=code&state=${STATE}` }) }
      )
    ).rejects.toThrow('invalid application callback')
  })
})
