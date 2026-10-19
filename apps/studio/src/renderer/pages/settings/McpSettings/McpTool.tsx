import '@a3s-lab/ui/components/composites/markdown/styles'
import type { ColumnDef } from '@a3s-lab/ui'
import { Badge, ColFlex, DataTable, Flex, InfoTooltip, Markdown, RequiredMark, Switch, Tooltip } from '@a3s-lab/ui'
import { Zap } from 'lucide-react'
import type { Key } from 'react'
import { useCallback, useMemo, useState } from 'react'
import { useTranslation } from 'react-i18next'

import { beginOptimistic, useOptimisticRows } from '@renderer/components/collection/optimisticRows'
import { McpLogo } from '@renderer/components/icons/SvgIcon'
import { useIsToolAutoApproved } from '@renderer/hooks/useMcpServer'
import { toast } from '@renderer/services/toast'
import type { McpTool } from '@renderer/types/tool'
import type { McpServer } from '@shared/data/types/mcpServer'

interface McpToolsSectionProps {
  tools: McpTool[]
  server: McpServer
  searchText: string
  onToggleTool: (tool: McpTool, enabled: boolean) => Promise<void>
  onToggleAutoApprove: (tool: McpTool, autoApprove: boolean) => Promise<void>
}

type ToolOverride = { enabled?: boolean; autoApprove?: boolean }

const MAX_NESTING_DEPTH = 5
const NO_OVERRIDES: ReadonlyMap<string, ToolOverride> = new Map()
const toolId = (tool: McpTool) => tool.id

interface SchemaProperty {
  type?: string
  description?: string
  enum?: Array<string | number | boolean | null>
  properties?: Record<string, SchemaProperty>
  required?: string[]
  items?: SchemaProperty
}

type SchemaProperties = Record<string, SchemaProperty>

const getToolSchemaProperties = (tool: McpTool): SchemaProperties | undefined =>
  tool.inputSchema?.properties as SchemaProperties | undefined

const AutoApproveCell = ({
  tool,
  enabled,
  override,
  writing,
  onToggle
}: {
  tool: McpTool
  enabled: boolean
  override?: boolean
  writing: boolean
  onToggle: (tool: McpTool, autoApprove: boolean) => void
}) => {
  const { t } = useTranslation()
  const owned = useIsToolAutoApproved(tool)
  const isAutoApproved = override ?? owned
  return (
    <Tooltip
      content={
        !enabled
          ? t('settings.mcp.tools.autoApprove.tooltip.howToEnable')
          : isAutoApproved
            ? t('settings.mcp.tools.autoApprove.tooltip.enabled')
            : t('settings.mcp.tools.autoApprove.tooltip.disabled')
      }>
      <Switch
        size="xs"
        checked={isAutoApproved}
        disabled={!enabled || writing}
        onCheckedChange={(checked) => onToggle(tool, checked)}
      />
    </Tooltip>
  )
}

