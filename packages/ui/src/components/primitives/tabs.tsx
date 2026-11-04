import * as TabsPrimitive from '@radix-ui/react-tabs'
import { cva } from 'class-variance-authority'
import {
  AnimatePresence,
  LayoutGroup,
  animate,
  motion,
  type AnimationPlaybackControls,
  type Variants
} from 'motion/react'
import * as React from 'react'

import { useReducedMotion } from '../../lib/useReducedMotion'
import { cn } from '../../lib/utils'
import { motionTokens } from '../arc/lib/motion-tokens'
import { SelectionHighlight } from './selection-highlight'

import arcTabs from '../arc/tabs/tabs.module.css'

type TabsVariant = 'default' | 'plain' | 'line' | 'underline' | 'workflow'

const TabsContext = React.createContext<{
  variant?: TabsVariant
  orientation?: 'horizontal' | 'vertical'
  value?: string
  highlightId: string
  direction: number
  panelHeightRef: React.RefObject<number | null>
  leavingRectRef: React.RefObject<DOMRect | null>
}>({
  variant: 'default',
  orientation: 'horizontal',
  highlightId: '',
  direction: 1,
  panelHeightRef: { current: null },
  leavingRectRef: { current: null }
})

function Tabs({
  className,
  variant = 'default',
  orientation = 'horizontal',
  value,
  defaultValue,
  onValueChange,
  ...props
}: React.ComponentProps<typeof TabsPrimitive.Root> & {
  /** `plain` keeps Arc plate travel without the segmented shell (sidebar / dialog rails). */
  variant?: TabsVariant
}) {
  const [internalValue, setInternalValue] = React.useState(defaultValue ?? '')
  const [direction, setDirection] = React.useState(1)
  const selectedValue = value ?? internalValue
  const highlightId = React.useId()
  const root = React.useRef<HTMLDivElement>(null)
  const panelHeightRef = React.useRef<number | null>(null)
  const leavingRectRef = React.useRef<DOMRect | null>(null)

  function handleChange(next: string) {
    const frame = root.current
    leavingRectRef.current =
      frame?.querySelector(':scope > [role="tabpanel"][data-state="active"]')?.getBoundingClientRect() ?? null
    const order = frame
      ? Array.from(frame.querySelectorAll<HTMLElement>('[role="tab"][data-value]'))
          .filter((tab) => tab.closest('[data-slot="tabs"]') === frame)
          .map((tab) => tab.dataset.value)
      : []
    const from = order.indexOf(selectedValue)
    const to = order.indexOf(next)
    if (from >= 0 && to >= 0 && from !== to) setDirection(to > from ? 1 : -1)
    if (value === undefined) setInternalValue(next)
    onValueChange?.(next)
  }

  return (
    <TabsContext
      value={{
        variant,
        orientation,
        value: selectedValue,
        highlightId,
        direction,
        panelHeightRef,
        leavingRectRef
      }}>
      {/* LayoutGroup scopes the shared selection plate so it glides inside this instance only. */}
      <LayoutGroup id={highlightId}>
        <TabsPrimitive.Root
          ref={root}
          data-slot="tabs"
          orientation={orientation}
          value={selectedValue}
          onValueChange={handleChange}
          className={cn(
            'relative flex flex-col gap-2',
            orientation === 'vertical' && 'flex-row',
            className
          )}
          {...props}
        />
      </LayoutGroup>
    </TabsContext>
  )
}

const tabsListVariants = cva('inline-flex items-center justify-center', {
  variants: {
    variant: {
      default: 'h-9 w-fit rounded-[8px] bg-[#f5f5f5] p-1 text-[#6f6f6f] dark:bg-[#2c2c2c] dark:text-[#9a9a9a]',
      plain: 'h-auto w-full flex-col gap-1 bg-transparent p-0',
      line: 'justify-start gap-4 border-b-0 bg-transparent p-0',
      underline: 'justify-start gap-0 border-b-0 bg-transparent p-0',
      workflow: 'justify-start gap-3 border-b-0 bg-transparent p-0'
    },
    orientation: {
      horizontal: 'flex-row',
      vertical: 'h-fit flex-col'
    }
  },
  compoundVariants: [
    {
      variant: 'default',
      orientation: 'vertical',
      class: 'h-fit w-fit flex-col'
    },
    {
      variant: 'line',
      orientation: 'vertical',
      class: 'flex-col items-stretch pb-0'
    }
  ],
  defaultVariants: {
    variant: 'default',
    orientation: 'horizontal'
  }
})

