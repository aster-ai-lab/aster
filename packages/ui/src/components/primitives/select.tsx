import * as SelectPrimitive from '@radix-ui/react-select'
import { cva, type VariantProps } from 'class-variance-authority'
import { CheckIcon, ChevronDownIcon, ChevronUpIcon } from 'lucide-react'
import * as React from 'react'

import { cn } from '../../lib/utils'
import { PopupHighlight } from './popup-highlight'
import { usePortalContainer } from './portal-container'

import arcSelect from '../arc/select/select.module.css'

type SelectContextValue = {
  contentElement: HTMLElement | null
  open: boolean
  setContentElement: (element: HTMLElement | null) => void
  setOpen: (open: boolean) => void
  setTriggerElement: (element: HTMLElement | null) => void
  triggerElement: HTMLElement | null
}

const SelectContext = React.createContext<SelectContextValue | null>(null)

function assignRef<T>(ref: React.Ref<T> | undefined, value: T | null) {
  if (typeof ref === 'function') {
    ref(value)
  } else if (ref) {
    ref.current = value
  }
}

const selectTriggerVariants = cva(
  cn(
    'inline-flex items-center justify-between rounded-[8px] border border-transparent text-[13px] font-normal transition-colors outline-none',
    'bg-[#f5f5f5] text-[#171717] dark:bg-[#2c2c2c] dark:text-[#f5f5f5]'
  ),
  {
    variants: {
      state: {
        default:
          'focus-visible:border-transparent focus-visible:ring-2 focus-visible:ring-inset focus-visible:ring-black/10',
        error: 'border border-destructive!',
        disabled: 'pointer-events-none cursor-not-allowed opacity-50'
      },
      size: {
        sm: 'h-[30px] gap-2 px-2.5',
        default: 'h-9 gap-2 px-3'
      }
    },
    defaultVariants: {
      state: 'default',
      size: 'default'
    }
  }
)

/**
 * @deprecated Prefer `SearchSelect` (or `Combobox` when the trigger must read as a text field).
 * DESIGN.md §6 / constitution XVI: product pickers are drawn; this Radix composite is shrink-only debt.
 */
function Select({
  defaultOpen,
  onOpenChange,
  open: openProp,
  ...props
}: React.ComponentProps<typeof SelectPrimitive.Root>) {
  const [contentElement, setContentElement] = React.useState<HTMLElement | null>(null)
  const [internalOpen, setInternalOpen] = React.useState(defaultOpen ?? false)
  const [triggerElement, setTriggerElement] = React.useState<HTMLElement | null>(null)
  const isControlled = openProp !== undefined
  const open = isControlled ? openProp : internalOpen

  const setOpen = React.useCallback(
    (nextOpen: boolean) => {
      if (!isControlled) setInternalOpen(nextOpen)
      onOpenChange?.(nextOpen)
    },
    [isControlled, onOpenChange]
  )

  const contextValue = React.useMemo(
    () => ({ contentElement, open, setContentElement, setOpen, setTriggerElement, triggerElement }),
    [contentElement, open, setOpen, triggerElement]
  )

  return (
    <SelectContext value={contextValue}>
      <SelectPrimitive.Root data-slot="select" open={open} onOpenChange={setOpen} {...props} />
    </SelectContext>
  )
}

function SelectGroup({ ...props }: React.ComponentProps<typeof SelectPrimitive.Group>) {
  return <SelectPrimitive.Group data-slot="select-group" {...props} />
}

function SelectValue({ ...props }: React.ComponentProps<typeof SelectPrimitive.Value>) {
  return <SelectPrimitive.Value data-slot="select-value" {...props} />
}