const McpToolsSection = ({ tools, server, searchText, onToggleTool, onToggleAutoApprove }: McpToolsSectionProps) => {
  const { t } = useTranslation()
  const [expandedRowKeys, setExpandedRowKeys] = useState<Key[]>([])
  const [overrides, setOverrides] = useState(NO_OVERRIDES)
  // Each toggle saves the whole disabled list read from the server, so a second write before the first
  // reconciles would drop it; switches stay locked until the owner answers.
  const [writing, setWriting] = useState(false)
  const collection = `mcp/${server.id}/tools`

  const isToolEnabled = (tool: McpTool) => overrides.get(tool.id)?.enabled ?? !server.disabledTools?.includes(tool.name)

  const writeTool = useCallback(
    async (tool: McpTool, override: ToolOverride, save: () => Promise<void>) => {
      const change = beginOptimistic<McpTool>(collection, { patch: { [tool.id]: (row) => row } })
      const own = (next: ToolOverride | undefined) =>
        setOverrides((current) => {
          const draft = new Map(current)
          if (next) draft.set(tool.id, next)
          else draft.delete(tool.id)
          return draft
        })
      setWriting(true)
      own(override)
      try {
        await save()
        change.confirm()
      } catch {
        change.rollback()
        toast.error(t('settings.mcp.updateError'))
      } finally {
        own(undefined)
        setWriting(false)
        change.release()
      }
    },
    [collection, t]
  )

  const handleToggle = (tool: McpTool, checked: boolean) =>
    void writeTool(tool, { enabled: checked }, () => onToggleTool(tool, checked))

  const handleAutoApproveToggle = (tool: McpTool, checked: boolean) =>
    void writeTool(tool, { autoApprove: checked }, () => onToggleAutoApprove(tool, checked))

  const getTypeBadgeClass = (type: string | undefined) => {
    switch (type) {
      case 'string':
        return 'border-foreground/30 bg-secondary text-foreground'
      case 'number':
        return 'border-success-border bg-success-subtle text-success-subtle-foreground'
      case 'boolean':
        return 'border-purple-500/30 bg-purple-500/10 text-purple-600 dark:text-purple-400'
      case 'object':
        return 'border-warning-border bg-warning-subtle text-warning-subtle-foreground'
      case 'array':
        return 'border-cyan-500/30 bg-cyan-500/10 text-cyan-600 dark:text-cyan-400'
      default:
        return 'border-border bg-background-subtle text-foreground'
    }
  }

  // Render a single schema property and its nested children as a tree node.
  const renderProperty = (key: string, property: SchemaProperty, required: boolean, depth: number = 0) => {
    const { type, description, items } = property
    const itemsType = items?.type
    const itemType = type === 'array' && itemsType ? `${itemsType}[]` : type

    return (
      <div key={key} data-schema-property={key} className="min-w-0 border-b border-border py-2 last:border-b-0">
        <Flex className="min-w-0 items-start gap-2">
          <Flex className="w-40 shrink-0 items-center gap-1">
            <span className="min-w-0 font-semibold wrap-anywhere">{key}</span>
            {required && (
              <Tooltip content={t('common.required_field')}>
                <RequiredMark />
              </Tooltip>
            )}
          </Flex>
          {itemType && <Badge className={`shrink-0 ${getTypeBadgeClass(type)}`}>{itemType}</Badge>}
          {description && (
            <span className="min-w-0 flex-1 text-sm leading-5 wrap-break-word text-muted-foreground">
              {description}
            </span>
          )}
        </Flex>
        {property.enum && (
          <div className="mt-1 ml-42 flex flex-wrap items-center gap-1.5">
            <span className="text-sm text-muted-foreground">
              {t('settings.mcp.tools.inputSchema.enum.allowedValues')}
            </span>
            {property.enum.map((enumValue, index) => (
              <Badge key={index} variant="outline">
                {String(enumValue)}
              </Badge>
            ))}
          </div>
        )}
        {depth < MAX_NESTING_DEPTH &&
          type === 'object' &&
          property.properties &&
          renderSchemaProperties(property.properties, property.required, depth + 1)}
        {depth < MAX_NESTING_DEPTH && type === 'array' && itemsType === 'object' && items?.properties && (
          <div className="mt-2">
            <span className="text-sm text-muted-foreground italic">items:</span>
            {renderSchemaProperties(items.properties, items.required, depth + 1)}
          </div>
        )}
      </div>
    )
  }

  const renderSchemaProperties = (
    properties: SchemaProperties,
    required: readonly string[] = [],
    depth: number = 0
  ) => {
    return (
      <div
        className={
          depth === 0
            ? 'mt-1 min-w-0 overflow-hidden rounded-md border border-border bg-background px-3 select-text'
            : 'mt-2 ml-3 min-w-0 border-l border-border pl-3 select-text'
        }>
        {Object.entries(properties).map(([key, property]) =>
          renderProperty(key, property, required.includes(key), depth)
        )}
      </div>
    )
  }

  const renderToolProperties = (tool: McpTool) => {
    const properties = getToolSchemaProperties(tool)
    const hasInputSchema = Boolean(properties && Object.keys(properties).length > 0)
    if (!tool.description && !hasInputSchema) return null

    return (
      <ColFlex className="gap-4">
        {tool.description && (
          <div>
            <h4 className="mb-2 text-sm font-bold text-foreground">{t('common.description')}</h4>
            <Markdown
              id={`mcp-tool-description-${tool.id}`}
              footnoteLabel={t('common.footnotes')}
              className="text-sm font-normal text-muted-foreground">
              {tool.description}
            </Markdown>
          </div>
        )}
        {hasInputSchema && properties && (
          <div>
            <h4 className="mb-2 text-sm font-bold text-foreground">{t('settings.mcp.tools.inputSchema.label')}</h4>
            {renderSchemaProperties(properties, tool.inputSchema.required)}
          </div>
        )}
      </ColFlex>
    )
  }

  const filteredTools = useMemo(() => {
    const query = searchText.trim().toLowerCase()

    if (!query) {
      return tools
    }

    return tools.filter((tool) =>
      [tool.name, tool.id, tool.description].some((value) => value?.toLowerCase().includes(query))
    )
  }, [searchText, tools])
  const optimistic = useOptimisticRows(collection, filteredTools, toolId)

  const columns: ColumnDef<McpTool>[] = [
    {
      id: 'name',
      header: () => <span className="font-medium">{t('settings.mcp.tools.availableTools')}</span>,
      meta: { width: 400, maxWidth: 400, className: 'overflow-hidden' },
      cell: ({ row }) => {
        const tool = row.original

        return (
          <ColFlex className="min-w-0 gap-1 overflow-hidden">
            <Flex className="min-w-0 items-center gap-1">
              <span className="truncate text-sm text-foreground" title={tool.name}>
                {tool.name}
              </span>
              <InfoTooltip content={`ID: ${tool.id}`} />
            </Flex>
            {tool.description && (
              <p className="m-0 line-clamp-1 w-full max-w-full min-w-0 overflow-hidden text-[13px] leading-5 text-muted-foreground">
                {tool.description}
              </p>
            )}
          </ColFlex>
        )
      }
    },
    {
      id: 'enable',
      header: () => (
        <Flex className="items-center justify-center gap-1">
          <McpLogo width={14} height={14} style={{ opacity: 0.8 }} />
          <span className="font-medium">{t('settings.mcp.tools.enable')}</span>
        </Flex>
      ),
      meta: { width: 150, maxWidth: 150, align: 'center' },
      cell: ({ row }) => {
        const tool = row.original

        return (
          <Switch
            size="xs"
            checked={isToolEnabled(tool)}
            disabled={writing}
            onCheckedChange={(checked) => handleToggle(tool, checked)}
          />
        )
      }
    },
    {
      id: 'autoApprove',
      header: () => (
        <Flex className="items-center justify-center gap-1">
          <Zap size={14} color="var(--error)" />
          <span className="font-medium">{t('settings.mcp.tools.autoApprove.label')}</span>
        </Flex>
      ),
      meta: { width: 150, maxWidth: 150, align: 'center' },
      cell: ({ row }) => (
        <AutoApproveCell
          tool={row.original}
          enabled={isToolEnabled(row.original)}
          override={overrides.get(row.original.id)?.autoApprove}
          writing={writing}
          onToggle={handleAutoApproveToggle}
        />
      )
    }
  ]

  return (
    <DataTable
      data={optimistic.rows}
      columns={columns}
      rowKey="id"
      rowMotion={(tool) => optimistic.motion(tool.id)}
      emptyText={searchText ? t('common.no_results') : t('settings.mcp.tools.noToolsAvailable')}
      expandedRowKeys={expandedRowKeys}
      onExpandedRowChange={setExpandedRowKeys}
      renderExpandedRow={(tool) => renderToolProperties(tool)}
      getCanExpand={(tool) => {
        const properties = getToolSchemaProperties(tool)
        return Boolean(tool.description) || Boolean(properties && Object.keys(properties).length)
      }}
      tableLayout="fixed"
      className="bg-transparent [&_[data-slot=table-cell]]:bg-transparent [&_[data-slot=table-head]]:bg-transparent [&_[data-slot=table-header]]:bg-transparent [&_[data-slot=table-header]_[data-slot=table-row]]:bg-transparent"
      rowClassName="bg-transparent"
    />
  )
}

export default McpToolsSection