function TabsList({ className, ...props }: React.ComponentProps<typeof TabsPrimitive.List>) {
  const { variant, orientation } = React.use(TabsContext)
  // Segmented shell is Arc default; `plain` is plate-only (rails); line/underline/workflow stay native.
  const isShell = variant === 'default'
  const isPlain = variant === 'plain'
  return (
    <TabsPrimitive.List
      data-slot="tabs-list"
      className={cn(
        isShell
          ? cn('arc', arcTabs.listShell, 'w-fit', orientation === 'vertical' && 'flex-col')
          : isPlain
            ? cn(
                'arc inline-flex items-stretch justify-start gap-1 bg-transparent p-0',
                orientation === 'vertical' ? 'h-auto w-full flex-col' : 'w-fit flex-row'
              )
            : tabsListVariants({ variant, orientation }),
        className
      )}
      {...props}
    />
  )
}

const tabsTriggerVariants = cva(
  [
    'inline-flex items-center justify-center text-sm font-medium whitespace-nowrap',
    'disabled:pointer-events-none disabled:opacity-40',
    'relative isolate motion-safe:transition-colors motion-safe:duration-150 outline-none',
    '[&_svg]:pointer-events-none [&_svg]:shrink-0 [&_svg:not([class*="size-"])]:size-4'
  ],
  {
    variants: {
      variant: {
        default: [
          'h-[calc(100%-1px)] flex-1 gap-1.5 rounded-[6px] px-2 py-1',
          'border border-transparent text-foreground',
          'dark:text-muted-foreground',
          'focus-visible:border-transparent focus-visible:bg-accent',
          'data-[state=active]:text-[#171717]',
          'dark:data-[state=active]:border-transparent dark:data-[state=active]:text-[#f5f5f5]'
        ],
        plain: [
          'h-8 justify-start gap-1.5 rounded-[10px] px-2.5',
          'border border-transparent text-muted-foreground',
          'hover:text-foreground',
          'data-[state=active]:font-medium data-[state=active]:text-foreground'
        ],
        line: [
          'relative gap-2 px-2 py-2',
          'font-normal text-muted-foreground hover:text-foreground',
          'focus-visible:text-foreground focus-visible:underline focus-visible:underline-offset-4',
          'data-[state=active]:text-foreground',
          'after:absolute after:rounded-full after:bg-transparent',
          'after:pointer-events-none'
        ],
        underline: [
          'relative gap-1.5 px-2.5 py-2',
          'font-normal text-muted-foreground hover:text-foreground',
          'focus-visible:text-foreground focus-visible:underline focus-visible:underline-offset-4',
          'data-[state=active]:text-foreground',
          'after:absolute after:rounded-none after:bg-transparent',
          'after:pointer-events-none'
        ],
        workflow: [
          'relative gap-1.5 px-1 py-1.5 text-sm font-normal',
          'text-muted-foreground hover:text-foreground',
          'rounded-sm focus-visible:text-foreground focus-visible:underline focus-visible:underline-offset-4',
          'data-[state=active]:font-semibold data-[state=active]:text-foreground',
          'data-[state=active]:underline data-[state=active]:decoration-1 data-[state=active]:underline-offset-4',
          "[&:not(:first-child)]:before:content-['›']",
          '[&:not(:first-child)]:before:mr-3 [&:not(:first-child)]:before:text-base',
          '[&:not(:first-child)]:before:font-normal [&:not(:first-child)]:before:no-underline',
          '[&:not(:first-child)]:before:text-muted-foreground'
        ]
      },
      orientation: {
        horizontal: '',
        vertical: 'rounded-[6px]'
      }
    },
    compoundVariants: [
      {
        variant: 'line',
        orientation: 'horizontal',
        class: 'after:bottom-0 after:left-0 after:h-[2px] after:w-full'
      },
      {
        variant: 'line',
        orientation: 'vertical',
        class: [
          'justify-center after:bottom-0 after:left-0 after:h-[4px] after:w-full after:bg-transparent',
          'hover:bg-[#f5f5f5] hover:text-[#171717] dark:hover:bg-[#2c2c2c] dark:hover:text-[#f5f5f5]'
        ]
      },
      {
        variant: 'underline',
        orientation: 'horizontal',
        class: 'after:bottom-0 after:left-0 after:h-0.5 after:w-full'
      }
    ],
    defaultVariants: {
      variant: 'default',
      orientation: 'horizontal'
    }
  }
)

