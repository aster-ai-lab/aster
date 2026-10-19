import { Popover, PopoverContent, PopoverTrigger } from '@a3s-lab/ui'
import { ChevronDown, Link2, Paperclip, Plus, Shield, Sparkles } from 'lucide-react'
import {
  use,
  useCallback,
  useEffect,
  useLayoutEffect,
  useMemo,
  useRef,
  useState,
  useSyncExternalStore,
  type ReactNode
} from 'react'
import { createPortal } from 'react-dom'
import { useTranslation } from 'react-i18next'

import { useMemberProfile } from '@renderer/components/account/memberProfile'
import {
  type ComposerSuggestionSource,
  type ComposerUnifiedPanelControl,
  ROOT_QUICK_PANEL_ALLOWED_PREFIXES
} from '@renderer/components/composer/quickPanel'
import type { ComposerSerializedToken } from '@renderer/components/composer/tokens'
import { useOptionalComposerToolLaunchers } from '@renderer/components/composer/tools/ComposerToolProvider'
import { ATTACHMENT_TOOLBAR_MANIFEST } from '@renderer/components/composer/tools/toolbarManifests'
import { ModelSelector, type ModelSelectorFilter } from '@renderer/components/ModelSelector'
import { type QuickPanelInputAdapter, useOptionalQuickPanel } from '@renderer/components/QuickPanel'
import { useQuery } from '@renderer/data/hooks/useDataApi'
import { usePreference } from '@renderer/data/hooks/usePreference'
import { orgModelLogoId, type OrgModel } from '@renderer/data/orgModels'
import { useAssistantsApi } from '@renderer/hooks/useAssistant'
import { useAssistantCatalogPresets } from '@renderer/hooks/useAssistantCatalogPresets'
import { fileMentionRoot } from '@renderer/pages/settings/GeneralSettings'
import { ProviderAvatar } from '@renderer/pages/settings/ProviderSettings'
import type { ThinkingOption } from '@renderer/types/reasoning'
import { cn } from '@renderer/utils/style'
import type { Model } from '@shared/data/types/model'

import { homeAgentSuggestionItems, installedMentionAgents, rememberAgentAvatars } from './homeAgentCatalog'
import { CodeWorkspacePlacement } from './homeComposerMode'
import { homeSlashItems, useHomeLocalSkills } from './homeSlashCatalog'
import { ReviewComposerSource } from './ReviewComposerSource'
import { reviewDraftNeedsSource } from './reviewHandoff'
import { useHomeComposerOptions } from './useHomeComposerOptions'
import { useOrgModels } from './useOrgModels'
import { findWorkbenchComposerScope, WorkbenchComposerHostContext } from './workbenchComposerHost'
import {
  type HomeComposerFlags,
  HOME_COMPOSER_PINNED_TOOL_IDS,
  composerGoalObjective,
  composerGoalSummary,
  sameToolOrder,
  sessionGoalFromUserTexts,
  taskGoalPresentation,
  userTextsFromSessionMessages,
  withoutComposerGoal
} from './workbenchHomeCapabilities'
import {
  composerReasoningEffort,
  reasoningEffortColor,
  reasoningEffortLabelKey,
  visibleReasoningEffort,
  WorkbenchModelPicker
} from './WorkbenchModelPicker'
import { WorkbenchPermissionPicker } from './WorkbenchPermissionPicker'
import {
  displayedTaskPermissionMode,
  readTaskPermissionRevision,
  storedPermissionMode,
  subscribeTaskPermission,
  writePendingPermissionMode,
  writeTaskPermission
} from './workbenchPermissionPostures'
import { useWorkbenchProjectSelection, WorkbenchProjectTray } from './WorkbenchProjectTray'
import { type WorkbenchWorkspaceFilesControl } from './WorkbenchWorkspaceFiles'

