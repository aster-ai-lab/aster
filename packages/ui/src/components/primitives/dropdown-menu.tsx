import * as DropdownMenuPrimitive from '@radix-ui/react-dropdown-menu'
import { CheckIcon, ChevronRightIcon, CircleIcon } from 'lucide-react'
import * as React from 'react'

import { cn } from '../../lib/utils'
import { PopupDismissContext, usePopupDismissal, usePopupState } from './popup-dismissal'
import { usePortalContainer } from './portal-container'

function DropdownMenu({
  open,
  defaultOpen,
  onOpenChange,
  modal = false,
  ...props
}: React.ComponentProps<typeof DropdownMenuPrimitive.Root>) {
  const state = usePopupState({ open, defaultOpen, onOpenChange })
  return (
    <PopupDismissContext value={state}>
      <DropdownMenuPrimitive.Root data-slot="dropdown-menu" modal={modal} {...props} {...state} />
    </PopupDismissContext>
  )
}

function DropdownMenuPortal({ container, ...props }: React.ComponentProps<typeof DropdownMenuPrimitive.Portal>) {
  const defaultPortalContainer = usePortalContainer()
  return (
    <DropdownMenuPrimitive.Portal
      data-slot="dropdown-menu-portal"
      container={container ?? defaultPortalContainer ?? undefined}
      {...props}
    />
  )
}

function DropdownMenuTrigger({ ...props }: React.ComponentProps<typeof DropdownMenuPrimitive.Trigger>) {
  return <DropdownMenuPrimitive.Trigger data-slot="dropdown-menu-trigger" {...props} />
}

type DropdownMenuContentProps = React.ComponentProps<typeof DropdownMenuPrimitive.Content> & {
  portalContainer?: React.ComponentProps<typeof DropdownMenuPrimitive.Portal>['container']
}

/**
 * A press on the menu's own trigger re-opens or closes the menu itself, so the
 * layer it must not reach is the one that is still animating out. Exported for
 * the primitive's tests: the handler it guards can only be observed in a browser.
 */
export function isTriggerPress(event: { target: EventTarget | null }): boolean {
  const target = event.target as Element | null
  return Boolean(target?.closest?.('[data-slot="dropdown-menu-trigger"]'))
}

function DropdownMenuContent({
  className,
  sideOffset = 4,
  portalContainer,
  onPointerDownOutside,
  ref,
  onPointerDownCapture,
  onClickCapture,
  onCloseAutoFocus,
  ...props
}: DropdownMenuContentProps) {
  const dismissal = usePopupDismissal(ref)
  return (
    <DropdownMenuPortal container={portalContainer}>
      <DropdownMenuPrimitive.Content
        data-slot="dropdown-menu-content"
        sideOffset={sideOffset}
        ref={dismissal.ref}
        onPointerDownCapture={(event) => {
          dismissal.inside()
          onPointerDownCapture?.(event)
        }}
        onClickCapture={(event) => {
          dismissal.inside()
          onClickCapture?.(event)
        }}
        onCloseAutoFocus={(event) => {
          onCloseAutoFocus?.(event)
          dismissal.onCloseAutoFocus(event)
        }}
        // Popups keep their content mounted for the length of the leave animation
        // in popup-motion.css (200ms), and that dying layer is still registered as
        // a dismissable layer. A press on the trigger during that window is
        // therefore an "outside" press for it, and it dismisses the menu the same
        // press just opened: the menu blinks shut and the trigger looks dead. Hand
        // the trigger's own press back to the trigger.
        onPointerDownOutside={(event) => {
          onPointerDownOutside?.(event)
          if (event.defaultPrevented) return
          if (isTriggerPress(event)) event.preventDefault()
        }}
        className={cn(
          // no-drag punches the popup's area out of any titlebar drag region it overlaps,
          // so hover/click reach the items instead of the window-drag hit test (Electron).
          'z-[240] max-h-(--radix-dropdown-menu-content-available-height) min-w-[8rem] origin-(--radix-dropdown-menu-content-transform-origin) overflow-x-hidden overflow-y-auto rounded-[12px] border border-[color:var(--color-border)] bg-[var(--color-bg-elevated)] p-1.5 text-[13px] text-[var(--color-ink)] shadow-[var(--shadow-elevated)] [-webkit-app-region:no-drag] data-[state=closed]:pointer-events-none',
          className
        )}
        {...props}
      />
    </DropdownMenuPortal>
  )
}