function TabsTrigger({
  asChild,
  className,
  children,
  value,
  ...props
}: React.ComponentProps<typeof TabsPrimitive.Trigger>) {
  const { variant, orientation, value: selectedValue, highlightId } = React.use(TabsContext)
  const usesArcTrigger = variant === 'default' || variant === 'plain'
  const selected = value === selectedValue
  // Arc shared-highlight needs the plate as a sibling under a relative owner. `asChild`
  // only admits one child, so plate travel is unavailable — callers should prefer a
  // styled TabsTrigger (or an explicit SelectionHighlight) over Slot composition.
  const showPlate = variant !== 'workflow' && selected && !asChild
  return (
    <TabsPrimitive.Trigger
      asChild={asChild}
      data-slot="tabs-trigger"
      data-value={value}
      value={value}
      className={cn(
        usesArcTrigger
          ? cn(
              'arc',
              arcTabs.trigger,
              variant === 'default' && 'flex-1',
              'gap-1.5 [&_svg:not([class*="size-"])]:size-4'
            )
          : tabsTriggerVariants({ variant, orientation }),
        className
      )}
      {...props}>
      {showPlate && (
        <SelectionHighlight
          data-slot="tabs-indicator"
          layoutId={highlightId}
          layoutDependency={value}
          className={
            usesArcTrigger
              ? arcTabs.selection
              : cn('inset-x-0 top-auto bottom-0 bg-foreground', variant === 'line' ? 'h-1' : 'h-0.5')
          }
          style={usesArcTrigger ? undefined : { borderRadius: 2 }}
        />
      )}
      {asChild ? (
        children
      ) : (
        <span className={usesArcTrigger ? arcTabs.triggerLabel : 'relative z-[1] inline-flex items-center gap-1.5'}>
          {children}
        </span>
      )}
    </TabsPrimitive.Trigger>
  )
}

const panelMotion: Variants = {
  enter: (direction: number) => ({ opacity: 0, x: direction * 8 }),
  center: {
    opacity: 1,
    x: 0,
    transition: {
      opacity: { duration: motionTokens.duration.standard, ease: [...motionTokens.ease.enter] },
      x: motionTokens.spring.smooth
    }
  },
  exit: (direction: number) => ({
    opacity: 0,
    x: direction * -6,
    transition: { duration: motionTokens.duration.instant, ease: [...motionTokens.ease.standard] }
  })
}

/** Reduced motion changes panels immediately in place. */
const panelFade: Variants = {
  enter: { opacity: 0, x: 0 },
  center: { opacity: 1, x: 0, transition: { duration: 0 } },
  exit: { opacity: 0, x: 0, transition: { duration: 0 } }
}

function TabsContent({
  className,
  value,
  forceMount,
  children,
  ...props
}: React.ComponentProps<typeof TabsPrimitive.Content>) {
  const { value: active, direction, panelHeightRef, leavingRectRef } = React.use(TabsContext)
  const reduced = useReducedMotion()
  const panel = React.useRef<HTMLDivElement>(null)
  const selected = active === value
  const classes = cn('flex-1 outline-none', className)

  // The incoming panel starts at the outgoing panel's height and settles at its own, so content below glides instead of jumping.
  React.useLayoutEffect(() => {
    const node = panel.current
    if (!node || forceMount) return
    if (!selected) {
      const before = leavingRectRef.current
      const now = node.getBoundingClientRect()
      if (before) node.style.translate = `${before.left - now.left}px ${before.top - now.top}px`
      node.inert = true
      return
    }
    node.style.translate = ''
    node.inert = false
    const from = panelHeightRef.current
    const to = node.offsetHeight
    let controls: AnimationPlaybackControls | undefined
    const release = () => {
      node.style.height = ''
      node.style.overflow = ''
    }
    if (from !== null && Math.abs(from - to) > 1 && !reduced) {
      if (to > from) node.style.overflow = 'clip'
      controls = animate(node, { height: [from, to] }, { ...motionTokens.spring.smooth, onComplete: release })
    }
    panelHeightRef.current = controls && from !== null ? from : to
    let observer: { disconnect(): void } | null = null
    if (typeof globalThis.ResizeObserver === 'function') {
      const resizeObserver = new globalThis.ResizeObserver(() => {
        panelHeightRef.current = node.offsetHeight
      })
      resizeObserver.observe(node)
      observer = resizeObserver
    }
    return () => {
      observer?.disconnect()
      controls?.stop()
      release()
    }
  }, [selected, reduced, forceMount, panelHeightRef, leavingRectRef])

  if (forceMount) {
    return (
      <TabsPrimitive.Content
        data-slot="tabs-content"
        {...props}
        value={value}
        forceMount
        className={classes}>
        {children}
      </TabsPrimitive.Content>
    )
  }

  return (
    <AnimatePresence initial={false} mode="popLayout" custom={direction}>
      {selected && (
        <TabsPrimitive.Content {...props} key={value} value={value} forceMount asChild>
          <motion.div
            ref={panel}
            data-slot="tabs-content"
            className={classes}
            custom={direction}
            variants={reduced ? panelFade : panelMotion}
            initial="enter"
            animate="center"
            exit="exit">
            {children}
          </motion.div>
        </TabsPrimitive.Content>
      )}
    </AnimatePresence>
  )
}

export { Tabs, TabsContent, TabsList, TabsTrigger }
