import { fireEvent, render, screen } from '@testing-library/react'
import i18n from 'i18next'
import { I18nextProvider, initReactI18next } from 'react-i18next'
import { beforeAll, describe, expect, it } from 'vitest'

import zhCN from '@renderer/i18n/locales/zh-cn.json'
import { EvolutionSettings } from '@renderer/pages/settings/EvolutionSettings/EvolutionSettings'
import { Route } from '@renderer/routes/settings/evolution'

const catalog = zhCN as Record<string, string>
const STOCK_LABELS = ['system', 'tools', 'budget', 'compact', 'infer'] as const
const i18nInstance = i18n.createInstance()

beforeAll(async () => {
  await i18nInstance.use(initReactI18next).init({
    lng: 'zh-CN',
    fallbackLng: 'zh-CN',
    resources: { 'zh-CN': { translation: zhCN } },
    keySeparator: false,
    interpolation: { escapeValue: false },
    react: { useSuspense: false },
    initImmediate: false
  })
})

function renderEvolutionRoute() {
  expect(Route.options.component).toBeTypeOf('function')
  return render(
    <I18nextProvider i18n={i18nInstance}>
      <div style={{ width: 960, height: 720 }}>
        <EvolutionSettings />
      </div>
    </I18nextProvider>
  )
}

function node(id: string): HTMLElement | null {
  return document.querySelector<HTMLElement>(`[data-fig-node="${id}"]`)
}

function pill(id: string): HTMLElement | null {
  return document.querySelector<HTMLElement>(`[data-fig-pill="${id}"]`)
}

function pickVersion(ui: string, id: string) {
  const select = document.querySelector(`[data-ui="${ui}"] [data-testid="combobox"]`)
  if (!(select instanceof HTMLSelectElement)) throw new Error(`missing combobox ${ui}`)
  fireEvent.change(select, { target: { value: id } })
}

