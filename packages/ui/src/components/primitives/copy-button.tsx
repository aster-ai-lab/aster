// Original path: src/renderer/components/CopyButton.tsx
import { Check, Copy } from 'lucide-react'
import { useState, type FC, type MouseEvent } from 'react'

import { cn } from '../../lib/utils'

import { Button } from './button'
import { Tooltip } from './tooltip'

interface CopyButtonProps {
  tooltip?: string
  label?: string
  size?: number
  className?: string
  /** Text to copy; when set the button writes it to the clipboard itself. */
  value?: string
  [key: string]: any
}

const CopyButton: FC<CopyButtonProps> = ({ tooltip, label, size = 14, className = '', value, onClick, ...props }) => {
  const [copied, setCopied] = useState(false)

  const handleClick = async (event: MouseEvent<HTMLButtonElement>) => {
    if (value !== undefined && typeof navigator !== 'undefined' && navigator.clipboard) {
      try {
        await navigator.clipboard.writeText(value)
        setCopied(true)
        window.setTimeout(() => setCopied(false), 1500)
      } catch {
        // clipboard unavailable (permissions / non-secure context); keep the click
      }
    }
    onClick?.(event)
  }

  const button = (
    <Button
      variant="ghost"
      size="icon-sm"
      aria-label={tooltip ?? label ?? 'Copy'}
      className={cn('rounded-md', className)}
      onClick={handleClick}
      {...props}>
      {copied ? <Check size={size} strokeWidth={1.7} /> : <Copy size={size} strokeWidth={1.7} />}
      {label && <span style={{ fontSize: `${size}px` }}>{label}</span>}
    </Button>
  )

  if (tooltip) {
    return <Tooltip content={tooltip}>{button}</Tooltip>
  }

  return button
}

export default CopyButton
export { CopyButton }
export type { CopyButtonProps }
