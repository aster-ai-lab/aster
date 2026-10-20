import { Kbd, Textarea } from '@a3s-lab/ui'
import { Loader2 } from 'lucide-react'
import { useCallback, useRef, useState } from 'react'
import { useHotkeys } from 'react-hotkeys-hook'
import { useTranslation } from 'react-i18next'

import { loggerService } from '@logger'
import { getToolGroupIcon } from '@renderer/components/chat/messages/blocks/ToolBlockGroup'
import { isValidAgentToolsType, renderTool, UnknownToolRenderer } from '@renderer/components/chat/messages/tools/agent'
import { AgentToolsType } from '@renderer/components/chat/messages/tools/shared/agentToolTypes'
import { ToolArgsTable } from '@renderer/components/chat/messages/tools/shared/ArgsTable'
import { ToolDisclosure, type ToolDisclosureItem } from '@renderer/components/chat/messages/tools/shared/ToolDisclosure'
import { TOOL_HEADER_UI } from '@renderer/components/chat/messages/tools/ToolHeader'
import type { ToolResponseLike } from '@renderer/components/chat/messages/tools/toolResponse'
import type { MessageToolApprovalInput } from '@renderer/components/chat/messages/types'
import Scrollbar from '@renderer/components/Scrollbar'
import { toast } from '@renderer/services/toast'
import type { McpToolResponse, NormalToolResponse } from '@renderer/types/mcpTool'
import { commandFromArgs, commandInText, isProseToolLabel } from '@renderer/utils/permissionPrompt'
import { cn } from '@renderer/utils/style'

import type { ComposerOverride } from '../ComposerContext'
import type { PermissionRequestComposerRequest } from './permissionRequestComposerRequest'

export type { PermissionRequestComposerRequest } from './permissionRequestComposerRequest'
export { findNextPendingPermissionRequest } from './permissionRequestComposerRequest'

const logger = loggerService.withContext('PermissionRequestComposer')

type PermissionRequestComposerProps = {
  request: PermissionRequestComposerRequest
  onRespond: (input: MessageToolApprovalInput) => void | Promise<void>
  allowRejectionReason?: boolean
  className?: string
}

type PermissionRequestComposerOverrideOptions = {
  request: PermissionRequestComposerRequest
  onRespond: (input: MessageToolApprovalInput) => void | Promise<void>
  allowRejectionReason?: boolean
}

function isMcpToolResponse(toolResponse: ToolResponseLike): toolResponse is McpToolResponse {
  return toolResponse.tool.type === 'mcp'
}

function normalizeArgs(args: ToolResponseLike['arguments']): Record<string, unknown> | unknown[] | null {
  if (args === undefined || args === null) return null
  if (typeof args === 'object') return args
  return { value: args }
}

const BUILTIN_TOOLS_WITH_OWN_PREVIEW_SCROLL = new Set<string>([
  AgentToolsType.Bash,
  AgentToolsType.BashOutput,
  AgentToolsType.Glob,
  AgentToolsType.Grep,
  AgentToolsType.Read,
  AgentToolsType.Skill,
  AgentToolsType.Write
])

function renderBuiltinPreviewChildren(toolName: string, children: ToolDisclosureItem['children']) {
  if (children === undefined || children === null || BUILTIN_TOOLS_WITH_OWN_PREVIEW_SCROLL.has(toolName)) {
    return children
  }

  return (
    <Scrollbar className="max-h-60 overflow-x-hidden" data-testid="permission-builtin-body-scroll">
      {children}
    </Scrollbar>
  )
}

export function createPermissionRequestComposerOverride({
  request,
  onRespond,
  allowRejectionReason = true
}: PermissionRequestComposerOverrideOptions): ComposerOverride {
  return {
    id: `tool-permission:${request.approvalId}`,
    priority: 90,
    render: ({ className }) => (
      <PermissionRequestComposer
        request={request}
        onRespond={onRespond}
        className={className}
        allowRejectionReason={allowRejectionReason}
      />
    )
  }
}

const SKILL_RUN_FIELDS = ['args', 'prompt', 'question', 'message'] as const

function readTrimmedString(value: unknown, fields: readonly string[]): string | undefined {
  if (!value || typeof value !== 'object' || Array.isArray(value)) return undefined

  const record = value as Record<string, unknown>
  for (const field of fields) {
    const candidate = record[field]
    if (typeof candidate === 'string' && candidate.trim()) return candidate.trim()
  }

  return undefined
}