describe('EvolutionSettings', () => {
  it('shows the Meta Harness as a stepped architecture figure', () => {
    window.localStorage.removeItem('a3s.studio.harness-assemble.v1')
    window.localStorage.removeItem('a3s.studio.harness-evolution.v1')
    renderEvolutionRoute()
    expect(document.querySelector('[data-ui="settings.evolution.versions"] [data-testid="combobox"]')).toBeTruthy()
    expect(document.querySelector('[data-ui="settings.evolution.versions"]')?.getAttribute('data-assemble')).toBe(
      'default'
    )

    expect(document.querySelector('[data-ui="settings.evolution"]')).toBeTruthy()
    expect(document.querySelector('[data-ui="settings.evolution.figure"]')).toBeTruthy()
    for (const label of STOCK_LABELS) {
      expect(document.body.textContent).toContain(label)
    }
    expect(node('permission-projection')?.textContent).toContain('permission projection')
    expect(node('completion-gate')?.textContent).toContain('completion gate')
    expect(node('tool-execution')?.textContent).toContain('tool execution')
    expect(node('compact-choice')?.textContent).toContain(catalog['settings.evolution.rules.compactChoice'])
    expect(node('compact-choice')?.textContent).not.toContain('input.approved')
    expect(pill('compact-choice__summarize')?.textContent).toContain(catalog['settings.evolution.edges.compactNow'])
    expect(pill('compact-choice__infer')?.textContent).toContain(catalog['settings.evolution.edges.directInfer'])
    expect(pill('text-choice__assistant-text')?.textContent).toContain(catalog['settings.evolution.edges.text'])
    expect(pill('question-choice__budget-choice')?.textContent).toContain(catalog['settings.evolution.edges.toolCall'])
    expect(pill('permission-projection__tool-execution')?.textContent).toContain(
      catalog['settings.evolution.edges.beforeTool']
    )
    expect(pill('mutating-choice__completion-gate')?.textContent).toContain(
      catalog['settings.evolution.edges.mutatingSuccess']
    )
    expect(pill('completion-gate__incomplete')?.textContent).toContain(catalog['settings.evolution.edges.gateOpen'])
    expect(pill('budget-choice__permission-projection')?.textContent).toContain(
      catalog['settings.evolution.edges.budgetLeft']
    )
    expect(pill('deny-queue__turn-done')?.textContent).toContain(catalog['settings.evolution.edges.denyEnd'])
    expect(document.querySelector('[data-ui="settings.evolution.summary"]')?.textContent).toBe(
      catalog['settings.evolution.summary']
    )
    const scenario = document.querySelector('[data-ui="settings.evolution.scenario"]')
    expect(scenario).toBeInstanceOf(HTMLTextAreaElement)
    expect((scenario as HTMLTextAreaElement).value).toContain('完成一次代码交付')
    expect(document.querySelector('[data-harness-phase]')).toBeNull()
    expect(document.querySelector('.harness-figure__caption')).toBeTruthy()
    expect(document.querySelector('.harness-figure__tabs .harness-figure__tab-split')).toBeTruthy()
    expect(screen.getByRole('tab', { name: '工具' })).toBeTruthy()
    expect(
      screen.queryByRole('button', { name: '播放' }) ?? screen.queryByRole('button', { name: '暂停' })
    ).toBeTruthy()
    expect(screen.queryByRole('button', { name: '保存' })).toBeNull()
    expect(screen.queryByRole('button', { name: '校验' })).toBeNull()
    expect(screen.queryByRole('button', { name: '试运行' })).toBeNull()
    expect(document.querySelector('.a3s-workflow-header')).toBeNull()
    expect(document.querySelector('.a3s-workflow-rail')).toBeNull()
    expect(
      Object.keys(window.localStorage).some(
        (key) => key.startsWith('a3s-flow-designer') || key.startsWith('a3s-flow-playground')
      )
    ).toBe(false)

    fireEvent.click(screen.getByRole('tab', { name: '工具' }))
    expect(document.querySelector('[data-ui="settings.evolution.figure"]')?.getAttribute('data-story')).toBe('tool')
    expect(
      document.querySelector('[data-fig-edge="question-choice__budget-choice"]')?.classList.contains('is-live')
    ).toBe(true)

    fireEvent.click(node('permission-projection') as Element)
    const role = document.querySelector('[data-ui="settings.evolution.role"]')
    expect(role?.textContent).toContain(catalog['settings.evolution.phases.kernel'])
    expect(role?.textContent).toContain('permission projection')
    expect(role?.textContent).toContain(catalog['settings.evolution.inspector.label'])
    expect(role?.textContent).toContain(catalog['settings.evolution.roles.permissionProjection'])
    const detail = document.querySelector('[data-ui="settings.evolution.detail"]')
    expect(detail?.textContent).toContain(catalog['settings.evolution.detail.kernel'])
    expect(detail?.textContent).toContain(catalog['settings.evolution.detail.order'])
    expect(detail?.textContent).toContain('system')

    fireEvent.mouseEnter(node('system') as Element)
    expect(document.querySelector('.harness-figure__peek')?.textContent).toContain(
      catalog['settings.evolution.detail.peek']
    )
    expect(document.querySelector('.harness-figure__transport button svg')).toBeTruthy()
    expect(document.querySelector('[role="tab"] svg')).toBeTruthy()

    pickVersion('settings.evolution.versions', 'lean')
    expect(document.querySelector('[data-ui="settings.evolution.figure"]')?.getAttribute('data-snapshot')).toBe('lean')
    expect(document.querySelector('[data-ui="settings.evolution.versions"]')?.getAttribute('data-assemble')).toBe(
      'lean'
    )
    expect(node('compact')?.getAttribute('data-mounted')).toBe('false')
    expect(node('tools')?.getAttribute('data-mounted')).toBe('true')
    expect(node('intent-stamp')).toBeNull()
    expect(document.querySelector('[data-ui="settings.evolution.version-note"]')?.textContent).toContain('去掉 compact')

    fireEvent.click(node('tools') as Element)
    const leanTools = document.querySelector('[data-ui="settings.evolution.detail"]')
    expect(leanTools?.querySelector('[data-tool="bash"]')?.getAttribute('data-included')).toBe('false')
    expect(leanTools?.querySelector('[data-tool="read"]')?.getAttribute('data-included')).toBe('true')
    expect(leanTools?.querySelector('[data-tool="write"]')?.getAttribute('data-included')).toBe('false')
    expect(leanTools?.textContent).toContain(catalog['settings.evolution.detail.emptySkills'])

    fireEvent.click(screen.getByRole('button', { name: 'Diff' }))
    const diff = document.querySelector('[data-ui="settings.evolution.diff.panel"]')
    expect(diff?.textContent).toContain(catalog['settings.evolution.diff.title'])
    expect(diff?.querySelector('[data-component="compact"]')?.getAttribute('data-diff')).toBe('removed')
    expect(diff?.querySelector('[data-component="tools"]')?.getAttribute('data-diff')).toBe('same')
    expect(diff?.querySelector('[data-asset="tool:bash"]')?.getAttribute('data-diff')).toBe('removed')
    expect(diff?.querySelector('[data-asset="tool:read"]')).toBeNull()
    expect(diff?.textContent).toContain('permission_overlay')

    pickVersion('settings.evolution.versions', 'host')
    expect(diff?.querySelector('[data-component="host:intent_stamp"]')?.getAttribute('data-diff')).toBe('added')
    expect(node('intent-stamp')?.textContent).toContain('host:intent_stamp')
    expect(node('compact')?.getAttribute('data-mounted')).toBe('true')
    fireEvent.click(node('intent-stamp') as Element)
    const hostDetail = document.querySelector('[data-ui="settings.evolution.detail"]')
    expect(hostDetail?.textContent).toContain(catalog['settings.evolution.detail.host'])
    expect(hostDetail?.querySelector('[data-prompt]')?.textContent).toBe('a3s.meta_harness.intent_stamp.v1')

    fireEvent.click(node('system') as Element)
    const systemDetail = document.querySelector('[data-ui="settings.evolution.detail"]')
    expect(systemDetail?.textContent).toContain(catalog['settings.evolution.detail.internals'])
    expect(systemDetail?.textContent).toContain(catalog['settings.evolution.detail.promptHost'])
    expect(systemDetail?.querySelector('[data-snapshot="host"][data-current="true"] [data-prompt]')?.textContent).toBe(
      'a3s.meta_harness.intent_stamp.v1'
    )
    expect(systemDetail?.querySelector('[data-snapshot="lean"]')?.getAttribute('data-admits')).toBe('false')

    fireEvent.click(node('tools') as Element)
    const toolsDetail = document.querySelector('[data-ui="settings.evolution.detail"]')
    expect(toolsDetail?.querySelector('[data-tool="bash"] [data-param="command"]')?.getAttribute('data-required')).toBe(
      'true'
    )
    expect(toolsDetail?.querySelector('[data-tool="Skill"] [data-param="skill_name"]')?.textContent).toContain(
      'skill_name'
    )
    expect(toolsDetail?.querySelector('[data-skill="intent-review"] [data-skill-input="content"]')).toBeTruthy()
    expect(toolsDetail?.textContent).not.toContain(catalog['settings.evolution.detail.emptySkills'])
    expect(toolsDetail?.querySelector('[data-skill-field="content"]')).toBeTruthy()
    expect(toolsDetail?.querySelector('[data-skill-field="allowed-tools"]')).toBeTruthy()
    fireEvent.change(toolsDetail?.querySelector('[data-tool="read"] textarea') as Element, {
      target: { value: '只读一段文本。' }
    })
    expect(
      document
        .querySelector('[data-ui="settings.evolution.diff.panel"] [data-asset="tool:read"]')
        ?.getAttribute('data-diff')
    ).toBe('optimized')

    pickVersion('settings.evolution.versions', 'lean')
    fireEvent.click(node('compact') as Element)
    const compactDetail = document.querySelector('[data-ui="settings.evolution.detail"]')
    expect(compactDetail?.textContent).toContain(catalog['settings.evolution.detail.compactAbsent'])
    expect(
      compactDetail?.querySelector('[data-ui="settings.evolution.detail.prompt"]')?.getAttribute('data-admitted')
    ).toBe('engine')
    expect(compactDetail?.textContent).toContain('You are a context-compaction engine.')
  })
})
