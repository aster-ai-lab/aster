// ToBot bridge for Telegram, Feishu, Discord, Slack, and QQ.
// Allow-list, ToBot binding, and reply extraction match
// src/renderer/pages/settings/ChannelsSettings/channelLink.ts.
// WeChat is stored by the settings page and is not connected here.

import { readFileSync } from 'node:fs'

import * as Lark from '@larksuiteoapi/node-sdk'
import { Bot } from 'grammy'

const TOBOT = 'tobot'
const QQ_INTENT = 1 << 25
const DISCORD_INTENTS = (1 << 0) | (1 << 9) | (1 << 12) | (1 << 15)

const state = { planeUrl: '', token: '' }
const live = new Map()
let tail = Promise.resolve()

function servesToBot(agentId) {
  if (agentId == null) return true
  const trimmed = String(agentId).trim()
  return trimmed.length === 0 || trimmed === TOBOT
}

function chatAllowed(allow, chatId) {
  if (!Array.isArray(allow) || allow.length === 0) return true
  return allow.some((item) => String(item) === String(chatId))
}

function channelPrompt(type, chatId, text) {
  return `[${type} ${chatId}] ${text}`
}

function textOfParts(parts) {
  if (!Array.isArray(parts)) return ''
  return parts
    .map((part) => (part && part.type === 'text' && typeof part.text === 'string' ? part.text : ''))
    .filter((line) => line.trim())
    .join('\n')
}

function replyAfterPrompt(items, prompt) {
  if (!Array.isArray(items)) return undefined
  const userIndex = items.findIndex((item) => item && item.role === 'user' && item.searchableText === prompt)
  if (userIndex < 0) return undefined
  for (let index = userIndex - 1; index >= 0; index -= 1) {
    const message = items[index]
    if (!message || typeof message !== 'object') continue
    if (message.role === 'user') return null
    if (message.role !== 'assistant') continue
    if (message.status === 'error' || message.status === 'failed') return null
    if (message.status !== 'success') return undefined
    const fromParts = textOfParts(message.data?.parts)
    const text = (fromParts || (typeof message.searchableText === 'string' ? message.searchableText : '')).trim()
    return text.length > 0 ? text : null
  }
  return undefined
}

function credential(config, key) {
  const value = config && typeof config[key] === 'string' ? config[key].trim() : ''
  return value
}

function ready(channel) {
  const config = channel.config || {}
  switch (channel.type) {
    case 'telegram':
    case 'discord':
      return credential(config, 'bot_token').length > 0
    case 'feishu':
      return credential(config, 'app_id').length > 0 && credential(config, 'app_secret').length > 0
    case 'slack':
      return credential(config, 'bot_token').length > 0 && credential(config, 'app_token').length > 0
    case 'qq':
      return credential(config, 'app_id').length > 0 && credential(config, 'client_secret').length > 0
    default:
      return false
  }
}

function signatureOf(channel) {
  return JSON.stringify({
    type: channel.type,
    isActive: channel.isActive === true,
    agentId: channel.agentId ?? '',
    config: channel.config || {}
  })
}

function sleep(ms) {
  return new Promise((resolve) => setTimeout(resolve, ms))
}

function enqueue(job) {
  const run = tail.then(job, job)
  tail = run.then(
    () => undefined,
    () => undefined
  )
  return run
}

function frameText(data) {
  if (typeof data === 'string') return data
  if (typeof Buffer !== 'undefined' && Buffer.isBuffer(data)) return data.toString('utf8')
  if (data instanceof ArrayBuffer) return Buffer.from(data).toString('utf8')
  return ''
}

async function plane(method, path, body) {
  if (!state.planeUrl || !state.token) return { ok: false, status: 0, json: null }
  const headers = { authorization: `Bearer ${state.token}` }
  if (body) headers['content-type'] = 'application/json'
  const response = await fetch(`${state.planeUrl}${path}`, {
    method,
    headers,
    body: body ? JSON.stringify(body) : undefined
  })
  let json = null
  try {
    json = await response.json()
  } catch {
    json = null
  }
  return { ok: response.ok, status: response.status, json }
}