export interface WorkbenchHomeComposerArgs {
  enabled: boolean
  fileCount: number
  model?: Model
  canChangeModel?: boolean
  onModelSelect?: (model: Model | undefined) => void
  modelFilter?: ModelSelectorFilter
  isModelDisabled?: ModelSelectorFilter
  workspace?: { type?: string; name?: string; path?: string } | null
  workspaceId?: string | null
  onWorkspaceChange?: (workspaceId: string | null) => void | Promise<void>
  workspaceChanging?: boolean
  permissionMode?: string
  onPermissionModeChange?: (mode: string) => void
  onModelUnavailable?: () => void
  /** Picks the model from the a3s OS catalog instead of the local selector. */
  catalogModelPicker?: boolean
  reasoningEffort?: ThinkingOption
  onReasoningEffortChange?: (effort: ThinkingOption) => void
  text: string
  tokens?: readonly ComposerSerializedToken[]
  onTextChange: (text: string) => void
  /** App workspace used when the composer has no project folder yet. */
  appWorkspacePath?: string
  /** Selected workspace files. The tray lists and toggles them beside the project menu. */
  workspaceFileSelection?: Pick<WorkbenchWorkspaceFilesControl, 'selectedPaths' | 'onCommit'>
  /** Task conversations hide the project menu. The goal stays on the right of the bottom toolbar. */
  projectLocked?: boolean
  /** The open task. Its own `/goal` messages are the only stored objective. */
  sessionId?: string | null
  /** Scope slot beside the conversation composer. Home finds its slot from the host. */
  scopeElement?: HTMLElement | null
}

interface ToolContext {
  inputAdapter?: QuickPanelInputAdapter
  unifiedPanelControl?: ComposerUnifiedPanelControl
}

const CHIP_CLASS =
  'flex h-[30px] items-center gap-1.5 rounded-[8px] px-2 text-[13px] leading-none text-[#3f3f3f] hover:bg-[#f7f7f7] hover:text-[#171717]'

/** Plus control. The popup lists attachment upload, 自动审查, and 产物追溯. */
export function WorkbenchComposerOptions({
  options,
  onChange,
  inputAdapter
}: {
  options: HomeComposerFlags
  onChange: (next: HomeComposerFlags) => void
  inputAdapter?: QuickPanelInputAdapter
}) {
  const { t } = useTranslation()
  const [open, setOpen] = useState(false)
  const launchers = useOptionalComposerToolLaunchers()
  const quickPanel = useOptionalQuickPanel()
  const attachment = launchers?.getLaunchers().find(
    (launcher) =>
      launcher.id === ATTACHMENT_TOOLBAR_MANIFEST.toolbar.id &&
      !launcher.hidden &&
      (!launcher.sources || launcher.sources.includes('popover'))
  )
  const canUpload = Boolean(attachment?.action && quickPanel && !attachment.disabled)
  const uploadLabel = typeof attachment?.label === 'string' ? attachment.label : t('chat.input.upload.attachment')
  const review = t('workbench.composer.review')
  const trace = t('workbench.composer.trace')
  return (
    <Popover open={open} onOpenChange={setOpen}>
      <PopoverTrigger asChild>
        <button
          type="button"
          data-workbench-chip="options"
          data-open={open ? 'true' : undefined}
          aria-expanded={open}
          aria-haspopup="dialog"
          aria-label={t('workbench.composer.options')}
          className="grid size-[30px] place-items-center rounded-[8px] bg-[#f5f5f5] text-[#8a8a8a] outline-none hover:bg-[#ececec] hover:text-[#171717] focus-visible:ring-2 focus-visible:ring-black/10 data-[state=open]:bg-[#ececec] data-[state=open]:text-[#171717]">
          <Plus
            aria-hidden="true"
            size={16}
            strokeWidth={1.7}
            className={cn('transition-transform motion-reduce:transition-none', open && 'rotate-90')}
          />
        </button>
      </PopoverTrigger>
      <PopoverContent
        side="top"
        align="start"
        sideOffset={8}
        aria-label={t('workbench.composer.options')}
        data-workbench-menu="options"
        className="w-56 rounded-[12px] border border-[#ececec] bg-white p-1.5 shadow-[0_12px_32px_rgba(0,0,0,0.12)]">
        <div data-composer-options-list="" className="grid">
          <button
            type="button"
            aria-haspopup="dialog"
            data-composer-option-kind="upload"
            disabled={!canUpload}
            title={typeof attachment?.tooltip === 'string' ? attachment.tooltip : undefined}
            className="flex min-h-10 w-full items-center gap-2 rounded-[8px] px-2 py-1.5 text-left text-[12px] leading-[18px] font-medium text-[#171717] hover:bg-[#f7f7f7] disabled:cursor-default disabled:opacity-50"
            onClick={() => {
              if (!canUpload || !attachment?.action || !quickPanel) return
              setOpen(false)
              attachment.action({ source: 'popover', inputAdapter, quickPanel })
            }}>
            <span
              aria-hidden="true"
              className="grid size-5 shrink-0 place-items-center rounded-[6px] bg-[#f5f5f5] text-[#8a8a8a]">
              <Paperclip size={14} strokeWidth={1.7} />
            </span>
            <span>{uploadLabel}</span>
            {typeof attachment?.suffix === 'string' ? (
              <span className="ml-auto text-[#8a8a8a]">{attachment.suffix}</span>
            ) : null}
          </button>
          <ComposerOptionSwitch
            kind="review"
            icon={<Shield aria-hidden="true" size={14} strokeWidth={1.7} />}
            label={review}
            checked={options.reviewMode}
            divider
            onChange={(reviewMode) => onChange({ ...options, reviewMode })}
          />
          <ComposerOptionSwitch
            kind="artifact"
            icon={<Link2 aria-hidden="true" size={14} strokeWidth={1.7} />}
            label={trace}
            checked={options.artifactTrace}
            divider
            onChange={(artifactTrace) => onChange({ ...options, artifactTrace })}
          />
        </div>
      </PopoverContent>
    </Popover>
  )
}