function SelectTrigger({
  className,
  size = 'default',
  children,
  ref,
  ...props
}: React.ComponentProps<typeof SelectPrimitive.Trigger> &
  Omit<VariantProps<typeof selectTriggerVariants>, 'state'> & {
    size?: 'sm' | 'default'
  }) {
  const selectContext = React.use(SelectContext)
  const invalid =
    props['aria-invalid'] !== undefined && props['aria-invalid'] !== false && props['aria-invalid'] !== 'false'
  const state = props.disabled ? 'disabled' : invalid ? 'error' : 'default'
  const handleRef = React.useCallback(
    (node: HTMLButtonElement | null) => {
      selectContext?.setTriggerElement(node)
      assignRef(ref, node)
    },
    [ref, selectContext]
  )

  return (
    <SelectPrimitive.Trigger
      ref={handleRef}
      data-slot="select-trigger"
      data-size={size}
      className={cn(
        'arc',
        arcSelect.trigger,
        state === 'error' && 'border-destructive!',
        "w-fit whitespace-nowrap data-[placeholder]:text-muted-foreground *:data-[slot=select-value]:line-clamp-1 *:data-[slot=select-value]:flex *:data-[slot=select-value]:items-center *:data-[slot=select-value]:gap-2 [&_svg]:pointer-events-none [&_svg]:shrink-0 [&_svg:not([class*='size-'])]:size-4",
        className
      )}
      {...props}>
      {children}
      <SelectPrimitive.Icon asChild>
        <ChevronDownIcon className={cn(arcSelect.chevron, 'size-4')} />
      </SelectPrimitive.Icon>
    </SelectPrimitive.Trigger>
  )
}

function SelectContent({
  className,
  children,
  position = 'popper',
  align = 'center',
  portalContainer,
  ref,
  ...props
}: React.ComponentProps<typeof SelectPrimitive.Content> & {
  portalContainer?: React.ComponentProps<typeof SelectPrimitive.Portal>['container']
}) {
  const defaultPortalContainer = usePortalContainer()
  const selectContext = React.use(SelectContext)
  const resolvedPortalContainer = portalContainer ?? defaultPortalContainer ?? undefined
  const handleRef = React.useCallback(
    (node: HTMLDivElement | null) => {
      selectContext?.setContentElement(node)
      assignRef(ref, node)
    },
    [ref, selectContext]
  )

  React.useEffect(() => {
    const portalElement = resolvedPortalContainer instanceof HTMLElement ? resolvedPortalContainer : null
    if (!selectContext?.open || !portalElement) return

    const ownerDocument = portalElement.ownerDocument
    const ownerWindow = ownerDocument.defaultView ?? window
    const previousPointerEvents = portalElement.style.getPropertyValue('pointer-events')
    const previousPointerEventsPriority = portalElement.style.getPropertyPriority('pointer-events')

    const keepPortalContainerInteractive = () => {
      if (portalElement.style.pointerEvents !== 'auto') {
        portalElement.style.setProperty('pointer-events', 'auto', 'important')
      }
    }

    keepPortalContainerInteractive()

    const pointerEventsObserver = new ownerWindow.MutationObserver(keepPortalContainerInteractive)
    pointerEventsObserver.observe(portalElement, { attributes: true, attributeFilter: ['style'] })

    const closeForInsidePortalPointerDown = (event: PointerEvent) => {
      if (!(event.target instanceof Node)) return
      if (selectContext.contentElement?.contains(event.target)) return
      if (selectContext.triggerElement?.contains(event.target)) return
      if (!portalElement.contains(event.target)) return

      selectContext.setOpen(false)
    }

    ownerDocument.addEventListener('pointerdown', closeForInsidePortalPointerDown, true)

    return () => {
      pointerEventsObserver.disconnect()
      ownerDocument.removeEventListener('pointerdown', closeForInsidePortalPointerDown, true)
      if (portalElement.style.pointerEvents === 'auto') {
        portalElement.style.removeProperty('pointer-events')
      }
      if (previousPointerEvents && previousPointerEvents !== 'none') {
        portalElement.style.setProperty('pointer-events', previousPointerEvents, previousPointerEventsPriority)
      }
    }
  }, [resolvedPortalContainer, selectContext])

  return (
    <SelectPrimitive.Portal container={resolvedPortalContainer}>
      <SelectPrimitive.Content
        ref={handleRef}
        data-slot="select-content"
        className={cn(
          // no-drag punches the popup's area out of any titlebar drag region it overlaps,
          // so hover/click reach the items instead of the window-drag hit test (Electron).
          'arc',
          arcSelect.menuSurface,
          'relative z-[240] max-h-(--radix-select-content-available-height) min-w-[8rem] origin-(--radix-select-content-transform-origin) overflow-x-hidden overflow-y-auto text-[13px] [-webkit-app-region:no-drag]',
          className
        )}
        position={position}
        align={align}
        {...props}
        data-popup-surface="">
        <SelectScrollUpButton />
        <SelectPrimitive.Viewport
          className={cn(
            'p-1',
            position === 'popper' &&
              'h-[var(--radix-select-trigger-height)] w-full min-w-[var(--radix-select-trigger-width)] scroll-my-1'
          )}>
          <PopupHighlight />
          {children}
        </SelectPrimitive.Viewport>
        <SelectScrollDownButton />
      </SelectPrimitive.Content>
    </SelectPrimitive.Portal>
  )
}