async function askToBot(channel, chatId, text) {
  const prompt = channelPrompt(channel.type, chatId, text)
  const deadline = Date.now() + 90_000
  while (Date.now() < deadline) {
    try {
      await plane('POST', '/api/agent-sessions/tobot/ensure', {})
      const turn = await plane('POST', '/api/agent-sessions/tobot/turns', {
        userMessageParts: [{ type: 'text', text: prompt }]
      })
      if (turn.status === 409 || turn.status === 503 || turn.status === 0) {
        await sleep(500)
        continue
      }
      if (!turn.ok) {
        console.error(`studio channel tobot turn ${turn.status}`)
        return null
      }
      while (Date.now() < deadline) {
        const page = await plane('GET', '/api/agent-sessions/tobot/messages?limit=50')
        if (page.ok) {
          const verdict = replyAfterPrompt(page.json?.items, prompt)
          if (verdict !== undefined) return verdict
        }
        await sleep(500)
      }
      return null
    } catch {
      await sleep(500)
    }
  }
  return null
}

function deliver(channel, chatId, text, send) {
  enqueue(async () => {
    const reply = await askToBot(channel, chatId, text)
    if (!reply) return
    try {
      await send(reply)
    } catch {
      console.error(`studio channel ${channel.type} reply was not sent`)
    }
  }).catch(() => {})
}

function larkValue(name) {
  if (Lark[name]) return Lark[name]
  if (Lark.default && Lark.default[name]) return Lark.default[name]
  return undefined
}

function startTelegram(channel) {
  const token = credential(channel.config, 'bot_token')
  const bot = new Bot(token)
  let stopped = false
  bot.on('message:text', (ctx) => {
    if (stopped || ctx.from?.is_bot) return
    const chatId = ctx.chat?.id
    const text = ctx.message?.text?.trim() ?? ''
    if (chatId == null || !text || !chatAllowed(channel.config?.allowed_chat_ids, chatId)) return
    deliver(channel, chatId, text, (reply) => bot.api.sendMessage(chatId, reply.slice(0, 4000)))
  })
  bot.start().catch(() => {
    if (!stopped) console.error('studio channel telegram stopped')
  })
  return {
    alive: () => !stopped,
    stop() {
      stopped = true
      try {
        bot.stop()
      } catch {
        // The process is leaving this adapter.
      }
    }
  }
}

function startFeishu(channel) {
  const WSClient = larkValue('WSClient')
  const Client = larkValue('Client')
  const EventDispatcher = larkValue('EventDispatcher')
  const Domain = larkValue('Domain')
  const appId = credential(channel.config, 'app_id')
  const appSecret = credential(channel.config, 'app_secret')
  let stopped = false
  if (!WSClient || !Client || !EventDispatcher || !/^cli_[0-9a-fA-F]{16}$/.test(appId)) {
    console.error('studio channel feishu app id was not accepted')
    return {
      alive: () => !stopped,
      stop() {
        stopped = true
      }
    }
  }
  const domain = channel.config?.domain === 'lark' ? Domain.Lark : Domain.Feishu
  const loggerLevel = larkValue('LoggerLevel')?.error
  const client = new Client({ appId, appSecret, domain, loggerLevel })
  const ws = new WSClient({ appId, appSecret, domain, loggerLevel })
  const dispatcher = new EventDispatcher({}).register({
    'im.message.receive_v1': (data) => {
      if (stopped) return
      const message = data?.message
      const sender = data?.sender
      if (!message || sender?.sender_type === 'app') return
      if (message.message_type && message.message_type !== 'text') return
      let text = ''
      try {
        const parsed = JSON.parse(message.content || '{}')
        text = typeof parsed.text === 'string' ? parsed.text.trim() : ''
      } catch {
        text = ''
      }
      const chatId = message.chat_id ? String(message.chat_id) : ''
      if (!chatId || !text || !chatAllowed(channel.config?.allowed_chat_ids, chatId)) return
      deliver(channel, chatId, text, async (reply) => {
        await client.im.v1.message.create({
          params: { receive_id_type: 'chat_id' },
          data: {
            receive_id: chatId,
            msg_type: 'text',
            content: JSON.stringify({ text: reply.slice(0, 4000) })
          }
        })
      })
    }
  })
  ws.start({ eventDispatcher: dispatcher }).catch(() => {
    if (!stopped) console.error('studio channel feishu did not connect')
  })
  return {
    alive: () => !stopped,
    stop() {
      stopped = true
      try {
        ws.close({})
      } catch {
        // The socket is already gone.
      }
    }
  }
}

