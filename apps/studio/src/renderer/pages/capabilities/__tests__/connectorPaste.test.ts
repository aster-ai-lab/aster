import { describe, expect, it } from 'vitest'

import { formatConnectorPaste, parseConnectorPaste } from '../connectorPaste'
import { customConnectorBody } from '../customConnector'

const stdio = {
  name: 'local-fs',
  transport: 'stdio' as const,
  command: 'npx',
  args: ['-y', 'filesystem'],
  values: { API_KEY: 'secret' }
}

describe('parseConnectorPaste', () => {
  it('reads Claude, Cursor, and VS Code documents, including JSONC', () => {
    const claude = `{
      // kept on this machine
      "preferences": { "ignored": true },
      "mcpServers": {
        "Local-FS": {
          "command": "npx",
          "args": ["-y", "filesystem",],
          "env": { "API_KEY": "secret", "EMPTY": "" },
        },
      },
    }`
    expect(parseConnectorPaste(claude)).toEqual({ ok: true, connectors: [stdio] })

    const vscode = `{
      "servers": {
        "docs": { "type": "sse", "url": "https://example.com/sse", "headers": { "Authorization": "Bearer x" } }
      }
    }`
    expect(parseConnectorPaste(vscode)).toEqual({
      ok: true,
      connectors: [
        {
          name: 'docs',
          transport: 'sse',
          url: 'https://example.com/sse',
          values: { Authorization: 'Bearer x' }
        }
      ]
    })

    const settings = `{
      "editor.fontSize": 14,
      "mcp": { "servers": { "search": { "type": "http", "url": "https://example.com/mcp" } } }
    }`
    expect(parseConnectorPaste(settings)).toEqual({
      ok: true,
      connectors: [{ name: 'search', transport: 'streamable_http', url: 'https://example.com/mcp', values: {} }]
    })
  })

  it('prefers mcpServers, maps transport aliases, and keeps a bare spec on the current name', () => {
    expect(
      parseConnectorPaste(`{
        "servers": { "from-servers": { "command": "node" } },
        "mcpServers": { "from-mcp": { "command": "npx" } }
      }`)
    ).toMatchObject({ ok: true, connectors: [{ name: 'from-mcp', command: 'npx' }] })

    for (const type of ['stdio', 'local', 'standard_io']) {
      expect(parseConnectorPaste(`{ "name": "local-fs", "type": "${type}", "command": "npx" }`)).toMatchObject({
        ok: true,
        connectors: [{ name: 'local-fs', transport: 'stdio', command: 'npx' }]
      })
    }
    for (const type of ['http', 'streamable_http', 'streamable-http', 'streamableHttp']) {
      expect(
        parseConnectorPaste(`{ "name": "search", "type": "${type}", "url": "https://example.com/mcp" }`)
      ).toMatchObject({
        ok: true,
        connectors: [{ transport: 'streamable_http' }]
      })
    }
    expect(parseConnectorPaste('{ "url": "https://example.com/mcp" }', { name: 'search' })).toMatchObject({
      ok: true,
      connectors: [{ name: 'search', transport: 'streamable_http' }]
    })
    expect(parseConnectorPaste('{ "command": ["npx", "-y"], "args": ["filesystem"] }', { name: 'Local-FS' })).toEqual({
      ok: true,
      connectors: [{ name: 'local-fs', transport: 'stdio', command: 'npx', args: ['-y', 'filesystem'], values: {} }]
    })
    expect(parseConnectorPaste('{ "name": "other", "command": "node" }', { name: 'local-fs' })).toMatchObject({
      ok: true,
      connectors: [{ name: 'other', command: 'node' }]
    })
  })

  it('does not split arguments on spaces, expand placeholders, or copy the unused secret map', () => {
    expect(parseConnectorPaste('{ "name": "local-fs", "command": "npx", "args": "-y filesystem" }')).toMatchObject({
      ok: true,
      connectors: [{ args: ['-y filesystem'] }]
    })
    expect(parseConnectorPaste('{ "name": "local-fs", "command": "npx", "args": "-y\\nfilesystem" }')).toMatchObject({
      ok: true,
      connectors: [{ args: ['-y', 'filesystem'] }]
    })
    expect(
      parseConnectorPaste(`{
        "mcpServers": {
          "local-fs": {
            "command": "npx",
            "env": { "API_KEY": "\${env:TOKEN}", "NOTE": "https://example.com/a,b" },
            "headers": { "Authorization": "nope" }
          }
        }
      }`)
    ).toMatchObject({
      ok: true,
      connectors: [{ values: { API_KEY: '${env:TOKEN}', NOTE: 'https://example.com/a,b' } }]
    })
    expect(
      parseConnectorPaste(`{
        "name": "docs",
        "type": "sse",
        "url": "https://example.com/sse",
        "env": { "TOKEN": "secret" },
        "headers": { "Authorization": "Bearer x" }
      }`)
    ).toMatchObject({ ok: true, connectors: [{ values: { Authorization: 'Bearer x' } }] })
  })

  it('refuses an unreadable document, an empty document, and any entry the form would refuse', () => {
    expect(parseConnectorPaste('')).toEqual({ ok: false, code: 'json_empty' })
    expect(parseConnectorPaste('  \n // nothing')).toEqual({ ok: false, code: 'json_empty' })
    expect(parseConnectorPaste('{ "mcpServers": {} }')).toEqual({ ok: false, code: 'json_empty' })
    expect(parseConnectorPaste('{')).toEqual({ ok: false, code: 'invalid_json' })
    expect(parseConnectorPaste('{ "command": "npx" }')).toEqual({ ok: false, code: 'invalid_name' })
    expect(parseConnectorPaste('{ "name": "My Server", "command": "npx" }')).toEqual({
      ok: false,
      code: 'invalid_name'
    })
    expect(
      parseConnectorPaste('{ "mcpServers": { "GitHub": { "command": "npx" }, "github": { "command": "node" } } }')
    ).toEqual({
      ok: false,
      code: 'invalid_config'
    })
    expect(parseConnectorPaste('{ "name": "local-fs", "type": "websocket", "command": "npx" }')).toEqual({
      ok: false,
      code: 'unknown_transport'
    })
    expect(parseConnectorPaste('{ "name": "docs", "type": "sse", "url": "javascript:alert(1)" }')).toEqual({
      ok: false,
      code: 'invalid_url'
    })
    expect(parseConnectorPaste('{ "name": "docs", "type": "http" }')).toEqual({ ok: false, code: 'missing_url' })
    expect(parseConnectorPaste('{ "name": "local-fs", "args": ["-y"] }')).toEqual({
      ok: false,
      code: 'unknown_transport'
    })
    expect(parseConnectorPaste('{ "name": "local-fs", "command": " " }')).toEqual({
      ok: false,
      code: 'missing_command'
    })
    expect(parseConnectorPaste('{ "name": "local-fs", "command": "npx", "env": { "PORT": 3000 } }')).toEqual({
      ok: false,
      code: 'invalid_config'
    })
    expect(parseConnectorPaste('{ "name": "local-fs", "command": "npx", "env": { "TOKEN": null } }')).toEqual({
      ok: false,
      code: 'invalid_config'
    })
    expect(parseConnectorPaste('{ "mcpServers": [] }')).toEqual({ ok: false, code: 'invalid_config' })

    const many = Object.fromEntries(Array.from({ length: 33 }, (_, index) => [`s${index}`, { command: 'npx' }]))
    expect(parseConnectorPaste(JSON.stringify({ mcpServers: many }))).toEqual({ ok: false, code: 'invalid_config' })

    const mixed = '{ "mcpServers": { "ok": { "command": "npx" }, "bad": { "type": "socket" } } }'
    expect(parseConnectorPaste(mixed)).toEqual({ ok: false, code: 'unknown_transport' })
  })

  it('round-trips a valid draft through mcp.json', () => {
    const body = customConnectorBody({
      name: stdio.name,
      transport: 'stdio',
      command: stdio.command,
      args: stdio.args.join('\n'),
      values: Object.entries(stdio.values).map(([key, value]) => ({ key, value }))
    })
    expect(body).not.toBeNull()
    expect(parseConnectorPaste(formatConnectorPaste(body!))).toEqual({ ok: true, connectors: [body] })

    const remote = customConnectorBody({
      name: 'docs',
      transport: 'sse',
      url: 'https://example.com/sse',
      values: [{ key: 'Authorization', value: 'Bearer x' }]
    })
    expect(parseConnectorPaste(formatConnectorPaste(remote!))).toEqual({ ok: true, connectors: [remote] })
  })
})