function permissionDecisionTarget(toolResponse: ToolResponseLike): string {
  const toolName = toolResponse.tool.name.trim()
  if (toolName === AgentToolsType.Skill) {
    return readTrimmedString(toolResponse.arguments, ['skill', 'name']) ?? toolName
  }

  return toolName
}

function skillRunText(args: unknown): string | undefined {
  return readTrimmedString(args, SKILL_RUN_FIELDS)
}

function BuiltinPermissionPreview({ toolResponse }: { toolResponse: NormalToolResponse }) {
  const toolName = toolResponse.tool.name
  const input = toolResponse.arguments as Record<string, unknown> | string | undefined

  if (toolName === AgentToolsType.Skill) {
    const runText = skillRunText(input)
    return (
      <div className="px-3 py-2">
        <PermissionPreviewHeader toolName={permissionDecisionTarget(toolResponse)} />
        {runText ? (
          <pre
            data-testid="permission-skill-run"
            className="mt-1 max-h-40 overflow-y-auto font-mono text-[12px] leading-5 break-all whitespace-pre-wrap text-[#3f3f3f]">
            {runText}
          </pre>
        ) : null}
      </div>
    )
  }

  const renderedItem = isValidAgentToolsType(toolName)
    ? renderTool(toolName, input)
    : UnknownToolRenderer({ toolName, input })

  const item: ToolDisclosureItem = {
    ...renderedItem,
    label: renderedItem.label ?? <PermissionPreviewHeader toolName={toolName} />,
    children: renderBuiltinPreviewChildren(toolName, renderedItem.children),
    classNames: {
      ...renderedItem.classNames,
      header: cn('px-3 py-2', renderedItem.classNames?.header),
      body: cn('max-h-none overflow-visible bg-transparent p-2 text-foreground', renderedItem.classNames?.body)
    }
  }

  return (
    <ToolDisclosure
      className="w-full"
      variant="light"
      defaultActiveKey={[String(renderedItem.key ?? toolName)]}
      items={[item]}
    />
  )
}

function McpPermissionPreview({ toolResponse }: { toolResponse: McpToolResponse }) {
  const { t } = useTranslation()
  const args = normalizeArgs(toolResponse.arguments)

  return (
    <div className="px-3 py-2">
      <PermissionPreviewHeader toolName={toolResponse.tool.name} description={toolResponse.tool.description} />
      {args ? (
        <Scrollbar className="max-h-60 overflow-x-hidden" data-testid="permission-mcp-args-scroll">
          <ToolArgsTable args={args} title={t('message.tools.sections.input')} />
        </Scrollbar>
      ) : (
        <div className="py-2 text-xs text-muted-foreground">{t('message.tools.noData')}</div>
      )}
    </div>
  )
}

function PermissionPreview({ toolResponse }: { toolResponse: ToolResponseLike }) {
  if (isMcpToolResponse(toolResponse)) {
    return <McpPermissionPreview toolResponse={toolResponse} />
  }

  return <BuiltinPermissionPreview toolResponse={toolResponse} />
}

function getPermissionRequestSubtitle(request: PermissionRequestComposerRequest, target: string): string | null {
  const title = request.title.trim()
  const toolName = request.toolResponse.tool.name.trim()

  if (!title || title === toolName || title === target) return null
  if (toolName === AgentToolsType.Skill && title === skillRunText(request.toolResponse.arguments)) return null
  return title
}

function PermissionPreviewHeader({ toolName, description }: { toolName: string; description?: string }) {
  return (
    <div className="min-w-0 text-sm text-foreground">
      <div className="truncate font-medium">{toolName}</div>
      {description ? (
        <div className="mt-0.5 line-clamp-2 text-xs leading-4 text-muted-foreground">{description}</div>
      ) : null}
    </div>
  )
}