function startDiscord(channel) {
  const token = credential(channel.config, 'bot_token')
  let stopped = false
  let socket = null
  let heartbeat = null
  const connect = async () => {
    if (stopped) return
    let url = ''
    try {
      const gateway = await fetch('https://discord.com/api/v10/gateway/bot', {
        headers: { authorization: `Bot ${token}` }
      })
      if (!gateway.ok) {
        console.error(`studio channel discord gateway ${gateway.status}`)
        if (!stopped) setTimeout(connect, 5000)
        return
      }
      const body = await gateway.json()
      url = typeof body.url === 'string' ? body.url : ''
    } catch {
      if (!stopped) setTimeout(connect, 5000)
      return
    }
    if (!url || stopped) return
    const ws = new WebSocket(`${url}?v=10&encoding=json`)
    socket = ws
    let seq = null
    ws.addEventListener('message', (event) => {
      let packet
      try {
        packet = JSON.parse(frameText(event.data))
      } catch {
        return
      }
      if (typeof packet.s === 'number') seq = packet.s
      if (packet.op === 10) {
        const interval = Number(packet.d?.heartbeat_interval) || 41250
        if (heartbeat) clearInterval(heartbeat)
        heartbeat = setInterval(() => {
          if (ws.readyState === WebSocket.OPEN) ws.send(JSON.stringify({ op: 1, d: seq }))
        }, interval)
        ws.send(
          JSON.stringify({
            op: 2,
            d: {
              token,
              intents: DISCORD_INTENTS,
              properties: { os: process.platform, browser: 'tobot', device: 'tobot' }
            }
          })
        )
        return
      }
      if (packet.op !== 0 || packet.t !== 'MESSAGE_CREATE') return
      const message = packet.d || {}
      if (message.author?.bot) return
      const channelId = message.channel_id ? String(message.channel_id) : ''
      const text = typeof message.content === 'string' ? message.content.trim() : ''
      if (!channelId || !text || !chatAllowed(channel.config?.allowed_channel_ids, channelId)) return
      deliver(channel, channelId, text, async (reply) => {
        const sent = await fetch(`https://discord.com/api/v10/channels/${channelId}/messages`, {
          method: 'POST',
          headers: { authorization: `Bot ${token}`, 'content-type': 'application/json' },
          body: JSON.stringify({ content: reply.slice(0, 2000) })
        })
        if (!sent.ok) console.error(`studio channel discord reply ${sent.status}`)
      })
    })
    ws.addEventListener('close', () => {
      if (heartbeat) clearInterval(heartbeat)
      heartbeat = null
      if (!stopped) setTimeout(connect, 2000)
    })
  }
  connect().catch(() => {
    if (!stopped) console.error('studio channel discord did not connect')
  })
  return {
    alive: () => !stopped,
    stop() {
      stopped = true
      if (heartbeat) clearInterval(heartbeat)
      try {
        socket?.close()
      } catch {
        // The socket is already gone.
      }
    }
  }
}

function startSlack(channel) {
  const botToken = credential(channel.config, 'bot_token')
  const appToken = credential(channel.config, 'app_token')
  let stopped = false
  let socket = null
  const connect = async () => {
    if (stopped) return
    let url = ''
    try {
      const opened = await fetch('https://slack.com/api/apps.connections.open', {
        method: 'POST',
        headers: { authorization: `Bearer ${appToken}`, 'content-type': 'application/x-www-form-urlencoded' }
      })
      const body = await opened.json()
      if (!opened.ok || !body.ok || typeof body.url !== 'string') {
        console.error(`studio channel slack gateway ${opened.status}`)
        if (!stopped) setTimeout(connect, 5000)
        return
      }
      url = body.url
    } catch {
      if (!stopped) setTimeout(connect, 5000)
      return
    }
    if (stopped) return
    const ws = new WebSocket(url)
    socket = ws
    ws.addEventListener('message', (event) => {
      let packet
      try {
        packet = JSON.parse(frameText(event.data))
      } catch {
        return
      }
      if (packet.envelope_id && ws.readyState === WebSocket.OPEN) {
        ws.send(JSON.stringify({ envelope_id: packet.envelope_id }))
      }
      if (packet.type === 'disconnect') {
        try {
          ws.close()
        } catch {
          // The close handler reconnects.
        }
        return
      }
      const message = packet.payload?.event
      if (!message || message.type !== 'message') return
      if (message.bot_id || message.subtype === 'bot_message') return
      const channelId = message.channel ? String(message.channel) : ''
      const text = typeof message.text === 'string' ? message.text.trim() : ''
      if (!channelId || !text || !chatAllowed(channel.config?.allowed_channel_ids, channelId)) return
      deliver(channel, channelId, text, async (reply) => {
        const sent = await fetch('https://slack.com/api/chat.postMessage', {
          method: 'POST',
          headers: { authorization: `Bearer ${botToken}`, 'content-type': 'application/json' },
          body: JSON.stringify({ channel: channelId, text: reply.slice(0, 4000) })
        })
        if (!sent.ok) console.error(`studio channel slack reply ${sent.status}`)
      })
    })
    ws.addEventListener('close', () => {
      if (!stopped) setTimeout(connect, 2000)
    })
  }
  connect().catch(() => {
    if (!stopped) console.error('studio channel slack did not connect')
  })
  return {
    alive: () => !stopped,
    stop() {
      stopped = true
      try {
        socket?.close()
      } catch {
        // The socket is already gone.
      }
    }
  }
}

