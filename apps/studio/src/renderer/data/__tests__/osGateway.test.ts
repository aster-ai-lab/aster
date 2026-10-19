import { describe, expect, it } from 'vitest'

import { osGatewayEndpoints, osGatewayOrigin, osGatewayProvider, protocolsToEndpointTypes } from '../osGateway'

describe('osGateway', () => {
  it('uses the shell origin when it is a bare http origin', () => {
    const host = { __A3S_GATEWAY_ORIGIN__: 'http://10.0.0.8:8080/' } as unknown as typeof globalThis
    expect(osGatewayOrigin(host)).toBe('http://10.0.0.8:8080')
    expect(osGatewayOrigin({} as typeof globalThis)).toBe('http://127.0.0.1:8080')
  })

  it('points each native adapter at the gateway and drops unknown protocols', () => {
    const endpoints = osGatewayEndpoints('http://127.0.0.1:8080')
    expect(endpoints?.['ollama-chat']?.baseUrl).toBe('http://127.0.0.1:8080')
    expect(endpoints?.['jina-rerank']?.adapterFamily).toBe('jina-rerank')
    expect(protocolsToEndpointTypes(['google', 'nope', 'jina'])).toEqual(['google-generate-content', 'jina-rerank'])
    const provider = osGatewayProvider('ToGateway', 'http://127.0.0.1:8080')
    expect(provider).toMatchObject({ id: 'a3s', name: 'ToGateway', isEnabled: true })
    expect(JSON.stringify(provider)).not.toContain('api.openai.com')
  })
})