export default function PermissionRequestComposer({
  request,
  onRespond,
  className,
  allowRejectionReason = true
}: PermissionRequestComposerProps) {
  const { t } = useTranslation()
  const composingApprovalIdRef = useRef<string | null>(null)
  const isHandledElsewhere = useCallback(
    (event: KeyboardEvent) =>
      event.defaultPrevented ||
      event.isComposing ||
      event.keyCode === 229 ||
      composingApprovalIdRef.current === request.approvalId,
    [request.approvalId]
  )
  const [submission, setSubmission] = useState<{ approvalId: string; action: 'approve' | 'deny' } | null>(null)
  const [noteDraft, setNoteDraft] = useState({ approvalId: request.approvalId, value: '', open: false })
  const isSubmitting = submission?.approvalId === request.approvalId
  const noteForRequest = noteDraft.approvalId === request.approvalId
  const rejectionReason = allowRejectionReason && noteForRequest ? noteDraft.value : ''
  const noteOpen = allowRejectionReason && noteForRequest && noteDraft.open
  // A typed reason means the user is denying. Enter must not approve behind their back.
  const hasRejectionReason = rejectionReason.trim().length > 0
  const toolName = request.toolResponse.tool.name.trim()
  const prose = isProseToolLabel(toolName)
  const command = prose
    ? (commandFromArgs(request.toolResponse.arguments) ?? commandInText(toolName) ?? commandInText(request.title))
    : undefined
  const target = permissionDecisionTarget(request.toolResponse)
  const targetLabelKey = toolName === AgentToolsType.Skill ? undefined : TOOL_HEADER_UI[toolName]?.labelKey
  const targetLabel = targetLabelKey ? t(targetLabelKey) : target
  const subtitle = prose ? null : getPermissionRequestSubtitle(request, target)
  const ToolIcon = getToolGroupIcon(request.toolResponse.tool, request.toolResponse.arguments)
  const decisionTitle = prose
    ? t(command ? 'agent.toolPermission.runCommand' : 'agent.toolPermission.confirmAction')
    : t('agent.toolPermission.decisionTitle', { target: targetLabel })
  const proseDetail = command ?? (prose ? toolName.replace(/\s*[?？]\s*$/, '').trim() : '')

  const respond = useCallback(
    async (input: MessageToolApprovalInput, action: 'approve' | 'deny') => {
      const approvalId = request.approvalId
      setSubmission({ approvalId, action })
      try {
        await onRespond(input)
      } catch (error) {
        logger.error('Failed to send permission response', error as Error, {
          action,
          approvalId
        })
        toast.error(t('agent.toolPermission.error.sendFailed'))
        setSubmission((current) => (current?.approvalId === approvalId ? null : current))
      }
    },
    [onRespond, request.approvalId, t]
  )

  const approve = useCallback(async () => {
    if (isSubmitting) return
    await respond(
      {
        match: request.match,
        approved: true
      },
      'approve'
    )
  }, [isSubmitting, request.match, respond])

  const deny = useCallback(async () => {
    if (isSubmitting) return
    const reason = rejectionReason.trim() || t('agent.toolPermission.defaultDenyMessage')
    await respond(
      {
        match: request.match,
        approved: false,
        ...(allowRejectionReason ? { reason } : {})
      },
      'deny'
    )
  }, [allowRejectionReason, isSubmitting, rejectionReason, request.match, respond, t])

  useHotkeys(
    'enter',
    () => void (hasRejectionReason ? deny() : approve()),
    { preventDefault: true, ignoreEventWhen: isHandledElsewhere },
    [approve, deny, hasRejectionReason]
  )
  useHotkeys('esc', () => void deny(), { preventDefault: true, ignoreEventWhen: isHandledElsewhere }, [deny])

  return (
    <div
      data-composer-viewport-inset-target=""
      // pointer-events-auto: the composer dock stack is click-through; override
      // composers re-enable interaction on their own root.
      className={cn('pointer-events-auto relative z-2 flex flex-col px-4.5 pt-0 pb-4.5', className)}>
      <div className="rounded-[14px] border border-[#ececec] bg-white p-3 text-[#171717] shadow-[0_8px_22px_rgba(20,40,80,0.04)]">
        <div className="flex min-w-0 items-center gap-2 px-1">
          <h2 className="flex shrink-0 items-center gap-2 text-[14px] leading-5 font-medium text-[#171717]">
            <span className="inline-flex shrink-0 text-[#3f3f3f]">
              <ToolIcon aria-hidden="true" className="size-4" strokeWidth={1.7} />
            </span>
            {decisionTitle}
          </h2>
          {subtitle ? (
            <span title={subtitle} className="min-w-0 truncate text-[12px] text-[#8a8a8a]">
              {subtitle}
            </span>
          ) : null}
        </div>

        <div className="mt-2 overflow-hidden rounded-[8px] bg-[#f5f5f5]" data-testid="permission-preview">
          {prose ? (
            <pre
              data-testid="permission-command"
              className="px-3 py-2 font-mono text-[13px] leading-5 break-all whitespace-pre-wrap text-[#171717]">
              {proseDetail}
            </pre>
          ) : (
            <PermissionPreview toolResponse={request.toolResponse} />
          )}
        </div>

        {noteOpen ? (
          <label className="mt-2.5 block px-1 text-[12px] text-[#8a8a8a]">
            <span>{t('agent.toolPermission.reasonLabel')}</span>
            <Textarea.Input
              value={rejectionReason}
              disabled={isSubmitting}
              maxLength={500}
              rows={2}
              autoFocus
              aria-label={t('agent.toolPermission.reasonLabel')}
              placeholder={t('agent.toolPermission.reasonPlaceholder')}
              className="mt-1 min-h-14 resize-none rounded-[8px] border-0 bg-[#f5f5f5] px-3 py-2 text-[13px] text-[#171717] placeholder:text-[#c5c5c5]"
              onValueChange={(value) => setNoteDraft({ approvalId: request.approvalId, value, open: true })}
              onCompositionStart={() => {
                composingApprovalIdRef.current = request.approvalId
              }}
              onCompositionEnd={() => {
                composingApprovalIdRef.current = null
              }}
              onBlur={() => {
                composingApprovalIdRef.current = null
              }}
              onKeyDown={(event) => {
                if (isHandledElsewhere(event.nativeEvent)) return
                // The global Esc hotkey ignores form fields, so deny from the note itself.
                if (event.key === 'Escape') {
                  event.preventDefault()
                  void deny()
                  return
                }
                if (event.key !== 'Enter' || event.shiftKey) return
                event.preventDefault()
                void (hasRejectionReason ? deny() : approve())
              }}
            />
          </label>
        ) : null}

        <div className="mt-2.5 flex items-center justify-end gap-2 px-1 pb-0.5">
          {allowRejectionReason && !isSubmitting && !noteOpen ? (
            <button
              type="button"
              onClick={() => setNoteDraft({ approvalId: request.approvalId, value: rejectionReason, open: true })}
              className="mr-auto inline-flex h-9 items-center rounded-[8px] px-2 text-[13px] text-[#8a8a8a] hover:bg-[#fafafa] hover:text-[#171717] focus-visible:ring-2 focus-visible:ring-black/10">
              {t('agent.toolPermission.addNote')}
            </button>
          ) : null}
          {/* Live region stays mounted while idle so the sending state is announced. */}
          <div
            role="status"
            aria-live="polite"
            className={cn(isSubmitting ? 'inline-flex items-center gap-1 text-[12px] text-[#8a8a8a]' : 'sr-only')}>
            {isSubmitting ? (
              <>
                <Loader2 aria-hidden="true" className="size-3.5 animate-spin" strokeWidth={1.7} />
                {t(
                  submission?.action === 'deny'
                    ? 'agent.toolPermission.sendingDeny'
                    : 'agent.toolPermission.sendingAllow'
                )}
              </>
            ) : null}
          </div>
          {isSubmitting ? null : (
            <>
              <button
                type="button"
                onClick={() => void deny()}
                className="inline-flex h-9 items-center gap-2 rounded-[8px] border border-[#e5e5e5] bg-white px-3 text-[13px] text-[#171717] hover:bg-[#fafafa] focus-visible:ring-2 focus-visible:ring-black/10">
                {t('agent.toolPermission.button.deny')}
                <Kbd aria-hidden="true" className="bg-[#f5f5f5] text-[#8a8a8a]">
                  {hasRejectionReason ? 'Enter' : 'Esc'}
                </Kbd>
              </button>
              <button
                type="button"
                onClick={() => void approve()}
                className="inline-flex h-9 items-center gap-2 rounded-[8px] bg-[#171717] px-3 text-[13px] text-white focus-visible:ring-2 focus-visible:ring-black/10">
                {t('agent.toolPermission.button.allow')}
                {!hasRejectionReason && (
                  <Kbd aria-hidden="true" className="bg-white/15 text-white">
                    Enter
                  </Kbd>
                )}
              </button>
            </>
          )}
        </div>
      </div>
    </div>
  )
}