async function qqToken(appId, secret, current) {
  if (current && current.until > Date.now() + 60_000) return current.token
  const response = await fetch('https://bots.qq.com/app/getAppAccessToken', {
    method: 'POST',
    headers: { 'content-type': 'application/json' },
    body: JSON.stringify({ appId, clientSecret: secret })
  })
  if (!response.ok) throw new Error(String(response.status))
  const body = await response.json()
  if (typeof body.access_token !== 'string' || !body.access_token) throw new Error('missing')
  current.token = body.access_token
  current.until = Date.now() + (Number(body.expires_in) || 7200) * 1000
  return current.token
}

function startQQ(channel) {
  const appId = credential(channel.config, 'app_id')
  const secret = credential(channel.config, 'client_secret')
  const mentionOnly = channel.config?.mention_only !== false
  const seen = new Set()
  const auth = { token: '', until: 0 }
  let stopped = false
  let socket = null
  let heartbeat = null
  const firstTime = (id) => {
    if (!id || seen.has(id)) return false
    seen.add(id)
    if (seen.size > 500) seen.delete(seen.values().next().value)
    return true
  }
  const connect = async () => {
    if (stopped) return
    let url = ''
    let token = ''
    try {
      token = await qqToken(appId, secret, auth)
      let response = await fetch('https://api.sgroup.qq.com/gateway', {
        headers: { authorization: `QQBot ${token}` }
      })
      if (!response.ok) {
        response = await fetch('https://api.sgroup.qq.com/gateway/bot', {
          headers: { authorization: `QQBot ${token}` }
        })
      }
      const body = await response.json()
      if (!response.ok || typeof body.url !== 'string') throw new Error(String(response.status))
      url = body.url
    } catch {
      console.error('studio channel qq did not connect')
      if (!stopped) setTimeout(connect, 5000)
      return
    }
    if (stopped) return
    const ws = new WebSocket(url)
    socket = ws
    let seq = null
    ws.addEventListener('message', (event) => {
      let packet
      try {
        packet = JSON.parse(frameText(event.data))
      } catch {
        return
      }
      if (typeof packet.s === 'number') seq = packet.s
      if (packet.op === 10) {
        const interval = Number(packet.d?.heartbeat_interval) || 45000
        if (heartbeat) clearInterval(heartbeat)
        heartbeat = setInterval(() => {
          if (ws.readyState === WebSocket.OPEN) ws.send(JSON.stringify({ op: 1, d: seq }))
        }, interval)
        ws.send(
          JSON.stringify({
            op: 2,
            d: {
              token: `QQBot ${token}`,
              intents: QQ_INTENT,
              shard: [0, 1]
            }
          })
        )
        return
      }
      if (packet.op === 7 || packet.op === 9) {
        try {
          ws.close()
        } catch {
          // The close handler reconnects.
        }
        return
      }
      if (packet.op !== 0 || !packet.d) return
      const eventName = packet.t
      const groupEvent =
        eventName === 'GROUP_AT_MESSAGE_CREATE' || (eventName === 'GROUP_MESSAGE_CREATE' && !mentionOnly)
      const direct = eventName === 'C2C_MESSAGE_CREATE'
      if (!groupEvent && !direct) return
      const payload = packet.d
      const messageId = payload.id ? String(payload.id) : ''
      if (!firstTime(messageId)) return
      const text = typeof payload.content === 'string' ? payload.content.trim() : ''
      const openid = direct ? payload.author?.user_openid : payload.author?.member_openid
      const target = direct ? openid : payload.group_openid
      const chatId = target ? String(target) : ''
      if (!chatId || !text || !chatAllowed(channel.config?.allowed_chat_ids, chatId)) return
      deliver(channel, chatId, text, async (reply) => {
        const fresh = await qqToken(appId, secret, auth)
        const path = direct
          ? `https://api.sgroup.qq.com/v2/users/${encodeURIComponent(chatId)}/messages`
          : `https://api.sgroup.qq.com/v2/groups/${encodeURIComponent(chatId)}/messages`
        const sent = await fetch(path, {
          method: 'POST',
          headers: { authorization: `QQBot ${fresh}`, 'content-type': 'application/json' },
          body: JSON.stringify({ content: reply.slice(0, 4000), msg_type: 0, msg_id: messageId, msg_seq: 1 })
        })
        if (!sent.ok) console.error(`studio channel qq reply ${sent.status}`)
      })
    })
    ws.addEventListener('close', () => {
      if (heartbeat) clearInterval(heartbeat)
      heartbeat = null
      if (!stopped) setTimeout(connect, 2000)
    })
  }
  connect().catch(() => {
    if (!stopped) console.error('studio channel qq did not connect')
  })
  return {
    alive: () => !stopped,
    stop() {
      stopped = true
      if (heartbeat) clearInterval(heartbeat)
      try {
        socket?.close()
      } catch {
        // The socket is already gone.
      }
    }
  }
}

