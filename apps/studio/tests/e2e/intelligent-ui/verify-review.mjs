import { spawnSync } from 'node:child_process'
import { writeFile, mkdir } from 'node:fs/promises'
import { resolve } from 'node:path'
import { fileURLToPath } from 'node:url'

const studio = fileURLToPath(new URL('../../../', import.meta.url))
const session = `intelligent-ui-cert-${process.pid}`
const adapter = resolve(studio, 'tests/e2e/intelligent-ui/browser-adapter.sh')
const output = resolve(process.argv[2] ?? '/private/tmp/intelligent-ui-browser-proof')
await mkdir(output, { recursive: true })
const observations = []
let started = false

function command(args) {
  const result = spawnSync('a3s-test', args.concat('--json'), { cwd: studio, encoding: 'utf8', timeout: 45000 })
  if (result.status !== 0) throw new Error(result.stderr || result.stdout || 'A3S Test failed')
  return JSON.parse(result.stdout)
}
function observe() {
  const result = command(['agent', 'observe', '--session', session])
  const state = result.output.data.data
  const observed = { id: result.observation_id, snapshot: state.snapshot, refs: state.refs }
  observations.push(observed)
  return observed
}
async function until(predicate, name) {
  for (let attempt = 0; attempt < 20; attempt++) {
    const observed = observe()
    if (predicate(observed)) {
      process.stdout.write(`${name}\n`)
      return observed
    }
    await new Promise((resolve) => setTimeout(resolve, 100))
  }
  throw new Error(`Missing observable result: ${name}`)
}
function ref(observed, role, name) {
  const rows = Object.entries(observed.refs).filter(([, row]) => row.role === role && row.name === name)
  if (rows.length !== 1) throw new Error(`Ambiguous control: ${role} ${name}`)
  return `@${rows[0][0]}`
}
function act(type, observed, role, name, value) {
  const args = ['agent', type, ref(observed, role, name)]
  if (value !== undefined) args.push(value)
  command(args.concat('--session', session, '--observation', String(observed.id)))
}

try {
  command([
    'agent',
    'start',
    'http://127.0.0.1:4338',
    '--session',
    session,
    '--goal',
    'Certify real IntelligentUI streaming, local calculation, input preservation, conversation submission and same-turn form submission',
    '--success',
    'Edited budget survives appended source and completed fence',
    '--success',
    'Answers return through explicit channels; executable source stays text',
    '--browser-executable',
    adapter
  ])
  started = true
  const initial = await until((s) => /spinbutton "每天预算".*: 300/.test(s.snapshot), 'early input visible')
  act('fill', initial, 'spinbutton', '每天预算', '400')
  const edited = await until(
    (s) => /spinbutton "每天预算".*: 400/.test(s.snapshot) && s.snapshot.includes('2,000元'),
    'immediate calculation correct'
  )
  act('click', edited, 'button', 'Append content')
  const appended = await until(
    (s) => s.snapshot.includes('已追加内容') && /spinbutton "每天预算".*: 400/.test(s.snapshot),
    'input retained across streamed append'
  )
  act('click', appended, 'button', 'Finish response')
  const settled = await until(
    (s) => s.snapshot.includes('button "使用这个预算"') && /spinbutton "每天预算".*: 400/.test(s.snapshot),
    'input retained across fence completion'
  )
  await until(
    (s) => s.snapshot.includes('Form unavailable') && /button "使用这个预算" \[disabled/.test(s.snapshot),
    'fence form does not submit'
  )
  const shot = command(['agent', 'screenshot', 'screenshots/intelligent-ui-submitted.png', '--session', session])
  await writeFile(resolve(output, 'screenshot.json'), JSON.stringify(shot, null, 2))
  act('click', observe(), 'button', 'Pending form')
  const form = await until(
    (s) => s.snapshot.includes('textbox "Reason"') && s.snapshot.includes('checkbox "Agree"'),
    'pending OpenUI form visible'
  )
  act('click', form, 'button', 'Continue')
  const required = await until(
    (s) => s.snapshot.includes('Required fields are missing') && !s.snapshot.includes('same-turn'),
    'required fields block submission'
  )
  act('fill', required, 'textbox', 'Reason', 'Need a detailed plan')
  act('click', observe(), 'checkbox', 'Agree')
  act('click', observe(), 'button', 'Continue')
  await until(
    (s) =>
      s.snapshot.includes('same-turn') &&
      s.snapshot.includes('Need a detailed plan') &&
      /button "Sent" \[disabled/.test(s.snapshot),
    'pending form resolves through addressed same-turn channel'
  )
  const formShot = command(['agent', 'screenshot', 'screenshots/intelligent-ui-pending-form.png', '--session', session])
  await writeFile(resolve(output, 'pending-form-screenshot.json'), JSON.stringify(formShot, null, 2))
  act('click', observe(), 'button', 'Unsafe source')
  await until(
    (s) => s.snapshot.includes('alert(1)') && !s.snapshot.includes('spinbutton'),
    'executable source remains text'
  )
  const report = command([
    'agent',
    'finish',
    '--session',
    session,
    '--status',
    'passed',
    '--summary',
    'Production Intelligent UI message/form outcomes verified with controlled submission channels; AIX installation and native form ownership are separate campaigns.'
  ])
  started = false
  await writeFile(resolve(output, 'report.json'), JSON.stringify(report, null, 2))
  await writeFile(resolve(output, 'observations.json'), JSON.stringify(observations, null, 2))
} catch (error) {
  if (started)
    command(['agent', 'finish', '--session', session, '--status', 'failed', '--summary', String(error).slice(0, 2000)])
  await writeFile(resolve(output, 'observations.json'), JSON.stringify(observations, null, 2))
  throw error
}
