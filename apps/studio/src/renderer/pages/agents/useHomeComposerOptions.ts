import { useCallback, useSyncExternalStore } from 'react'
import { useTranslation } from 'react-i18next'

import { toast } from '@renderer/services/toast'

import {
  EMPTY_HOME_COMPOSER_FLAGS,
  HOME_COMPOSER_OPTIONS_CHANGED,
  readHomeComposerOptions,
  writeHomeComposerOptions,
  type HomeComposerFlags
} from './workbenchHomeCapabilities'

let snapshot = EMPTY_HOME_COMPOSER_FLAGS
function getSnapshot() {
  const next = readHomeComposerOptions()
  if (
    next.reviewMode !== snapshot.reviewMode ||
    next.artifactTrace !== snapshot.artifactTrace ||
    next.researchMode !== snapshot.researchMode
  )
    snapshot = next
  return snapshot
}

function subscribe(change: () => void) {
  window.addEventListener(HOME_COMPOSER_OPTIONS_CHANGED, change)
  window.addEventListener('storage', change)
  return () => {
    window.removeEventListener(HOME_COMPOSER_OPTIONS_CHANGED, change)
    window.removeEventListener('storage', change)
  }
}

/** Fallback and live input share one snapshot; mounting cannot overwrite a newer choice. */
export function useHomeComposerOptions() {
  const { t } = useTranslation()
  const options = useSyncExternalStore(subscribe, getSnapshot, () => EMPTY_HOME_COMPOSER_FLAGS)
  const changeOptions = useCallback(
    (next: HomeComposerFlags) => {
      if (!writeHomeComposerOptions(next)) toast.error(t('workbench.composer.options_save_failed'))
    },
    [t]
  )
  return { options, changeOptions }
}
