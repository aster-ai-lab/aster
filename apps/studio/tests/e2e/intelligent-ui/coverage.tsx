import i18next from 'i18next'
import { useState } from 'react'
import { createRoot } from 'react-dom/client'
import { initReactI18next } from 'react-i18next'

import { IntelligentUiSubmitContext } from '../../../src/renderer/components/chat/messages/markdown/intelligentUiHost'
import { OpenUiMessage } from '../../../src/renderer/components/chat/messages/markdown/OpenUiMessage'
import { splitOpenUiFences } from '../../../src/renderer/components/chat/messages/markdown/splitOpenUiFences'

await i18next.use(initReactI18next).init({
  lng: 'zh-CN',
  resources: {
    'zh-CN': {
      translation: {
        'agent.form.invalid': '界面无效',
        'agent.form.required': '还有必填项',
        'agent.form.unavailable': '暂时不能提交',
        'agent.form.submitted': '已提交',
        'agent.form.busy': '正在提交',
        'agent.form.sendFailed': '提交失败',
        'common.loading': '正在加载'
      }
    }
  }
})

const leaveView = [
  'n0 = Text("cm_content: 去医院")',
  'n1 = Text("cm_type: 病假")',
  'n2 = Metric("days", 2, "")',
  'root = Card("leave_card", [n0, n1, n2])'
].join('\n')

const leaveForm = [
  'n0 = Input("n0", "cm_content", "去医院", "text")',
  'n1 = Input("n1", "cm_type", "病假", "text")',
  'n2 = Input("n2", "days", 2, "number")',
  'submit = Button("submit", "提交")',
  'root = Card("leave_card", [n0, n1, n2, submit])'
].join('\n')

const chartView = [
  'n0 = BarChart(["一", "二"], [1, 2])',
  'n1 = Text("note: 已汇总完成")',
  'root = Card("本周", [n0, n1])'
].join('\n')

const controls = [
  'days = Slider("days", "天数", 5, 1, 14)',
  'shift = Select("shift", "班次", "上午", ["上午", "下午"])',
  'agree = Checkbox("agree", "确认", false)',
  'root = Card("排班", [days, shift, agree])'
].join('\n')

const invalid = 'root = Metric("Total", {alert(1)})'
const jsonFence = '说明\n```json\n{"root":"leave_card","state":{"form":{"cm_content":"去医院"}}}\n```\n'

function Fence({ source, streaming = false }: { source: string; streaming?: boolean }) {
  return (
    <>
      {splitOpenUiFences('```openui\n' + source + (streaming ? '' : '\n```')).map((segment) =>
        segment.kind === 'openui' ? (
          <OpenUiMessage
            key={segment.start}
            source={segment.source}
            streaming={streaming || segment.incomplete === true}
          />
        ) : (
          <p key={segment.start}>{segment.source}</p>
        )
      )}
    </>
  )
}

function Coverage() {
  const [answer, setAnswer] = useState('')
  return (
    <IntelligentUiSubmitContext
      value={async (text) => {
        setAnswer(text)
        return 'sent'
      }}>
      <main>
        <h1>Intelligent UI coverage</h1>
        <section aria-label="view" data-case="view">
          <h2>查看</h2>
          <Fence source={leaveView} />
        </section>
        <section aria-label="form" data-case="form">
          <h2>表单</h2>
          <IntelligentUiSubmitContext
            value={async (text) => {
              setAnswer(text)
              return 'sent'
            }}>
            <Fence source={leaveForm} />
          </IntelligentUiSubmitContext>
        </section>
        <section aria-label="chart" data-case="chart">
          <h2>图表</h2>
          <Fence source={chartView} />
        </section>
        <section aria-label="controls" data-case="controls">
          <h2>控件</h2>
          <Fence source={controls} />
        </section>
        <section aria-label="json" data-case="json">
          <h2>普通 JSON</h2>
          {splitOpenUiFences(jsonFence).map((segment) => (
            <pre key={segment.start} aria-label="raw json">
              {segment.source}
            </pre>
          ))}
        </section>
        <section aria-label="invalid" data-case="invalid">
          <h2>无效界面</h2>
          <Fence source={invalid} />
        </section>
        <pre aria-label="User continuation">{answer}</pre>
      </main>
    </IntelligentUiSubmitContext>
  )
}

createRoot(document.getElementById('root')!).render(<Coverage />)
