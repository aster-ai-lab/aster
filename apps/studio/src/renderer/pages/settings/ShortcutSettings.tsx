import {
  Button,
  DropdownMenu,
  DropdownMenuContent,
  DropdownMenuItem,
  DropdownMenuSeparator,
  DropdownMenuTrigger,
  Kbd,
  SegmentedControl,
  RowFlex,
  Switch,
  Tooltip
} from '@a3s-lab/ui'
import { getRouteApi } from '@tanstack/react-router'
import { isEmpty } from 'es-toolkit/compat'
import { MoreHorizontal, Undo2 } from 'lucide-react'
import type { FC, KeyboardEvent as ReactKeyboardEvent } from 'react'
import { useEffect, useMemo, useRef, useState } from 'react'
import { useTranslation } from 'react-i18next'

import { preferenceService } from '@data/PreferenceService'
import { loggerService } from '@logger'
import CollapsibleSearchBar from '@renderer/components/CollapsibleSearchBar'
import Scrollbar from '@renderer/components/Scrollbar'
import { SettingsContentBody } from '@renderer/components/SettingsPrimitives'
import {
  getAllShortcutDefaultPreferences,
  type ShortcutSettingsGroup,
  useCommandShortcuts
} from '@renderer/hooks/command'
import { useTheme } from '@renderer/hooks/useTheme'
import { useTimer } from '@renderer/hooks/useTimer'
import { settingsContentHeaderClassName, settingsContentScrollClassName } from '@renderer/pages/settings/settingsStyles'
import { shortcutAnchorId } from '@renderer/pages/settings/shortcut.search'
import { ShortcutCommandIcon, ShortcutFilterLabel, ShortcutGroupIcon } from '@renderer/pages/settings/shortcutIcons'
import { popup } from '@renderer/services/popup'
import { toast } from '@renderer/services/toast'
import type { AppRouter } from '@renderer/types/router'
import { scrollIntoView } from '@renderer/utils/dom'
import { isMac, platform } from '@renderer/utils/platform'
import { cn } from '@renderer/utils/style'
import type { PreferenceShortcutType } from '@shared/data/preference/preferenceTypes'
import type { SupportedPlatform } from '@shared/types/command'
import type { ShortcutPreferenceKey } from '@shared/types/shortcut'
import { type CommandId, findCommandDefinition, findKeybindingConflicts } from '@shared/utils/command'
import {
  convertKeyToAccelerator,
  formatKeyDisplay,
  formatShortcutDisplay,
  isValidShortcut,
  normalizeShortcutToken,
  type ShortcutBinding,
  type ShortcutToken
} from '@shared/utils/shortcut'

const logger = loggerService.withContext('ShortcutSettings')
const shortcutRouteApi = getRouteApi('/settings/shortcut')

const isBindingEqual = (a: ShortcutBinding, b: ShortcutBinding): boolean =>
  a.length === b.length && a.every((key, index) => key === b[index])

const keyCodeToAccelerator: Record<string, ShortcutToken> = {
  Backquote: '`',
  Period: '.',
  NumpadEnter: 'numenter',
  NumpadAdd: 'numadd',
  NumpadSubtract: 'numsub',
  Space: 'Space',
  Enter: 'Enter',
  Backspace: 'Backspace',
  Tab: 'Tab',
  Delete: 'Delete'
}

const passthrough =
  /^(Page(Up|Down)|Insert|Home|End|CapsLock|Arrow(Up|Down|Left|Right)|F([1-9]|1\d|2[0-4])|Slash|Semicolon|Bracket(Left|Right)|Backslash|Quote|Comma|Minus|Equal)$/

const usableEndKeys = (code: string): ShortcutToken | null => {
  if (/^Key[A-Z]$/.test(code) || /^(Digit|Numpad)\d$/.test(code)) return normalizeShortcutToken(code) ?? null
  if (keyCodeToAccelerator[code]) return keyCodeToAccelerator[code]
  if (passthrough.test(code)) return convertKeyToAccelerator(code) ?? null
  return null
}

type ShortcutSettingsFilterGroup = 'all' | ShortcutSettingsGroup

