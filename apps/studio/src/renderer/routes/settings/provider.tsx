import { createFileRoute, redirect } from '@tanstack/react-router'

/** The model-service page is not in this product. Old links land on general settings. */
export const Route = createFileRoute('/settings/provider')({
  beforeLoad: () => {
    throw redirect({ to: '/settings/general' })
  }
})
