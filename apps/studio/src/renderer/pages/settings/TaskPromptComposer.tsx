import { cn } from '@a3s-lab/ui/lib/utils'
import type { FC, ReactNode } from 'react'
import { useMemo, useRef } from 'react'
import { useTranslation } from 'react-i18next'

import { ComposerToolRuntimeProvider } from '@renderer/components/composer/ComposerToolRuntime'
import { HomeComposerInput } from '@renderer/components/composer/HomeComposerInput'
import { QuickPanelProvider } from '@renderer/components/QuickPanel'
import { homeComposerTextHighlights, homeSlashItems, useHomeLocalSkills } from '@renderer/pages/agents/homeSlashCatalog'
import type { ComposerShortcut } from '@shared/data/preference/preferenceTypes'

import '@renderer/pages/agents/workbenchComposer.css'
import './taskPromptComposer.css'

/** Four lines at 13px. The field stays this tall so the schedule below does not jump. */
const TASK_PROMPT_EDITOR_MIN_HEIGHT = 96
const TASK_PROMPT_NEWLINE: ComposerShortcut = ['Enter']
/** Send stays disabled. This chord is not Enter and not ⌘/Ctrl+Enter, which saves the dialog. */
const TASK_PROMPT_SEND: ComposerShortcut = ['Alt', 'Enter']
/** The task form must not offer the homepage goal command. */
const TASK_OMIT_COMMANDS = ['/goal']

/** Homepage editor for the task form. Enter inserts a line; the dialog keeps ⌘/Ctrl+Enter. */
export const TaskPromptComposer: FC<{
  prompt: string
  onPromptChange: (value: string) => void
  disabled?: boolean
  invalid?: boolean
  label: string
  toolbar?: ReactNode
}> = ({ prompt, onPromptChange, disabled, invalid, label, toolbar }) => {
  const { t } = useTranslation()
  const localSkills = useHomeLocalSkills(true)
  const slashItems = useMemo(
    () => homeSlashItems(t, localSkills, { includeLaunchers: false, omitCommands: TASK_OMIT_COMMANDS }),
    [localSkills, t]
  )
  const textHighlights = useMemo(() => homeComposerTextHighlights({ omitCommands: TASK_OMIT_COMMANDS }), [])
  const promptRef = useRef(prompt)
  promptRef.current = prompt
  const onPromptChangeRef = useRef(onPromptChange)
  onPromptChangeRef.current = onPromptChange
  const toolActions = useMemo(
    () => ({
      addNewTopic: () => undefined,
      onTextChange: (updater: string | ((previous: string) => string)) => {
        const next = typeof updater === 'function' ? updater(promptRef.current) : updater
        onPromptChangeRef.current(next)
      }
    }),
    []
  )
  const toolState = useMemo(
    () => ({
      mentionedModels: [],
      selectedKnowledgeBases: [],
      files: [],
      isExpanded: false,
      couldAddImageFile: false,
      extensions: [] as string[]
    }),
    []
  )
  return (
    <div
      data-ui="task.prompt.composer workbench.composer"
      data-invalid={invalid || undefined}
      aria-label={label}
      className={cn(
        'w-full min-w-0',
        '[&_[data-composer-toolbar]]:h-auto [&_[data-composer-toolbar]]:min-h-10 [&_[data-composer-toolbar]]:flex-wrap [&_[data-composer-toolbar]]:gap-1',
        '[&_[data-composer-toolbar]>div:first-child]:min-w-0 [&_[data-composer-toolbar]>div:first-child]:flex-wrap [&_[data-composer-toolbar]>div:first-child]:overflow-visible',
        invalid && '[&_[data-composer-inputbar]]:border-[#b42318]'
      )}>
      <QuickPanelProvider>
        <ComposerToolRuntimeProvider initialState={toolState} actions={toolActions}>
          <HomeComposerInput
            showScreenshot={false}
            text={prompt}
            onTextChange={onPromptChange}
            tokens={[]}
            managedTokenKinds={[]}
            onTokensChange={() => undefined}
            placeholder={t('agent.tasks.prompt.placeholder')}
            sendMessageShortcut={TASK_PROMPT_SEND}
            newlineShortcut={TASK_PROMPT_NEWLINE}
            sendDisabled
            hideSend
            isLoading={false}
            onSendDraft={() => undefined}
            onPause={() => undefined}
            supportedExts={[]}
            setFiles={() => undefined}
            filesCount={0}
            isExpanded={false}
            onExpandedChange={() => undefined}
            quickPanelEnabled
            enableDragDrop={false}
            enableSpellCheck={false}
            editable={!disabled}
            fontSize={13}
            editorMinHeight={TASK_PROMPT_EDITOR_MIN_HEIGHT}
            lockEditorHeight
            narrowMode={false}
            rootPanelCatalogOnly
            rootPanelAdditionalItems={slashItems}
            textHighlights={textHighlights}
            renderLeftControls={() => toolbar}
          />
        </ComposerToolRuntimeProvider>
      </QuickPanelProvider>
    </div>
  )
}