function startAdapter(channel) {
  switch (channel.type) {
    case 'telegram':
      return startTelegram(channel)
    case 'feishu':
      return startFeishu(channel)
    case 'discord':
      return startDiscord(channel)
    case 'slack':
      return startSlack(channel)
    case 'qq':
      return startQQ(channel)
    default:
      return null
  }
}

function stopAll() {
  for (const running of live.values()) {
    try {
      running.stop()
    } catch {
      // One adapter must not stop the others.
    }
  }
  live.clear()
}

function reconcile(channels) {
  const next = new Map()
  for (const channel of channels) {
    if (!channel || typeof channel.id !== 'string') continue
    if (channel.isActive !== true || !servesToBot(channel.agentId) || !ready(channel)) continue
    next.set(channel.id, { channel, signature: signatureOf(channel) })
  }
  for (const [id, running] of live) {
    const wanted = next.get(id)
    if (!wanted || wanted.signature !== running.signature || !running.alive()) {
      try {
        running.stop()
      } catch {
        // The replacement starts on this pass.
      }
      live.delete(id)
    }
  }
  for (const [id, wanted] of next) {
    if (live.has(id)) continue
    try {
      const running = startAdapter(wanted.channel)
      if (running) live.set(id, { signature: wanted.signature, ...running })
    } catch {
      console.error(`studio channel ${wanted.channel.type} did not start`)
    }
  }
}

async function tick(configPath) {
  if (process.ppid === 1) {
    stopAll()
    process.exit(0)
  }
  const raw = readFileSync(configPath, 'utf8')
  const config = JSON.parse(raw)
  state.planeUrl = String(config.planeUrl || '').replace(/\/$/, '')
  state.token = typeof config.token === 'string' ? config.token : ''
  reconcile(Array.isArray(config.channels) ? config.channels : [])
}

async function main() {
  const configPath = process.argv[2]
  if (!configPath) {
    console.error('studio channel gateway needs a config path')
    process.exit(1)
  }
  const run = async () => {
    try {
      await tick(configPath)
    } catch {
      console.error('studio channel gateway config was not read')
    }
  }
  await run()
  const timer = setInterval(run, 1500)
  timer.unref?.()
}

const invoked = typeof process.argv[1] === 'string' && process.argv[1].endsWith('channel-gateway.mjs')
if (invoked) {
  main().catch(() => {
    console.error('studio channel gateway stopped')
    process.exit(1)
  })
}

export { chatAllowed, replyAfterPrompt, servesToBot }
