import { spawnSync } from 'node:child_process'
import { mkdir, writeFile } from 'node:fs/promises'
import { resolve } from 'node:path'
import { fileURLToPath } from 'node:url'

const studio = fileURLToPath(new URL('../../../', import.meta.url))
const session = `team-workbench-${process.pid}`
const output = resolve(process.argv[2] ?? '/private/tmp/a3s-team-browser-proof')
await mkdir(output, { recursive: true })
const observations = []
let started = false
function command(args) {
  const result = spawnSync(process.env.A3S_TEST_BIN ?? `${process.env.HOME}/.local/bin/a3s-test`, [...args, '--json'], {
    cwd: studio,
    encoding: 'utf8',
    timeout: 45000
  })
  if (result.status !== 0 && !(args.includes('finish') && args.includes('failed')))
    throw new Error(result.stderr || result.stdout)
  return JSON.parse(result.stdout)
}
function observe() {
  const result = command(['agent', 'observe', '--session', session])
  const state = result.output.data.data
  const observation = { id: result.observation_id, snapshot: state.snapshot, refs: state.refs }
  observations.push(observation)
  return observation
}
async function until(predicate, label) {
  for (let index = 0; index < 20; index++) {
    const state = observe()
    if (predicate(state)) return state
    await new Promise((resolve) => setTimeout(resolve, 100))
  }
  throw new Error(`Missing observable result: ${label}`)
}
function act(type, state, role, name, value) {
  const refs = Object.entries(state.refs).filter(([, row]) => row.role === role && row.name === name)
  if (refs.length !== 1) throw new Error(`Ambiguous control ${role} ${name}`)
  const args = ['agent', type, `@${refs[0][0]}`]
  if (value !== undefined) args.push(value)
  command([...args, '--session', session, '--observation', String(state.id)])
}
try {
  command([
    'agent',
    'start',
    'http://127.0.0.1:4388',
    '--session',
    session,
    '--goal',
    'Verify the real built-in form and task-center approval UI',
    '--success',
    'Submitted requests appear in sent tasks and approval updates the status',
    '--browser-executable',
    resolve(studio, 'tests/e2e/intelligent-ui/browser-adapter.sh')
  ])
  started = true
  let state = await until((s) => s.snapshot.includes('button "审批"'), 'built-in form')
  act('click', state, 'button', '审批')
  state = await until((s) => s.snapshot.includes('textbox "标题"'), 'approval fields')
  if (!/button "提交" \[disabled/.test(state.snapshot)) throw new Error('Empty form is submittable')
  act('fill', state, 'textbox', '标题', '设备采购')
  state = observe()
  act('fill', state, 'textbox', '说明', '采购一台显示器')
  state = observe()
  act('click', state, 'button', '提交')
  state = await until((s) => s.snapshot.includes('button "已发起 1"'), 'sent badge refresh')
  act('click', state, 'button', '已发起 1')
  state = await until((s) => s.snapshot.includes('button "设备采购 审批中'), 'sent request')
  const pending = Object.values(state.refs).find(
    (row) => row.role === 'button' && row.name.startsWith('设备采购 审批中')
  )
  act('click', state, 'button', pending.name)
  state = await until((s) => s.snapshot.includes('button "撤回"'), 'initiator authority')
  if (state.snapshot.includes('button "同意"')) throw new Error('Initiator can approve own request')
  act('click', state, 'button', '切换为审批人（测试）')
  state = await until((s) => s.snapshot.includes('button "同意"'), 'approver authority')
  act('click', state, 'button', '同意')
  state = await until(
    (s) => /dialog "同意后[\s\S]*button "同意" \[ref=e\d+\]/.test(s.snapshot),
    'decision confirmation controls'
  )
  const dialog = state.snapshot.slice(state.snapshot.indexOf('dialog "同意后'))
  const confirm = dialog.match(/button "同意" \[ref=(e\d+)\]/)
  if (!confirm) throw new Error('Missing dialog confirmation')
  command(['agent', 'click', `@${confirm[1]}`, '--session', session, '--observation', String(state.id)])
  state = await until((s) => s.snapshot.includes('button "设备采购 已通过'), 'approved request refresh')
  if (state.snapshot.includes('invalid work item')) throw new Error('Invalid response leaked into the UI')
  const screenshot = command(['agent', 'screenshot', 'screenshots/team-workbench-approved.png', '--session', session])
  const report = command([
    'agent',
    'finish',
    '--session',
    session,
    '--status',
    'passed',
    '--summary',
    'The real built-in approval form creates a team request, refreshes sent tasks, respects initiator/approver actions and shows the approved status'
  ])
  started = false
  await writeFile(resolve(output, 'observations.json'), `${JSON.stringify(observations, null, 2)}\n`)
  await writeFile(resolve(output, 'report.json'), `${JSON.stringify({ report, screenshot }, null, 2)}\n`)
  process.stdout.write(`${JSON.stringify({ status: 'passed', session, output, screenshot })}\n`)
} catch (error) {
  if (started)
    command(['agent', 'finish', '--session', session, '--status', 'failed', '--summary', String(error).slice(0, 2000)])
  await writeFile(resolve(output, 'observations.json'), `${JSON.stringify(observations, null, 2)}\n`)
  throw error
}
