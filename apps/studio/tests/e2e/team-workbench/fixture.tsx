import i18next from 'i18next'
import { createRoot } from 'react-dom/client'
import { initReactI18next } from 'react-i18next'

import zh from '../../../src/renderer/i18n/locales/zh-cn.json'
import { OrgWorkbench } from '../../../src/renderer/pages/org/OrgWorkbench'
import { notifyWorkInboxChanged } from '../../../src/renderer/workInbox/client'

await i18next
  .use(initReactI18next)
  .init({ lng: 'zh-CN', resources: { 'zh-CN': { translation: zh } }, interpolation: { escapeValue: false } })
localStorage.setItem('a3s.org.center', '11111111-1111-4111-8111-111111111111')
createRoot(document.getElementById('root')!).render(
  <main style={{ height: '100vh', display: 'flex', flexDirection: 'column' }}>
    <div style={{ padding: 12 }}>
      <button
        onClick={async () => {
          await fetch('/fixture/approver', { method: 'POST' })
          notifyWorkInboxChanged()
        }}>
        切换为审批人（测试）
      </button>
    </div>
    <OrgWorkbench
      orgId="11111111-1111-4111-8111-111111111111"
      org={{
        id: '11111111-1111-4111-8111-111111111111',
        name: '测试团队',
        logo: '',
        canReadWork: true,
        canCreateWork: true,
        rules: [
          { action: 'read', subject: 'WorkItem' },
          { action: 'create', subject: 'WorkItem' }
        ]
      }}
    />
  </main>
)
