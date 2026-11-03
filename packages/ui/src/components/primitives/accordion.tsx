import * as AccordionPrimitive from '@radix-ui/react-accordion'
import { ChevronDownIcon } from 'lucide-react'
import * as React from 'react'

import { cn } from '../../lib/utils'

import arcAccordion from '../arc/accordion/accordion.module.css'

function Accordion({ className, ...props }: React.ComponentProps<typeof AccordionPrimitive.Root>) {
  return (
    <AccordionPrimitive.Root
      data-slot="accordion"
      className={cn('arc', arcAccordion.accordion, className)}
      {...props}
    />
  )
}

function AccordionItem({ className, ...props }: React.ComponentProps<typeof AccordionPrimitive.Item>) {
  return (
    <AccordionPrimitive.Item
      data-slot="accordion-item"
      className={cn('arc', arcAccordion.item, className)}
      {...props}
    />
  )
}

function AccordionTrigger({ className, children, ...props }: React.ComponentProps<typeof AccordionPrimitive.Trigger>) {
  return (
    <AccordionPrimitive.Header className="flex">
      <AccordionPrimitive.Trigger
        data-slot="accordion-trigger"
        className={cn(
          'arc',
          arcAccordion.trigger,
          'flex-1 disabled:pointer-events-none disabled:opacity-40 [&[data-state=open]>svg]:rotate-180',
          className
        )}
        {...props}>
        {children}
        <ChevronDownIcon
          className={cn(
            arcAccordion.icon,
            'pointer-events-none size-4 shrink-0 motion-safe:transition-transform motion-safe:duration-200'
          )}
        />
      </AccordionPrimitive.Trigger>
    </AccordionPrimitive.Header>
  )
}

type AccordionContentProps = React.ComponentProps<typeof AccordionPrimitive.Content> & {
  contentClassName?: string
}

function AccordionContent({ className, contentClassName, children, ...props }: AccordionContentProps) {
  return (
    <AccordionPrimitive.Content
      data-slot="accordion-content"
      className={cn(
        'arc',
        arcAccordion.panel,
        'motion-safe:data-[state=closed]:animate-accordion-up motion-safe:data-[state=open]:animate-accordion-down',
        contentClassName
      )}
      {...props}>
      <div className={cn('pt-2 pb-4', className)}>{children}</div>
    </AccordionPrimitive.Content>
  )
}

export { Accordion, AccordionContent, AccordionItem, AccordionTrigger }