const ShortcutSettings: FC = () => {
  const { t } = useTranslation()
  const { theme } = useTheme()
  const { shortcuts, updatePreference } = useCommandShortcuts()
  const recorderRefs = useRef<Record<string, HTMLButtonElement>>({})
  const [editingKey, setEditingKey] = useState<string | null>(null)
  const [pendingKeys, setPendingKeys] = useState<ShortcutBinding>([])
  const [conflictLabel, setConflictLabel] = useState<string | null>(null)
  const [systemConflictKey, setSystemConflictKey] = useState<ShortcutPreferenceKey | null>(null)
  const [searchQuery, setSearchQuery] = useState('')
  const { setTimeoutTimer, clearTimeoutTimer } = useTimer()

  // `?command=<id>` arrives from pages that own a feature but not its shortcut, so landing
  // here mid-list would leave the user hunting. Highlight fades; the scroll stays put.
  // `?section=` opens this same page with that category already selected.
  const { command: focusedCommand, section } = shortcutRouteApi.useSearch<AppRouter>()
  const [activeGroup, setActiveGroup] = useState<ShortcutSettingsFilterGroup>(section ?? 'all')
  const focusedRowRef = useRef<HTMLDivElement | null>(null)
  const [focusFaded, setFocusFaded] = useState(false)

  useEffect(() => {
    setActiveGroup(section ?? 'all')
    setSearchQuery('')
  }, [section])

  useEffect(() => {
    if (!focusedCommand || !focusedRowRef.current) return
    scrollIntoView(focusedRowRef.current)
    setFocusFaded(false)
    setTimeoutTimer('focus-fade', () => setFocusFaded(true), 2000)
  }, [focusedCommand, setTimeoutTimer])

  const groupMeta = useMemo(
    () => [
      { key: 'general' as const, label: t('settings.shortcuts.categories.general') },
      { key: 'chat' as const, label: t('settings.shortcuts.categories.chat') },
      { key: 'topic' as const, label: t('settings.shortcuts.categories.topic') },
      { key: 'assistant' as const, label: t('settings.shortcuts.categories.assistant') }
    ],
    [t]
  )

  const shortcutsByGroup = useMemo(() => {
    return shortcuts.reduce<Record<ShortcutSettingsGroup, typeof shortcuts>>(
      (acc, shortcut) => {
        acc[shortcut.group].push(shortcut)
        return acc
      },
      { general: [], chat: [], topic: [], assistant: [] }
    )
  }, [shortcuts])

  const segmentOptions = useMemo(
    () => [
      {
        value: 'all' as const,
        label: <ShortcutFilterLabel group="all" label={t('settings.shortcuts.categories.all')} />
      },
      ...groupMeta.flatMap((group) =>
        shortcutsByGroup[group.key].length === 0
          ? []
          : [{ value: group.key, label: <ShortcutFilterLabel group={group.key} label={group.label} /> }]
      )
    ],
    [groupMeta, shortcutsByGroup, t]
  )

  const visibleSections = useMemo(() => {
    const query = searchQuery.toLowerCase().trim()
    const matches = (record: (typeof shortcuts)[number]) => {
      if (!query) return true
      const display =
        record.preference.binding.length > 0
          ? formatShortcutDisplay(record.preference.binding, isMac).toLowerCase()
          : ''
      return record.label.toLowerCase().includes(query) || display.includes(query)
    }
    const keys = activeGroup === 'all' ? groupMeta.map((group) => group.key) : [activeGroup]
    return keys.flatMap((key) => {
      const rows = shortcutsByGroup[key].filter(matches)
      if (rows.length === 0) return []
      const label = groupMeta.find((group) => group.key === key)?.label ?? key
      return [{ key, label, rows }]
    })
  }, [activeGroup, groupMeta, searchQuery, shortcutsByGroup])

  const visibleShortcuts = visibleSections.flatMap((section) => section.rows)

  const shortcutPreferences = useMemo(
    () =>
      shortcuts.reduce<Partial<Record<CommandId, PreferenceShortcutType>>>((acc, shortcut) => {
        acc[shortcut.command] = shortcut.preference
        return acc
      }, {}),
    [shortcuts]
  )

  const clearEditingState = () => {
    clearTimeoutTimer('conflict-clear')
    setEditingKey(null)
    setPendingKeys([])
    setConflictLabel(null)
  }

  const clearSystemConflict = (key?: ShortcutPreferenceKey) => {
    setSystemConflictKey((currentKey) => {
      if (!key || currentKey === key) {
        return null
      }
      return currentKey
    })
  }

  useEffect(() => {
    const listen = window.api?.shortcut?.onRegistrationConflict
    if (!listen) return
    return listen(({ key, hasConflict }) => {
      setSystemConflictKey((currentKey) => {
        if (hasConflict) {
          return key
        }
        return currentKey === key ? null : currentKey
      })

      if (hasConflict) {
        toast.error(t('settings.shortcuts.occupied_by_other_application'))
      }
    })
  }, [t])

  useEffect(() => {
    if (activeGroup === 'all' || shortcutsByGroup[activeGroup].length > 0) return
    const firstAvailable = groupMeta.find((group) => shortcutsByGroup[group.key].length > 0)
    if (firstAvailable && firstAvailable.key !== activeGroup) setActiveGroup(firstAvailable.key)
  }, [activeGroup, groupMeta, shortcutsByGroup])

  const handleAddShortcut = (key: ShortcutPreferenceKey) => {
    clearEditingState()
    setEditingKey(key)
    setTimeoutTimer(
      `focus-${key}`,
      () => {
        recorderRefs.current[key]?.focus()
      },
      0
    )
  }

  const handleUpdateFailure = (record: (typeof shortcuts)[number], error: unknown) => {
    logger.error(`Failed to update shortcut preference: ${record.key}`, error as Error)
    toast.error(t('settings.shortcuts.save_failed_with_name', { name: record.label }))
  }

  const handleResetShortcut = async (record: (typeof shortcuts)[number]) => {
    const conflict = findConflictLabel(record.command, {
      binding: record.defaultPreference.binding,
      enabled: record.defaultPreference.enabled
    })
    if (conflict) {
      showConflictToast(conflict)
      return
    }

    try {
      clearSystemConflict(record.key)
      await updatePreference(record.key, {
        binding: record.defaultPreference.binding,
        enabled: record.defaultPreference.enabled
      })
      clearEditingState()
    } catch (error) {
      handleUpdateFailure(record, error)
    }
  }

  const getCommandLabel = (command: CommandId): string => {
    const shortcut = shortcuts.find((item) => item.command === command)
    if (shortcut) {
      return shortcut.label
    }

    const definition = findCommandDefinition(command)
    return definition ? t(definition.titleKey) : command
  }

  const findConflictLabel = (
    command: CommandId,
    preference: PreferenceShortcutType,
    preferences = shortcutPreferences
  ): string | null => {
    const conflict = findKeybindingConflicts({
      command,
      preference,
      preferences,
      platform: platform as SupportedPlatform
    })[0]
    return conflict ? getCommandLabel(conflict.conflictingCommand) : null
  }

  const showConflictToast = (label: string) => {
    toast.error(t('settings.shortcuts.conflict_with', { name: label }))
  }

  const handleKeyDown = async (event: ReactKeyboardEvent, record: (typeof shortcuts)[number]) => {
    event.preventDefault()
    event.stopPropagation()

    if (event.nativeEvent.isComposing || event.key === 'Process') {
      return
    }

    if (event.code === 'Escape') {
      clearEditingState()
      return
    }

    if (event.key === 'Backspace' && !event.ctrlKey && !event.altKey && !event.metaKey && !event.shiftKey) {
      setConflictLabel(null)
      try {
        await updatePreference(record.key, { binding: [], enabled: false })
        clearEditingState()
      } catch (error) {
        handleUpdateFailure(record, error)
      }
      return
    }

    const keys: ShortcutToken[] = []

    if (event.ctrlKey) keys.push(isMac ? 'Ctrl' : 'CommandOrControl')
    if (event.altKey) keys.push('Alt')
    if (event.metaKey) keys.push(isMac ? 'CommandOrControl' : 'Meta')
    if (event.shiftKey) keys.push('Shift')

    const endKey = usableEndKeys(event.code)
    if (endKey) {
      keys.push(endKey)
    }

    const binding: ShortcutBinding = keys
    setPendingKeys(binding)

    if (!isValidShortcut(binding)) {
      setConflictLabel(null)
      return
    }

    const conflict = findConflictLabel(record.command, { binding, enabled: true })
    if (conflict) {
      setConflictLabel(conflict)
      clearTimeoutTimer('conflict-clear')
      setTimeoutTimer('conflict-clear', () => setConflictLabel(null), 2000)
      return
    }

    setConflictLabel(null)
    try {
      clearSystemConflict(record.key)
      await updatePreference(record.key, { binding, enabled: true })
      clearEditingState()
    } catch (error) {
      handleUpdateFailure(record, error)
    }
  }

  const handleResetAllShortcuts = async () => {
    const confirmed = await popup.confirm({
      title: t('settings.shortcuts.reset_defaults_confirm'),
      centered: true
    })
    if (!confirmed) return

    const updates: Record<string, PreferenceShortcutType> = getAllShortcutDefaultPreferences()

    try {
      clearSystemConflict()
      await preferenceService.setMultiple(updates)
    } catch (error) {
      logger.error('Failed to reset all shortcuts to defaults', error as Error)
      toast.error(t('settings.shortcuts.reset_defaults_failed'))
    }
  }

  const handleToggleVisibleShortcuts = async (enabled: boolean) => {
    const nextPreferencesByCommand: Partial<Record<CommandId, PreferenceShortcutType>> = { ...shortcutPreferences }
    const updates = visibleShortcuts.reduce(
      (acc, record) => {
        // Non-editable commands keep a fixed binding. The bulk switch leaves them alone.
        if (!record.preference.binding.length || record.keybinding.editable === false) return acc
        nextPreferencesByCommand[record.command] = {
          binding: record.preference.binding,
          enabled
        }
        acc[record.key] = {
          binding: record.preference.binding,
          enabled
        }
        return acc
      },
      {} as Record<string, PreferenceShortcutType>
    )

    if (isEmpty(updates)) return

    if (enabled) {
      for (const record of visibleShortcuts) {
        const nextPreference = nextPreferencesByCommand[record.command]
        if (!nextPreference?.enabled || !nextPreference.binding.length) continue

        const conflict = findConflictLabel(record.command, nextPreference, nextPreferencesByCommand)
        if (conflict) {
          showConflictToast(conflict)
          return
        }
      }
    }

    try {
      clearSystemConflict()
      await preferenceService.setMultiple(updates)
    } catch (error) {
      logger.error(`Failed to toggle shortcuts for group ${activeGroup}`, error as Error)
      toast.error(t('settings.shortcuts.save_failed'))
    }
  }

  const renderShortcutCell = (record: (typeof shortcuts)[number]) => {
    const isEditing = editingKey === record.key
    const displayKeys = record.preference.binding
    const displayShortcut = displayKeys.length > 0 ? formatShortcutDisplay(displayKeys, isMac) : ''
    const isEditable = record.keybinding.editable !== false
    const isBindingModified = !isBindingEqual(displayKeys, record.defaultPreference.binding)
    const hasSystemConflict = systemConflictKey === record.key
    const conflictMessage =
      conflictLabel ?? (hasSystemConflict ? t('settings.shortcuts.occupied_by_other_application') : null)

    if (isEditing) {
      const pendingDisplay = pendingKeys.length > 0 ? formatShortcutDisplay(pendingKeys, isMac) : ''
      const hasConflict = conflictMessage !== null

      return (
        <div className="relative flex flex-col items-end">
          <Button
            ref={(el) => {
              if (el) recorderRefs.current[record.key] = el
            }}
            type="button"
            variant="ghost"
            className={cn(
              'h-[30px] w-36 rounded-[8px] border border-[#e6e6e6] bg-[#f5f5f5] text-center text-[13px] text-[#171717] shadow-none hover:bg-[#fafafa]',
              !pendingDisplay && 'text-[#8a8a8a]',
              hasConflict && 'border-[#b42318] text-[#b42318]'
            )}
            onKeyDown={(event) => void handleKeyDown(event, record)}
            onBlur={(event) => {
              const isUndoClick = (event.relatedTarget as HTMLElement)?.closest('.shortcut-undo-icon')
              if (!isUndoClick) {
                clearEditingState()
              }
            }}>
            {pendingDisplay || t('settings.shortcuts.press_shortcut')}
          </Button>
          {hasConflict && (
            <span className="absolute top-full right-0 mt-1 text-[12px] whitespace-nowrap text-[#b42318]">
              {conflictLabel ? t('settings.shortcuts.conflict_with', { name: conflictLabel }) : conflictMessage}
            </span>
          )}
        </div>
      )
    }

    if (displayShortcut) {
      return (
        <div className="relative flex flex-col items-end">
          <RowFlex className="items-center justify-end gap-2">
            {isBindingModified && (
              <Tooltip content={t('settings.shortcuts.reset_to_default')}>
                <Undo2
                  size={16}
                  className="shortcut-undo-icon cursor-pointer text-muted-foreground opacity-70 transition-opacity hover:opacity-100"
                  onClick={() => {
                    void handleResetShortcut(record)
                  }}
                />
              </Tooltip>
            )}
            <RowFlex
              className={cn(
                'min-h-9 items-center gap-1 rounded-[8px] border border-transparent bg-transparent px-2 py-1 transition-colors',
                hasSystemConflict && 'border-[#b42318]',
                isEditable ? 'cursor-pointer hover:bg-[#f5f5f5]' : 'cursor-not-allowed opacity-50'
              )}
              onClick={() => isEditable && handleAddShortcut(record.key)}>
              {displayKeys.map((key) => (
                <Kbd
                  key={key}
                  className={cn(
                    'min-w-6 rounded-[6px] border border-[#e6e6e6] bg-white px-1.5 py-0.75 text-[12px] text-[#171717] shadow-none',
                    hasSystemConflict && 'border-[#b42318] text-[#b42318]'
                  )}>
                  {formatKeyDisplay(key, isMac)}
                </Kbd>
              ))}
            </RowFlex>
          </RowFlex>
          {hasSystemConflict && (
            <span className="absolute top-full right-0 mt-1 text-[12px] whitespace-nowrap text-[#b42318]">
              {conflictMessage}
            </span>
          )}
        </div>
      )
    }

    return (
      <div className="relative flex flex-col items-end">
        <span
          className={cn(
            'rounded-[8px] px-2.5 py-1.5 text-[13px] text-[#8a8a8a] transition-colors hover:text-[#171717]',
            hasSystemConflict && 'text-[#b42318]',
            isEditable ? 'cursor-pointer hover:bg-[#f5f5f5]' : 'cursor-not-allowed opacity-50'
          )}
          onClick={() => isEditable && handleAddShortcut(record.key)}>
          {t('settings.shortcuts.press_shortcut')}
        </span>
        {hasSystemConflict && (
          <span className="absolute top-full right-0 mt-1 text-[12px] whitespace-nowrap text-[#b42318]">
            {conflictMessage}
          </span>
        )}
      </div>
    )
  }

  const renderShortcutRow = (record: (typeof shortcuts)[number], isLast: boolean) => {
    const switchNode = (
      <Switch
        size="sm"
        checked={record.preference.enabled}
        disabled={!record.preference.binding.length || record.keybinding.editable === false}
        onCheckedChange={() => {
          const nextPreference = {
            binding: record.preference.binding,
            enabled: !record.preference.enabled
          }
          if (nextPreference.enabled) {
            const conflict = findConflictLabel(record.command, nextPreference)
            if (conflict) {
              showConflictToast(conflict)
              return
            }
          }

          clearSystemConflict(record.key)
          updatePreference(record.key, { enabled: nextPreference.enabled }).catch((error) => {
            handleUpdateFailure(record, error)
          })
        }}
      />
    )

    return (
      <div
        key={record.key}
        ref={record.command === focusedCommand ? focusedRowRef : undefined}
        id={`setting-shortcut-${shortcutAnchorId(record.command)}`}
        data-focused={record.command === focusedCommand || undefined}
        className={cn(
          'grid scroll-mt-6 grid-cols-[minmax(0,1fr)_14rem_2.5rem] items-center gap-3 rounded-[8px] px-2.5 py-1.5',
          !record.preference.enabled && 'opacity-60',
          !isLast && 'border-b border-[#efefef]',
          record.command === focusedCommand && !focusFaded
            ? 'bg-[#f2f3f5] text-[#171717] ring-2 ring-black/10 ring-inset'
            : 'text-[#3f3f3f] hover:bg-[#f7f7f7] hover:text-[#171717]'
        )}>
        <div className="flex min-w-0 items-center gap-2.5 pr-2">
          <ShortcutCommandIcon command={record.command} />
          <div className="min-w-0 truncate text-[13px]">{record.label}</div>
        </div>
        <div className="flex min-h-9 items-center justify-end">{renderShortcutCell(record)}</div>
        <div className="flex justify-end">
          {!record.preference.binding.length ? (
            <Tooltip content={t('settings.shortcuts.bind_first_to_enable')}>
              <span>{switchNode}</span>
            </Tooltip>
          ) : (
            switchNode
          )}
        </div>
      </div>
    )
  }

  return (
    <div className="flex flex-1" data-theme-mode={theme}>
      <div className="flex h-full min-h-0 w-full flex-1 overflow-hidden">
        <Scrollbar className={settingsContentScrollClassName}>
          <SettingsContentBody className="pt-2">
            <div className={cn(settingsContentHeaderClassName, 'flex flex-wrap items-center justify-between gap-3')}>
              <SegmentedControl<ShortcutSettingsFilterGroup>
                size="sm"
                aria-label={t('settings.shortcuts.title')}
                options={segmentOptions}
                value={segmentOptions.some((option) => option.value === activeGroup) ? activeGroup : 'all'}
                onValueChange={(value) => {
                  setActiveGroup(value)
                  setSearchQuery('')
                }}
              />
              <div className="ml-auto flex items-center gap-2">
                <CollapsibleSearchBar
                  value={searchQuery}
                  onSearch={setSearchQuery}
                  placeholder={t('settings.shortcuts.search_placeholder')}
                  tooltip={t('common.search')}
                  clearLabel={t('common.clear')}
                  maxWidth={220}
                  collapsedSize={30}
                  style={{ background: '#f5f5f5', border: 'none', borderRadius: 8, boxShadow: 'none' }}
                />
                <DropdownMenu>
                  <DropdownMenuTrigger asChild>
                    <Button
                      variant="ghost"
                      size="icon-sm"
                      className="size-[30px] shrink-0 rounded-[8px] text-[#3f3f3f] shadow-none hover:bg-[#f5f5f5] hover:text-[#171717]"
                      aria-label={t('common.more')}>
                      <MoreHorizontal className="size-4" />
                    </Button>
                  </DropdownMenuTrigger>
                  <DropdownMenuContent align="end" className="min-w-36">
                    <DropdownMenuItem onSelect={() => void handleToggleVisibleShortcuts(true)}>
                      {t('settings.shortcuts.all_enable')}
                    </DropdownMenuItem>
                    <DropdownMenuItem onSelect={() => void handleToggleVisibleShortcuts(false)}>
                      {t('settings.shortcuts.all_disable')}
                    </DropdownMenuItem>
                    <DropdownMenuSeparator />
                    <DropdownMenuItem variant="destructive" onSelect={handleResetAllShortcuts}>
                      {t('settings.shortcuts.reset')}
                    </DropdownMenuItem>
                  </DropdownMenuContent>
                </DropdownMenu>
              </div>
            </div>

            {visibleSections.length > 0 ? (
              visibleSections.map((section) => (
                <section key={section.key} className="mb-4 last:mb-0">
                  {activeGroup === 'all' ? (
                    <h2 className="m-0 flex items-center gap-1.5 px-2.5 pt-2 pb-1 text-[12px] leading-4 font-normal text-[#8a8a8a]">
                      <ShortcutGroupIcon group={section.key} />
                      {section.label}
                    </h2>
                  ) : null}
                  <div>
                    {section.rows.map((record, index) => renderShortcutRow(record, index === section.rows.length - 1))}
                  </div>
                </section>
              ))
            ) : (
              <div className="py-10 text-center text-[13px] text-[#8a8a8a]">{t('settings.shortcuts.empty')}</div>
            )}
          </SettingsContentBody>
        </Scrollbar>
      </div>
    </div>
  )
}

export default ShortcutSettings