function SelectLabel({ className, ...props }: React.ComponentProps<typeof SelectPrimitive.Label>) {
  return (
    <SelectPrimitive.Label
      data-slot="select-label"
      className={cn('px-2 py-1.5 text-xs text-muted-foreground', className)}
      {...props}
    />
  )
}

function SelectItem({ className, children, ...props }: React.ComponentProps<typeof SelectPrimitive.Item>) {
  return (
    <SelectPrimitive.Item
      data-slot="select-item"
      className={cn(
        'arc',
        arcSelect.item,
        'relative flex w-full gap-2 pr-8 pl-2 text-[13px] data-[disabled]:pointer-events-none data-[state=checked]:font-medium [&_svg]:pointer-events-none [&_svg]:shrink-0 [&_svg:not([class*="size-"])]:size-4',
        className
      )}
      {...props}>
      <span className="absolute right-2 flex size-3.5 items-center justify-center">
        <SelectPrimitive.ItemIndicator>
          <CheckIcon className="size-4 text-foreground" />
        </SelectPrimitive.ItemIndicator>
      </span>
      <SelectPrimitive.ItemText>{children}</SelectPrimitive.ItemText>
    </SelectPrimitive.Item>
  )
}

function SelectSeparator({ className, ...props }: React.ComponentProps<typeof SelectPrimitive.Separator>) {
  return (
    <SelectPrimitive.Separator
      data-slot="select-separator"
      className={cn('pointer-events-none -mx-1 my-1 h-px bg-border', className)}
      {...props}
    />
  )
}

function SelectScrollUpButton({ className, ...props }: React.ComponentProps<typeof SelectPrimitive.ScrollUpButton>) {
  return (
    <SelectPrimitive.ScrollUpButton
      data-slot="select-scroll-up-button"
      className={cn('flex cursor-default items-center justify-center py-1', className)}
      {...props}>
      <ChevronUpIcon className="size-4" />
    </SelectPrimitive.ScrollUpButton>
  )
}

function SelectScrollDownButton({
  className,
  ...props
}: React.ComponentProps<typeof SelectPrimitive.ScrollDownButton>) {
  return (
    <SelectPrimitive.ScrollDownButton
      data-slot="select-scroll-down-button"
      className={cn('flex cursor-default items-center justify-center py-1', className)}
      {...props}>
      <ChevronDownIcon className="size-4" />
    </SelectPrimitive.ScrollDownButton>
  )
}

/** @deprecated Prefer `SearchSelect` / `Combobox` — see `Select` JSDoc. */
export {
  Select,
  SelectContent,
  SelectGroup,
  SelectItem,
  SelectLabel,
  SelectScrollDownButton,
  SelectScrollUpButton,
  SelectSeparator,
  SelectTrigger,
  SelectValue
}
