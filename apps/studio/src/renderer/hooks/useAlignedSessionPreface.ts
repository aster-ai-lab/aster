import { useEffect, useState } from 'react'

import { usePreference } from '@data/hooks/usePreference'
import {
  applyAlignedSessionPreface,
  prefaceComposerText,
  type AlignedPreface
} from '@renderer/utils/alignedSessionPreface'
import type { CodeReviewMode } from '@renderer/utils/codeReviewMode'
import { getCodeReviewMode } from '@renderer/utils/codeReviewMode'
import { loadCustomInstructions } from '@renderer/utils/customInstructions'

export { applyAlignedSessionPreface, prefaceComposerText, type AlignedPreface }

/** Loads the saved preface on the chat path. Custom instructions come from the preference store. */
export function useAlignedSessionPreface(): AlignedPreface {
  const [customInstructions] = usePreference('app.custom_instructions')
  const [reviewMode] = usePreference('app.code_review.mode')
  const [loaded, setLoaded] = useState<string | null>(null)

  useEffect(() => {
    let active = true
    void loadCustomInstructions()
      .then((text) => {
        if (active) setLoaded(text)
      })
      .catch(() => {
        if (active) setLoaded(null)
      })
    return () => {
      active = false
    }
  }, [customInstructions])

  const mode: CodeReviewMode = reviewMode === 'subagent' || reviewMode === 'inline' ? reviewMode : getCodeReviewMode()

  return {
    customInstructions: loaded ?? (typeof customInstructions === 'string' ? customInstructions : ''),
    reviewMode: mode
  }
}