function ComposerOptionSwitch({
  kind,
  icon,
  label,
  checked,
  divider = false,
  onChange
}: {
  kind: 'review' | 'artifact'
  icon: ReactNode
  label: string
  checked: boolean
  divider?: boolean
  onChange: (checked: boolean) => void
}) {
  return (
    <label
      data-composer-option-row=""
      className={cn(
        'flex min-h-10 cursor-pointer items-center justify-between gap-4 rounded-[8px] px-2 py-1.5 text-[12px] leading-[18px] font-medium text-[#171717] focus-within:bg-[#f7f7f7] hover:bg-[#f7f7f7]',
        divider && 'border-t border-[#ececec]'
      )}>
      <span className="inline-flex min-w-0 items-center gap-2">
        <span
          aria-hidden="true"
          data-composer-option-kind={kind}
          className="grid size-5 shrink-0 place-items-center rounded-[6px] bg-[#f5f5f5] text-[#8a8a8a]">
          {icon}
        </span>
        <span>{label}</span>
      </span>
      <span className="relative inline-flex h-5 w-9 shrink-0">
        <input
          aria-label={label}
          checked={checked}
          className="peer absolute inset-0 z-10 m-0 cursor-pointer opacity-0"
          role="switch"
          type="checkbox"
          onChange={(event) => onChange(event.currentTarget.checked)}
        />
        <span
          aria-hidden="true"
          className={cn(
            'absolute inset-0 rounded-full shadow-[inset_0_0_0_1px_rgba(23,23,23,0.12)] transition-colors peer-focus-visible:outline-2 peer-focus-visible:outline-offset-2 peer-focus-visible:outline-[#3867d6] motion-reduce:transition-none',
            checked ? 'bg-[var(--ui-blue,#3867d6)]' : 'bg-[#d4d4d4]'
          )}>
          <span
            className={cn(
              'absolute top-[3px] left-[3px] size-3.5 rounded-full bg-white shadow-[0_1px_2px_rgba(15,23,42,0.18)] transition-transform motion-reduce:transition-none',
              checked && 'translate-x-4'
            )}
          />
        </span>
      </span>
    </label>
  )
}

