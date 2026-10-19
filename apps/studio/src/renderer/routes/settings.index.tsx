import { createFileRoute, redirect } from '@tanstack/react-router'

// /settings/ 重定向到侧栏第一项
export const Route = createFileRoute('/settings/')({
  beforeLoad: () => {
    throw redirect({ to: '/settings/general' })
  }
})