function DropdownMenuGroup({ ...props }: React.ComponentProps<typeof DropdownMenuPrimitive.Group>) {
  return <DropdownMenuPrimitive.Group data-slot="dropdown-menu-group" {...props} />
}

function DropdownMenuItem({
  className,
  inset,
  variant = 'default',
  ...props
}: React.ComponentProps<typeof DropdownMenuPrimitive.Item> & {
  inset?: boolean
  variant?: 'default' | 'destructive'
}) {
  return (
    <DropdownMenuPrimitive.Item
      data-slot="dropdown-menu-item"
      data-inset={inset}
      data-variant={variant}
      className={cn(
        "relative flex cursor-default items-center gap-2 rounded-[8px] px-2 py-1.5 text-[13px] text-[var(--color-ink-muted)] outline-hidden select-none focus:bg-[var(--color-surface-2)] focus:text-[var(--color-ink)] data-[disabled]:pointer-events-none data-[disabled]:opacity-50 data-[inset]:pl-8 data-[variant=destructive]:text-destructive data-[variant=destructive]:focus:bg-destructive/10 data-[variant=destructive]:focus:text-destructive dark:data-[variant=destructive]:focus:bg-destructive/20 [&_svg]:pointer-events-none [&_svg]:shrink-0 [&_svg:not([class*='size-'])]:size-4 [&_svg:not([class*='text-'])]:text-muted-foreground data-[variant=destructive]:[&_svg:not([class*='text-'])]:text-destructive!",
        className
      )}
      {...props}
    />
  )
}

function DropdownMenuCheckboxItem({
  className,
  children,
  checked,
  ...props
}: React.ComponentProps<typeof DropdownMenuPrimitive.CheckboxItem>) {
  return (
    <DropdownMenuPrimitive.CheckboxItem
      data-slot="dropdown-menu-checkbox-item"
      className={cn(
        "relative flex cursor-default items-center gap-2 rounded-[8px] py-1.5 pr-2 pl-8 text-[13px] text-[var(--color-ink-muted)] outline-hidden select-none focus:bg-[var(--color-surface-2)] focus:text-[var(--color-ink)] data-[disabled]:pointer-events-none data-[disabled]:opacity-50 [&_svg]:pointer-events-none [&_svg]:shrink-0 [&_svg:not([class*='size-'])]:size-4",
        className
      )}
      checked={checked}
      {...props}>
      <span className="pointer-events-none absolute left-2 flex size-3.5 items-center justify-center">
        <DropdownMenuPrimitive.ItemIndicator>
          <CheckIcon className="size-4" />
        </DropdownMenuPrimitive.ItemIndicator>
      </span>
      {children}
    </DropdownMenuPrimitive.CheckboxItem>
  )
}

function DropdownMenuRadioGroup({ ...props }: React.ComponentProps<typeof DropdownMenuPrimitive.RadioGroup>) {
  return <DropdownMenuPrimitive.RadioGroup data-slot="dropdown-menu-radio-group" {...props} />
}

function DropdownMenuRadioItem({
  className,
  children,
  ...props
}: React.ComponentProps<typeof DropdownMenuPrimitive.RadioItem>) {
  return (
    <DropdownMenuPrimitive.RadioItem
      data-slot="dropdown-menu-radio-item"
      className={cn(
        "relative flex cursor-default items-center gap-2 rounded-[8px] py-1.5 pr-2 pl-8 text-[13px] text-[var(--color-ink-muted)] outline-hidden select-none focus:bg-[var(--color-surface-2)] focus:text-[var(--color-ink)] data-[disabled]:pointer-events-none data-[disabled]:opacity-50 [&_svg]:pointer-events-none [&_svg]:shrink-0 [&_svg:not([class*='size-'])]:size-4",
        className
      )}
      {...props}>
      <span className="pointer-events-none absolute left-2 flex size-3.5 items-center justify-center">
        <DropdownMenuPrimitive.ItemIndicator>
          <CircleIcon className="size-2 fill-current" />
        </DropdownMenuPrimitive.ItemIndicator>
      </span>
      {children}
    </DropdownMenuPrimitive.RadioItem>
  )
}

