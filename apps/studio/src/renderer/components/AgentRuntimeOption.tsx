import type { TFunction } from 'i18next'
import { Check } from 'lucide-react'
import { useId, type SVGProps } from 'react'

import {
  Item,
  ItemActions,
  ItemContent,
  ItemDescription,
  ItemMedia,
  ItemTitle,
  Label,
  RadioGroup,
  RadioGroupItem
} from '@a3s-lab/ui'
import { type IconComponent, PiCli } from '@a3s-lab/ui/icons'
import { ClaudeCode, Deepseek } from '@a3s-lab/ui/icons/providers'
import { cn } from '@a3s-lab/ui/lib/utils'
import { AGENT_RUNTIME_CAPABILITIES } from '@shared/ai/agentRuntimeCapabilities'
import type { AgentType } from '@shared/data/types/agent'

/**
 * Shared presentation for the agent runtimes.
 *
 * The runtime is picked once and never again, so both surfaces render the same card: the create
 * wizard makes them selectable, the editor shows the chosen one as a plain summary. Keeping them
 * here means the two never drift into looking like different decisions.
 */

const A3sCodeIcon: IconComponent = (props: SVGProps<SVGSVGElement>) => (
  <svg viewBox="0 0 24 24" fill="none" stroke="currentColor" strokeWidth="1.75" aria-hidden {...props}>
    <path d="M7 17 L12 6 L17 17" />
    <path d="M9 13h6" />
  </svg>
)

const RUNTIME_ICONS = {
  'claude-code': ClaudeCode,
  pi: PiCli,
  dsh: Deepseek,
  'a3s-code': A3sCodeIcon
} satisfies Record<AgentType, IconComponent>

const COMPACT_RUNTIME_ICON_CLASS: Record<AgentType, string> = {
  'claude-code': 'size-6',
  pi: 'size-4',
  dsh: 'size-7',
  'a3s-code': 'size-6'
}

const RUNTIME_DESCRIPTION_KEYS: Record<AgentType, string> = {
  // t('library.config.agent.field.runtime.option_description.claude_code')
  'claude-code': 'library.config.agent.field.runtime.option_description.claude_code',
  // t('library.config.agent.field.runtime.option_description.pi')
  pi: 'library.config.agent.field.runtime.option_description.pi',
  // t('library.config.agent.field.runtime.option_description.dsh')
  dsh: 'library.config.agent.field.runtime.option_description.dsh',
  // t('library.config.agent.field.runtime.option_description.a3s_code')
  'a3s-code': 'library.config.agent.field.runtime.option_description.a3s_code'
}

const RUNTIMES = (Object.keys(AGENT_RUNTIME_CAPABILITIES) as AgentType[]).filter((runtime) => runtime !== 'dsh')
const RUNTIME_CARD_CLASS_NAME = 'w-full items-center gap-2 rounded-lg px-3 py-1.5 font-normal'

function RuntimeCardBody({ runtime, t, compact = false }: { runtime: AgentType; t: TFunction; compact?: boolean }) {
  const caps = AGENT_RUNTIME_CAPABILITIES[runtime]
  const Icon = RUNTIME_ICONS[runtime]

  return (
    <>
      <ItemMedia
        variant={compact ? 'default' : 'icon'}
        className={cn(
          compact
            ? 'size-7 self-center text-foreground group-has-[[data-slot=item-description]]/item:translate-y-0 group-has-[[data-slot=item-description]]/item:self-center'
            : 'border-border-subtle bg-muted/60'
        )}>
        <Icon className={compact ? COMPACT_RUNTIME_ICON_CLASS[runtime] : undefined} />
      </ItemMedia>
      <ItemContent className={cn('min-w-0 text-left', compact && 'gap-0.5')}>
        <ItemTitle className={compact ? 'block max-w-full truncate font-medium leading-4' : undefined}>
          {t(caps.labelKey, caps.labelFallback)}
        </ItemTitle>
        <ItemDescription className={cn('text-xs', compact && 'min-w-0 text-ellipsis whitespace-nowrap leading-4')}>
          {t(RUNTIME_DESCRIPTION_KEYS[runtime])}
        </ItemDescription>
      </ItemContent>
    </>
  )
}

export function AgentRuntimeTiles({
  value,
  onValueChange,
  ariaLabel,
  t
}: {
  value: AgentType
  onValueChange: (value: AgentType) => void
  ariaLabel: string
  t: TFunction
}) {
  const uid = useId()

  // Radix owns the radio semantics (roving tabindex, arrow-key navigation); the visual is a card, so
  // the radio control itself is hidden and the card carries the selected and focus states.
  return (
    <RadioGroup
      aria-label={ariaLabel}
      className="grid-cols-1 gap-2"
      value={value}
      onValueChange={(next) => onValueChange(next as AgentType)}>
      {RUNTIMES.map((runtime) => {
        const optionId = `${uid}-${runtime}`
        const selected = runtime === value
        return (
          <Item
            key={runtime}
            asChild
            size="sm"
            variant="outline"
            className={cn(
              RUNTIME_CARD_CLASS_NAME,
              'cursor-pointer hover:bg-accent/50',
              // Keep keyboard focus visible while selection is indicated by the checkmark alone.
              'has-[[data-slot=radio-group-item]:focus-visible]:bg-accent',
              'has-[[data-slot=radio-group-item]:focus-visible]:ring-1 has-[[data-slot=radio-group-item]:focus-visible]:ring-ring has-[[data-slot=radio-group-item]:focus-visible]:ring-inset'
            )}>
            <Label htmlFor={optionId}>
              <RadioGroupItem id={optionId} value={runtime} className="sr-only" />
              <RuntimeCardBody runtime={runtime} t={t} compact />
              <ItemActions className="size-4 shrink-0">
                {selected ? <Check className="size-4 text-foreground" /> : null}
              </ItemActions>
            </Label>
          </Item>
        )
      })}
    </RadioGroup>
  )
}

/** The runtime an agent already has. Not a control — there is nothing left to choose. */
export function AgentRuntimeSummary({ value, t }: { value: AgentType; t: TFunction }) {
  return (
    <Item size="sm" variant="outline" className={RUNTIME_CARD_CLASS_NAME}>
      <RuntimeCardBody runtime={value} t={t} compact />
    </Item>
  )
}
