import type { ComponentProps } from 'react'

import ComposerSurface from './ComposerSurface'
import { ComposerScreenshotControl } from './screenshot/ComposerScreenshotControl'

type SurfaceProps = ComponentProps<typeof ComposerSurface>

export type HomeComposerInputProps = SurfaceProps & {
  /** The task form collects a prompt and does not capture the screen. */
  showScreenshot?: boolean
  screenshotDisabled?: boolean
  screenshotAcceptQueued?: boolean
}

/** Homepage input. The scheduled-task form uses this with the screenshot hidden. */
export function HomeComposerInput({
  showScreenshot = true,
  screenshotDisabled = false,
  screenshotAcceptQueued = false,
  renderLeftControls,
  setFiles,
  ...props
}: HomeComposerInputProps) {
  return (
    <ComposerSurface
      {...props}
      setFiles={setFiles}
      renderLeftControls={(inputAdapter, unifiedPanelControl) => (
        <>
          {showScreenshot ? (
            <ComposerScreenshotControl
              disabled={screenshotDisabled}
              setFiles={setFiles}
              acceptQueued={screenshotAcceptQueued}
            />
          ) : null}
          {renderLeftControls?.(inputAdapter, unifiedPanelControl)}
        </>
      )}
    />
  )
}
