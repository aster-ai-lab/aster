import { test } from '@playwright/test'

test('车道 B 文件交接到主窗口', async () => {
  test.skip(
    true,
    '不另起 Studio，也不附着正在运行的 ToWork。deliver_workbench_review 与系统工作区回落仍由壳层库测试和 AgentPage 单测证明。'
  )
})
