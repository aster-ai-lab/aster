import type { TFunction } from 'i18next'

import { toast } from '@renderer/services/toast'

export function toastNotOpen(t: TFunction) {
  toast.info({ title: t('workbench.composer.not_open'), appearance: 'pill' })
}
