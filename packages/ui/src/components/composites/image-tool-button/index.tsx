import { memo } from 'react'

// Original path: src/renderer/components/Preview/ImageToolButton.tsx
import { Button } from '../../primitives/button'
import { Tooltip } from '../../primitives/tooltip'

interface ImageToolButtonProps {
  tooltip: string
  icon: React.ReactNode
  onPress: () => void
}

const ImageToolButton = ({ tooltip, icon, onPress }: ImageToolButtonProps) => {
  return (
    <Tooltip content={tooltip} delay={500}>
      <Button size="icon" className="rounded-full" onClick={onPress} aria-label={tooltip}>
        {icon}
      </Button>
    </Tooltip>
  )
}

export default memo(ImageToolButton)