function DropdownMenuLabel({
  className,
  inset,
  ...props
}: React.ComponentProps<typeof DropdownMenuPrimitive.Label> & {
  inset?: boolean
}) {
  return (
    <DropdownMenuPrimitive.Label
      data-slot="dropdown-menu-label"
      data-inset={inset}
      className={cn('px-2 py-1.5 text-sm font-medium data-[inset]:pl-8', className)}
      {...props}
    />
  )
}

function DropdownMenuSeparator({ className, ...props }: React.ComponentProps<typeof DropdownMenuPrimitive.Separator>) {
  return (
    <DropdownMenuPrimitive.Separator
      data-slot="dropdown-menu-separator"
      className={cn('-mx-1 my-1 h-px bg-border', className)}
      {...props}
    />
  )
}

function DropdownMenuShortcut({ className, ...props }: React.ComponentProps<'span'>) {
  return (
    <span
      data-slot="dropdown-menu-shortcut"
      className={cn('ml-auto text-xs tracking-widest text-muted-foreground', className)}
      {...props}
    />
  )
}

function DropdownMenuSub({ ...props }: React.ComponentProps<typeof DropdownMenuPrimitive.Sub>) {
  return <DropdownMenuPrimitive.Sub data-slot="dropdown-menu-sub" {...props} />
}

function DropdownMenuSubTrigger({
  className,
  inset,
  children,
  ...props
}: React.ComponentProps<typeof DropdownMenuPrimitive.SubTrigger> & {
  inset?: boolean
}) {
  return (
    <DropdownMenuPrimitive.SubTrigger
      data-slot="dropdown-menu-sub-trigger"
      data-inset={inset}
      className={cn(
        "flex cursor-default items-center gap-2 rounded-[8px] px-2 py-1.5 text-[13px] text-[var(--color-ink-muted)] outline-hidden select-none focus:bg-[var(--color-surface-2)] focus:text-[var(--color-ink)] data-[inset]:pl-8 data-[state=open]:bg-[var(--color-surface-2)] data-[state=open]:text-[var(--color-ink)] [&_svg]:pointer-events-none [&_svg]:shrink-0 [&_svg:not([class*='size-'])]:size-4 [&_svg:not([class*='text-'])]:text-muted-foreground",
        className
      )}
      {...props}>
      {children}
      <ChevronRightIcon className="ml-auto size-4" />
    </DropdownMenuPrimitive.SubTrigger>
  )
}

function DropdownMenuSubContent({
  className,
  ...props
}: React.ComponentProps<typeof DropdownMenuPrimitive.SubContent>) {
  return (
    <DropdownMenuPortal>
      <DropdownMenuPrimitive.SubContent
        data-slot="dropdown-menu-sub-content"
        className={cn(
          'z-[240] min-w-[8rem] origin-(--radix-dropdown-menu-content-transform-origin) overflow-hidden rounded-[12px] border border-[color:var(--color-border)] bg-[var(--color-bg-elevated)] p-1.5 text-[13px] text-[var(--color-ink)] shadow-[var(--shadow-elevated)] [-webkit-app-region:no-drag]',
          className
        )}
        {...props}
      />
    </DropdownMenuPortal>
  )
}

export {
  DropdownMenu,
  DropdownMenuCheckboxItem,
  DropdownMenuContent,
  DropdownMenuGroup,
  DropdownMenuItem,
  DropdownMenuLabel,
  DropdownMenuPortal,
  DropdownMenuRadioGroup,
  DropdownMenuRadioItem,
  DropdownMenuSeparator,
  DropdownMenuShortcut,
  DropdownMenuSub,
  DropdownMenuSubContent,
  DropdownMenuSubTrigger,
  DropdownMenuTrigger
}