function findCatalogModel(items: readonly OrgModel[], model: Model | undefined): OrgModel | undefined {
  if (!model) return undefined
  const alias = model.apiModelId || model.id
  return items.find((item) => item.id === alias || item.id === model.id)
}

export function useWorkbenchHomeComposer({
  enabled,
  fileCount,
  model,
  canChangeModel = false,
  onModelSelect,
  modelFilter,
  isModelDisabled,
  workspace,
  workspaceId,
  onWorkspaceChange,
  workspaceChanging = false,
  onPermissionModeChange,
  onModelUnavailable,
  catalogModelPicker = false,
  reasoningEffort,
  onReasoningEffortChange,
  text,
  tokens = [],
  onTextChange,
  projectLocked = false,
  sessionId = null,
  scopeElement = null,
  appWorkspacePath,
  workspaceFileSelection
}: WorkbenchHomeComposerArgs) {
  const host = use(WorkbenchComposerHostContext)
  const { t } = useTranslation()
  const { presets } = useAssistantCatalogPresets({ enabled })
  const { assistants } = useAssistantsApi({ enabled })
  const orgCatalog = useOrgModels({ enabled: enabled && catalogModelPicker })
  const localSkills = useHomeLocalSkills(enabled)
  const rootPanelItems = useMemo(() => (enabled ? homeSlashItems(t, localSkills) : []), [enabled, localSkills, t])
  const { options, changeOptions } = useHomeComposerOptions()
  const [pinnedIds, setPinnedIds] = useState<string[]>(() => [...HOME_COMPOSER_PINNED_TOOL_IDS])
  const inputAdapterRef = useRef<QuickPanelInputAdapter | null>(null)
  const [inputbar, setInputbar] = useState<HTMLElement | null>(null)
  const [scopeSlot, setScopeSlot] = useState<HTMLElement | null>(null)
  const memberId = useMemberProfile()?.id ?? null
  const permissionRevision = useSyncExternalStore(subscribeTaskPermission, readTaskPermissionRevision, () => 0)
  const permissionSessionId = sessionId ?? 'draft'
  const shownPermissionMode = useMemo(
    () => displayedTaskPermissionMode(permissionSessionId, memberId),
    [memberId, permissionRevision, permissionSessionId]
  )
  const projectSelection = useWorkbenchProjectSelection({
    workspace,
    workspaceId,
    onWorkspaceChange,
    enabled,
    locked: projectLocked
  })
  const { setPickerOpen: setProjectPickerOpen } = projectSelection
  // Deferred a frame so the closing mention panel's focus restore does not dismiss the picker.
  const openProjectPicker = useCallback(() => {
    if (projectLocked) return
    window.requestAnimationFrame(() => setProjectPickerOpen(true))
  }, [projectLocked, setProjectPickerOpen])

  const setQuickPanelFill = useOptionalQuickPanel()?.setFillToAvailableHeight
  useLayoutEffect(() => {
    if (!enabled || !host || !setQuickPanelFill) return
    setQuickPanelFill(true)
    return () => setQuickPanelFill(false)
  }, [enabled, host, setQuickPanelFill])

  const changePermissionMode = useCallback(
    (mode: string) => {
      const next = storedPermissionMode(mode)
      if (!next) return
      writeTaskPermission(permissionSessionId, { mode: next }, undefined, memberId)
      if (permissionSessionId === 'draft') writePendingPermissionMode(next, undefined, memberId)
      onPermissionModeChange?.(next)
    },
    [memberId, onPermissionModeChange, permissionSessionId]
  )

  useEffect(() => {
    if (!enabled) {
      setInputbar(null)
      setScopeSlot(null)
      return
    }
    if (scopeElement) {
      setScopeSlot(scopeElement)
      const frame = scopeElement.parentElement
      const findInput = () => setInputbar(frame?.querySelector<HTMLElement>('[data-composer-inputbar]') ?? null)
      findInput()
      if (!frame) return
      const observer = new MutationObserver(findInput)
      observer.observe(frame, { childList: true, subtree: true })
      return () => observer.disconnect()
    }
    if (!host) {
      setInputbar(null)
      setScopeSlot(null)
      return
    }
    const find = () => {
      setInputbar(host.querySelector<HTMLElement>('[data-composer-inputbar]'))
      setScopeSlot(findWorkbenchComposerScope(host))
    }
    find()
    const observer = new MutationObserver(find)
    observer.observe(host, { childList: true, subtree: true })
    const scopeRoot = host.parentElement?.parentElement
    if (scopeRoot) observer.observe(scopeRoot, { childList: true })
    return () => observer.disconnect()
  }, [enabled, host, scopeElement])

  const installedAgents = useMemo(() => installedMentionAgents(presets, assistants), [assistants, presets])
  rememberAgentAvatars(installedAgents)

  const suggestionSource = useMemo<ComposerSuggestionSource | null>(() => {
    if (!enabled) return null
    return {
      pluginKey: 'workbench-agent-suggestion',
      char: '@',
      title: t('workbench.composer.agents_title', '智能体'),
      allowedPrefixes: [...ROOT_QUICK_PANEL_ALLOWED_PREFIXES],
      items: ({ query }) => homeAgentSuggestionItems(t, installedAgents, query)
    }
  }, [enabled, installedAgents, t])

  const goal = composerGoalSummary(text)
  const sessionMessages = useQuery('/agent-sessions/:sessionId/messages', {
    params: { sessionId: sessionId ?? '' },
    query: { limit: 200 },
    enabled: Boolean(enabled && projectLocked && sessionId)
  })
  const sessionGoal = useMemo(() => {
    const texts = userTextsFromSessionMessages(sessionMessages.data?.items)
    return sessionGoalFromUserTexts([...texts].reverse())
  }, [sessionMessages.data?.items])
  const taskGoal = projectLocked ? taskGoalPresentation(composerGoalObjective(text), sessionGoal) : null
  const modelAlias = model?.apiModelId || model?.id || ''
  const catalogModel = useMemo(
    () => findCatalogModel(orgCatalog.catalog.items, model),
    [model, orgCatalog.catalog.items]
  )
  const logoId = catalogModel ? orgModelLogoId(catalogModel) : undefined
  const publishedEfforts = catalogModel?.reasoningEfforts
  const shownEffort = visibleReasoningEffort(modelAlias, reasoningEffort ?? 'default', publishedEfforts)
  const effort = composerReasoningEffort(modelAlias, reasoningEffort ?? 'default', publishedEfforts)
  const effortLabel = effort ? t(reasoningEffortLabelKey(effort)) : null
  const [defaultModelId] = usePreference('chat.default_model_id')
  const autoModel = Boolean(catalogModelPicker && !defaultModelId)
  const modelLabel = autoModel
    ? t('workbench.composer.auto_model')
    : model?.name || t('workbench.composer.choose_model')

  const modelButton = (
    <button
      type="button"
      data-workbench-chip="model"
      data-model-auto={autoModel ? 'true' : undefined}
      data-provider-logo={autoModel ? undefined : logoId}
      data-reasoning-effort={shownEffort ?? undefined}
      title={effortLabel ? `${modelLabel} · ${t('workbench.model_picker.effort')} ${effortLabel}` : modelLabel}
      className={CHIP_CLASS}
      onClick={catalogModelPicker || (model && canChangeModel) ? undefined : () => onModelUnavailable?.()}>
      {autoModel ? (
        <span
          className="inline-flex size-4 shrink-0 items-center justify-center self-center text-[#171717]"
          data-model-icon="auto">
          <Sparkles size={16} strokeWidth={1.7} aria-hidden />
        </span>
      ) : logoId ? (
        <span className="inline-flex size-4 shrink-0 items-center justify-center self-center">
          <ProviderAvatar provider={{ id: logoId, name: catalogModel?.brand || logoId }} size={16} className="size-4" />
        </span>
      ) : null}
      <span className="inline-flex h-4 max-w-44 min-w-0 items-center self-center">
        <span className="min-w-0 truncate leading-none">{modelLabel}</span>
      </span>
      {effort && effortLabel ? (
        <span
          data-workbench-chip="effort"
          className="inline-flex h-4 shrink-0 items-center self-center leading-none font-medium"
          style={{ color: reasoningEffortColor(effort) }}>
          {effortLabel}
        </span>
      ) : null}
      <ChevronDown size={14} strokeWidth={1.7} className="shrink-0 self-center" />
    </button>
  )

  const modelControl = catalogModelPicker ? (
    <WorkbenchModelPicker
      model={model}
      trigger={modelButton}
      onSelect={onModelSelect}
      reasoningEffort={reasoningEffort}
      onReasoningEffortChange={onReasoningEffortChange}
    />
  ) : model && canChangeModel && onModelSelect ? (
    <ModelSelector
      multiple={false}
      value={model}
      onSelect={onModelSelect}
      filter={modelFilter}
      isModelDisabled={isModelDisabled}
      trigger={modelButton}
    />
  ) : (
    modelButton
  )

  const renderLayout = useCallback(
    ({ inputAdapter }: ToolContext) => {
      if (!enabled) return null
      inputAdapterRef.current = inputAdapter ?? null
      return (
        <div data-workbench-home-tools="true" className="flex shrink-0 items-center gap-0.5">
          <WorkbenchComposerOptions options={options} onChange={changeOptions} inputAdapter={inputAdapter} />
          <WorkbenchPermissionPicker
            permissionMode={shownPermissionMode}
            onPermissionModeChange={changePermissionMode}
          />
          <CodeWorkspacePlacement projectPath={projectSelection.selectedPath || null} />
        </div>
      )
    },
    [changeOptions, changePermissionMode, enabled, options, projectSelection.selectedPath, shownPermissionMode]
  )

  const accessory = enabled ? modelControl : null

  const chrome = enabled ? (
    <>
      {inputbar && reviewDraftNeedsSource({ text, tokens: [...tokens] })
        ? createPortal(<ReviewComposerSource adapter={() => inputAdapterRef.current} />, inputbar)
        : null}
      {scopeSlot
        ? createPortal(
            <WorkbenchProjectTray
              selection={projectSelection}
              workspaceChanging={workspaceChanging}
              locked={projectLocked}
              goalPaused={Boolean(projectLocked && taskGoal?.paused)}
              goalClearable={projectLocked ? Boolean(taskGoal?.clearable) : true}
              goal={projectLocked ? (taskGoal?.text ?? null) : goal}
              onClearGoal={() => onTextChange(withoutComposerGoal(text))}
              files={
                workspaceFileSelection
                  ? {
                      root: fileMentionRoot(
                        projectSelection.selectedPath || (workspace?.type === 'user' ? workspace.path : undefined),
                        appWorkspacePath
                      ),
                      selectedPaths: workspaceFileSelection.selectedPaths,
                      onCommit: workspaceFileSelection.onCommit
                    }
                  : undefined
              }
            />,
            scopeSlot
          )
        : null}
      {inputbar && options.researchMode && fileCount > 0
        ? createPortal(
            <p role="alert" data-research-attachment-warning="true" className="px-4 pt-3 text-[13px] text-[#9a3412]">
              {t('workbench.composer.research_warning', '深度研究不接受任务资源；移除资源后才能发送。')}
            </p>,
            inputbar
          )
        : null}
    </>
  ) : null

  return {
    options,
    pinnedIds,
    setPinnedIds,
    resetPinnedIds: () => setPinnedIds([...HOME_COMPOSER_PINNED_TOOL_IDS]),
    pinsAtDefault: sameToolOrder(pinnedIds, HOME_COMPOSER_PINNED_TOOL_IDS),
    suggestionSource,
    rootPanelItems,
    projectPath: enabled ? projectSelection.selectedPath : null,
    openProjectPicker,
    renderLayout,
    accessory,
    chrome
  }
}
