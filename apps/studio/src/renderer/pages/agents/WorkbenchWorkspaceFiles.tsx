import { Checkbox } from '@a3s-lab/ui'
import { cn } from '@a3s-lab/ui/lib/utils'
import { ChevronDown, ChevronRight } from 'lucide-react'
import {
  useCallback,
  useEffect,
  useLayoutEffect,
  useId,
  useMemo,
  useRef,
  useState,
  type Dispatch,
  type KeyboardEvent,
  type SetStateAction
} from 'react'
import { useTranslation } from 'react-i18next'

import { createComposerFolderToken } from '@renderer/components/composer/folderToken'
import type { ComposerDraftToken } from '@renderer/components/composer/tokens'
import {
  agentComposerTokenId,
  agentFileToComposerToken
} from '@renderer/components/composer/variants/agentComposerTokens'
import { workspaceGroupIcon } from '@renderer/components/composer/workspaceFileGroup'
import {
  DocumentFileIcon,
  DocumentFolderIcon,
  workspaceFileIconType
} from '@renderer/components/icons/DocumentFileIcon'
import Scrollbar from '@renderer/components/Scrollbar'
import { SelectorShell } from '@renderer/components/SelectorShell'
import { FILE_TYPE } from '@renderer/types/file'
import type { ComposerAttachment } from '@renderer/utils/message/composerAttachment'
import { createComposerFileTokenSourceId } from '@renderer/utils/message/composerFileTokenSource'
import { AbsoluteFilePathSchema, type DirectoryEntry } from '@shared/types/file'
import { getFileTypeByExt } from '@shared/utils/file'

const SEARCH_DEBOUNCE_MS = 200
const RESULT_LIMIT = 40
const SEARCH_MAX_DEPTH = 10

export interface WorkspaceFileChoice {
  path: string
  isDirectory: boolean
}

export interface WorkbenchWorkspaceFilesControl {
  root: string
  selectedPaths: readonly string[]
  /** Replaces the attached set. The popover commits once, after the user confirms. */
  onCommit: (next: readonly WorkspaceFileChoice[]) => void
}

const normalizePath = (value: string) => value.replace(/\\/g, '/')

const stripTrailingSlash = (value: string) => value.replace(/\/+$/, '')

const baseName = (value: string) => {
  const normalized = stripTrailingSlash(normalizePath(value))
  return normalized.split('/').pop() || normalized
}

const isDirectoryPath = (value: string) => normalizePath(value).endsWith('/')

const parentPath = (value: string) => {
  const normalized = normalizePath(value)
  const index = normalized.lastIndexOf('/')
  return index > 0 ? normalized.slice(0, index) : ''
}

const relativeToRoot = (root: string, filePath: string) => {
  const normalizedRoot = normalizePath(root).replace(/\/$/, '')
  const normalizedPath = normalizePath(filePath)
  const prefix = `${normalizedRoot}/`
  return normalizedPath.startsWith(prefix) ? normalizedPath.slice(prefix.length) : normalizedPath
}

export function selectedWorkspacePaths(
  files: readonly Pick<ComposerAttachment, 'path'>[],
  tokens: readonly Pick<ComposerDraftToken, 'kind' | 'promptText'>[]
): string[] {
  const paths: string[] = []
  const seen = new Set<string>()
  const add = (value: string) => {
    const normalized = normalizePath(value).trim()
    const key = stripTrailingSlash(normalized)
    if (!key || seen.has(key)) return
    seen.add(key)
    paths.push(normalized)
  }
  for (const file of files) {
    if (file.path) add(file.path)
  }
  for (const token of tokens) {
    if (token.kind !== 'folder' || !token.promptText) continue
    for (const line of token.promptText.split('\n')) add(line)
  }
  return paths
}

const attachmentFromPath = (filePath: string): ComposerAttachment | null => {
  const parsed = AbsoluteFilePathSchema.safeParse(normalizePath(filePath))
  if (!parsed.success) return null
  const name = baseName(parsed.data)
  const extensionIndex = name.lastIndexOf('.')
  const ext = extensionIndex > 0 ? name.slice(extensionIndex) : ''
  return {
    fileTokenSourceId: createComposerFileTokenSourceId(),
    name,
    origin_name: name,
    path: parsed.data,
    size: 0,
    ext,
    type: ext ? getFileTypeByExt(ext) : FILE_TYPE.OTHER
  }
}

export function applyWorkspaceFileToggle(input: {
  choice: WorkspaceFileChoice
  files: readonly ComposerAttachment[]
  tokens: readonly ComposerDraftToken[]
  insertToken: (token: ComposerDraftToken) => void
  removeToken: (tokenId: string) => void
  setFiles: Dispatch<SetStateAction<ComposerAttachment[]>>
}) {
  const pathKey = normalizePath(input.choice.path)
  if (input.choice.isDirectory) {
    const existing = input.tokens.find(
      (token) => token.kind === 'folder' && token.promptText && normalizePath(token.promptText) === pathKey
    )
    if (existing) {
      input.removeToken(existing.id)
      return
    }
    input.insertToken(createComposerFolderToken(input.choice.path))
    return
  }

  const existing = input.files.find((file) => file.path && normalizePath(file.path) === pathKey)
  if (existing) {
    input.removeToken(agentComposerTokenId.file(existing))
    input.setFiles((prev) => prev.filter((file) => !file.path || normalizePath(file.path) !== pathKey))
    return
  }

  const file = attachmentFromPath(input.choice.path)
  if (!file) return
  input.insertToken(agentFileToComposerToken(file))
  input.setFiles((prev) =>
    prev.some((item) => item.path && normalizePath(item.path) === pathKey) ? prev : [...prev, file]
  )
}

/** Applies a confirmed file set in one pass. Each step sees the previous step's result. */
export function applyWorkspaceFileSelection(input: {
  next: readonly WorkspaceFileChoice[]
  files: readonly ComposerAttachment[]
  tokens: readonly ComposerDraftToken[]
  insertToken: (token: ComposerDraftToken) => void
  removeToken: (tokenId: string) => void
  setFiles: Dispatch<SetStateAction<ComposerAttachment[]>>
}) {
  const wanted = new Map<string, WorkspaceFileChoice>()
  for (const choice of input.next) {
    const key = normalizePath(choice.path)
    if (!wanted.has(key)) wanted.set(key, choice)
  }
  let files = input.files.slice()
  let tokens = input.tokens.slice()
  const current = new Map<string, WorkspaceFileChoice>()
  for (const file of files) {
    if (file.path) current.set(normalizePath(file.path), { path: file.path, isDirectory: false })
  }
  for (const token of tokens) {
    if (token.kind === 'folder' && token.promptText) {
      current.set(normalizePath(token.promptText), { path: token.promptText, isDirectory: true })
    }
  }

  const apply = (choice: WorkspaceFileChoice) => {
    applyWorkspaceFileToggle({
      choice,
      files,
      tokens,
      insertToken: (token) => {
        input.insertToken(token)
        tokens = [...tokens, { ...token }]
      },
      removeToken: (tokenId) => {
        input.removeToken(tokenId)
        tokens = tokens.filter((token) => token.id !== tokenId)
      },
      setFiles: (update) => {
        files = typeof update === 'function' ? update(files) : update
      }
    })
  }

  for (const [key, choice] of current) {
    if (!wanted.has(key)) apply(choice)
  }
  for (const [key, choice] of wanted) {
    if (!current.has(key)) apply(choice)
  }
  input.setFiles(files)
}

const PANEL_WIDTH = 320
const PANEL_MAX_HEIGHT = 344

const directoryListOptions = {
  recursive: false,
  includeHidden: false,
  includeFiles: true,
  includeDirectories: true,
  maxEntries: RESULT_LIMIT,
  searchPattern: '.'
} as const

function sortDirectoryEntries(entries: readonly DirectoryEntry[]): DirectoryEntry[] {
  return entries.slice().sort((left, right) => {
    if (left.isDirectory !== right.isDirectory) return left.isDirectory ? -1 : 1
    return baseName(left.path).localeCompare(baseName(right.path))
  })
}

interface FileTreeRow {
  entry: DirectoryEntry
  depth: number
}

function visibleFileTreeRows(
  directory: string,
  depth: number,
  childrenByDirectory: Readonly<Record<string, readonly DirectoryEntry[]>>,
  expanded: ReadonlySet<string>
): FileTreeRow[] {
  const children = childrenByDirectory[normalizePath(directory)] ?? []
  const rows: FileTreeRow[] = []
  for (const entry of children) {
    rows.push({ entry, depth })
    const path = normalizePath(entry.path)
    if (entry.isDirectory && expanded.has(path)) {
      rows.push(...visibleFileTreeRows(path, depth + 1, childrenByDirectory, expanded))
    }
  }
  return rows
}

function placeFilePanel(trigger: HTMLElement | null): {
  side: 'top' | 'bottom'
  align: 'start' | 'end'
  room: number
} {
  const bounds = document.documentElement.getBoundingClientRect()
  const rect = trigger?.getBoundingClientRect()
  const spaceAbove = rect ? Math.max(0, rect.top - bounds.top - 16) : PANEL_MAX_HEIGHT
  const spaceBelow = rect ? Math.max(0, bounds.bottom - rect.bottom - 16) : 0
  const spaceRight = rect ? bounds.right - rect.left : PANEL_WIDTH
  const spaceLeft = rect ? rect.right - bounds.left : PANEL_WIDTH
  const side = spaceAbove >= spaceBelow ? 'top' : 'bottom'
  const available = Math.max(160, side === 'top' ? spaceAbove : spaceBelow)
  return {
    side,
    align: spaceRight >= PANEL_WIDTH + 16 || spaceRight >= spaceLeft ? 'start' : 'end',
    room: Math.min(PANEL_MAX_HEIGHT, available)
  }
}

export function WorkbenchWorkspaceFilePicker({ root, selectedPaths, onCommit }: WorkbenchWorkspaceFilesControl) {
  const { t } = useTranslation()
  const [open, setOpen] = useState(false)
  const [focusedPath, setFocusedPath] = useState('')
  const listId = useId()
  const listRef = useRef<HTMLDivElement>(null)
  const searchInputRef = useRef<HTMLInputElement>(null)
  const [query, setQuery] = useState('')
  const [childrenByDirectory, setChildrenByDirectory] = useState<Record<string, DirectoryEntry[]>>({})
  const [expanded, setExpanded] = useState<ReadonlySet<string>>(() => new Set())
  const [searchHits, setSearchHits] = useState<DirectoryEntry[]>([])
  const [directoryStates, setDirectoryStates] = useState<Record<string, 'loading' | 'ready' | 'error'>>({})
  const [searchLoading, setSearchLoading] = useState(false)
  const [searchError, setSearchError] = useState(false)
  const [searchRetry, setSearchRetry] = useState(0)
  const [draft, setDraft] = useState<WorkspaceFileChoice[]>([])
  const [placement, setPlacement] = useState(() => placeFilePanel(null))
  const requestGeneration = useRef(0)
  const directoryGeneration = useRef(new Map<string, number>())
  const directoryRequest = useRef(0)
  const triggerRef = useRef<HTMLButtonElement>(null)
  const trimmedQuery = query.trim()
  const loading = trimmedQuery ? searchLoading : directoryStates[normalizePath(root)] === 'loading'
  const loadFailed = trimmedQuery ? searchError : directoryStates[normalizePath(root)] === 'error'
  const draftPaths = useMemo(() => draft.map((choice) => normalizePath(choice.path)), [draft])
  const draftSelected = useMemo(() => new Set(draftPaths), [draftPaths])
  const unchanged = draft.length === 0

  const loadDirectory = useCallback((directory: string) => {
    const key = normalizePath(directory)
    const generation = ++directoryRequest.current
    directoryGeneration.current.set(key, generation)
    setDirectoryStates((current) => ({ ...current, [key]: 'loading' }))
    void window.api.file
      .listDirectoryEntries(directory, directoryListOptions)
      .then((listed) => {
        if (directoryGeneration.current.get(key) !== generation) return
        setChildrenByDirectory((current) => ({ ...current, [key]: sortDirectoryEntries(listed) }))
        setDirectoryStates((current) => ({ ...current, [key]: 'ready' }))
      })
      .catch(() => {
        if (directoryGeneration.current.get(key) !== generation) return
        setChildrenByDirectory((current) => ({ ...current, [key]: [] }))
        setDirectoryStates((current) => ({ ...current, [key]: 'error' }))
      })
  }, [])

  useEffect(() => {
    if (!open || trimmedQuery || !root) return
    loadDirectory(root)
  }, [loadDirectory, open, root, trimmedQuery])

  useEffect(() => {
    if (!open) return
    const generation = ++requestGeneration.current
    setSearchError(false)
    setSearchLoading(Boolean(trimmedQuery && root))
    if (!trimmedQuery) {
      setSearchHits([])
      return
    }
    const timer = window.setTimeout(() => {
      if (!root) {
        if (generation === requestGeneration.current) {
          setSearchHits([])
          setSearchLoading(false)
        }
        return
      }
      void window.api.file
        .listDirectoryEntries(root, {
          recursive: true,
          maxDepth: SEARCH_MAX_DEPTH,
          includeHidden: false,
          includeFiles: true,
          includeDirectories: true,
          maxEntries: RESULT_LIMIT,
          searchPattern: trimmedQuery
        })
        .then((listed) => {
          if (generation !== requestGeneration.current) return
          setSearchHits(listed)
        })
        .catch(() => {
          if (generation !== requestGeneration.current) return
          setSearchHits([])
          setSearchError(true)
        })
        .finally(() => {
          if (generation === requestGeneration.current) setSearchLoading(false)
        })
    }, SEARCH_DEBOUNCE_MS)
    return () => window.clearTimeout(timer)
  }, [open, root, trimmedQuery, searchRetry])

  const onOpenChange = useCallback((next: boolean) => {
    if (next) {
      setPlacement(placeFilePanel(triggerRef.current))
      setDraft([])
      setFocusedPath('')
    }
    setOpen(next)
    if (!next) {
      requestGeneration.current += 1
      setQuery('')
      setExpanded(new Set())
      setChildrenByDirectory({})
      setSearchHits([])
      setSearchError(false)
      setSearchLoading(false)
      setDirectoryStates({})
      directoryGeneration.current.clear()
    }
  }, [])

  useLayoutEffect(() => {
    if (!open) return
    const update = () => setPlacement(placeFilePanel(triggerRef.current))
    update()
    window.addEventListener('resize', update)
    return () => window.removeEventListener('resize', update)
  }, [open])

  const toggleExpanded = useCallback(
    (directory: string) => {
      const key = normalizePath(directory)
      setExpanded((current) => {
        const next = new Set(current)
        if (next.has(key)) next.delete(key)
        else next.add(key)
        return next
      })
      if (!childrenByDirectory[key]) loadDirectory(directory)
    },
    [childrenByDirectory, loadDirectory]
  )

  const treeRows = useMemo(
    () => (root && !trimmedQuery ? visibleFileTreeRows(root, 0, childrenByDirectory, expanded) : []),
    [childrenByDirectory, expanded, root, trimmedQuery]
  )
  const rows: FileTreeRow[] = trimmedQuery ? searchHits.map((entry) => ({ entry, depth: 0 })) : treeRows
  const tabStopPath = rows.some((row) => normalizePath(row.entry.path) === focusedPath)
    ? focusedPath
    : normalizePath(rows[0]?.entry.path ?? '')

  const toggleDraft = useCallback((choice: WorkspaceFileChoice) => {
    const key = normalizePath(choice.path)
    setDraft((current) => {
      const exists = current.some((item) => normalizePath(item.path) === key)
      return exists ? current.filter((item) => normalizePath(item.path) !== key) : [...current, choice]
    })
  }, [])

  const navigateRows = (event: KeyboardEvent<HTMLElement>, fromSearch = false) => {
    // Keep tree rows focusable in the native WebView, including newly expanded and offscreen entries.
    // oxlint-disable-next-line no-deprecated
    if (event.nativeEvent.isComposing || event.keyCode === 229) return
    const options = Array.from(listRef.current?.querySelectorAll<HTMLElement>('[role="option"]') ?? [])
    if (options.length === 0) return
    const target = event.target instanceof Element ? event.target.closest<HTMLElement>('[role="option"]') : null
    const current = target ? options.indexOf(target) : -1
    const focus = (index: number) => {
      const option = options[index]
      option?.focus()
      option?.scrollIntoView({ block: 'nearest' })
    }
    const handled = () => {
      event.preventDefault()
      event.stopPropagation()
    }
    if (event.key === 'ArrowDown' || event.key === 'ArrowUp') {
      handled()
      focus(
        current < 0
          ? event.key === 'ArrowDown'
            ? 0
            : options.length - 1
          : Math.max(0, Math.min(options.length - 1, current + (event.key === 'ArrowDown' ? 1 : -1)))
      )
      return
    }
    if (fromSearch || current < 0) return
    if (event.key === 'Home' || event.key === 'End') {
      handled()
      focus(event.key === 'Home' ? 0 : options.length - 1)
    } else if (event.key === ' ' || event.key === 'Enter') {
      handled()
      target?.click()
    } else if (!trimmedQuery && (event.key === 'ArrowLeft' || event.key === 'ArrowRight')) {
      const row = rows[current]
      if (!row) return
      handled()
      const path = normalizePath(row.entry.path)
      if (event.key === 'ArrowRight') {
        if (row.entry.isDirectory && !expanded.has(path)) toggleExpanded(row.entry.path)
        else if (rows[current + 1]?.depth > row.depth) focus(current + 1)
      } else if (row.entry.isDirectory && expanded.has(path)) toggleExpanded(row.entry.path)
      else {
        const parent = parentPath(path)
        const index = rows.findIndex((item) => normalizePath(item.entry.path) === parent)
        if (index >= 0) focus(index)
      }
    }
  }

  const commit = useCallback(() => {
    if (unchanged) return
    onCommit(draft)
    onOpenChange(false)
  }, [draft, onCommit, onOpenChange, unchanged])

  const triggerLabel = t('workbench.composer.mention.title', '工作区文件')
  const triggerTitle =
    selectedPaths.length > 0
      ? selectedPaths.map((path) => stripTrailingSlash(normalizePath(path))).join('\n')
      : root
        ? undefined
        : t('workbench.composer.files.need_project', '先选择项目文件夹')
  const triggerIcon =
    selectedPaths.length === 0
      ? 'file'
      : workspaceGroupIcon(selectedPaths.map((path) => ({ path, isDirectory: isDirectoryPath(path) })))

  return (
    <SelectorShell
      trigger={
        <button
          ref={triggerRef}
          type="button"
          data-workbench-files-trigger="true"
          aria-label={t('workbench.composer.mention.title', '工作区文件')}
          disabled={!root}
          title={triggerTitle}
          className={`inline-flex h-7 max-w-[46%] min-w-0 items-center gap-1.5 rounded-[8px] px-2 text-[13px] transition-colors hover:bg-black/[0.05] hover:text-[#171717] disabled:opacity-50 data-[state=open]:bg-black/[0.05] ${
            selectedPaths.length > 0 ? 'text-[#171717]' : 'text-[#60646c]'
          }`}>
          <span data-workspace-file-icon={triggerIcon} className="inline-flex shrink-0">
            {triggerIcon === 'directory' ? (
              <DocumentFolderIcon className="size-4 shrink-0" />
            ) : (
              <DocumentFileIcon type={triggerIcon} className="size-4 shrink-0" />
            )}
          </span>
          <span className="truncate">{triggerLabel}</span>
          <ChevronDown size={12} strokeWidth={1.7} className="shrink-0 text-[#8a8a8a]" />
        </button>
      }
      open={open}
      onOpenChange={onOpenChange}
      side={placement.side}
      align={placement.align}
      sideOffset={6}
      width={PANEL_WIDTH}
      contentHeight={placement.room}
      maxContentHeight={placement.room}
      contentProps={{
        avoidCollisions: true,
        collisionBoundary: typeof document === 'undefined' ? undefined : document.documentElement,
        collisionPadding: 16,
        sticky: 'partial'
      }}
      search={{
        inputRef: searchInputRef,
        value: query,
        onChange: setQuery,
        ariaControls: listId,
        onKeyDown: (event) => navigateRows(event, true),
        placeholder: t('workbench.composer.files.search', '搜索文件')
      }}>
      {() => (
        <div className="flex h-full min-h-0 flex-col">
          <Scrollbar
            ref={listRef}
            id={listId}
            onKeyDown={(event) => navigateRows(event)}
            role="listbox"
            aria-busy={loading}
            aria-label={t('workbench.composer.mention.title', '工作区文件')}
            aria-multiselectable="true"
            className="min-h-0 flex-1 px-1 py-1 outline-none">
            {!root ? (
              <p className="px-2.5 py-3 text-[13px] text-[#8a8a8a]">
                {t('workbench.composer.files.need_project', '先选择项目文件夹')}
              </p>
            ) : loadFailed ? (
              <div className="flex items-center gap-2 px-2.5 py-3 text-[13px]">
                <p role="alert" className="min-w-0 flex-1 text-[#b42318]">
                  {t('workbench.composer.files.load_failed', '无法加载文件，请重试。')}
                </p>
                <button
                  type="button"
                  className="h-[30px] shrink-0 rounded-lg px-2 text-[#171717] hover:bg-[#f7f7f7]"
                  onClick={() => {
                    if (trimmedQuery) setSearchRetry((value) => value + 1)
                    else loadDirectory(root)
                    searchInputRef.current?.focus()
                  }}>
                  {t('common.retry', '重试')}
                </button>
              </div>
            ) : loading && rows.length === 0 ? (
              <p role="status" className="px-2.5 py-3 text-[13px] text-[#8a8a8a]">
                {t('common.loading', '加载中')}
              </p>
            ) : rows.length === 0 ? (
              <p className="px-2.5 py-3 text-[13px] text-[#8a8a8a]">
                {t('workbench.composer.files.empty', '没有匹配的文件')}
              </p>
            ) : (
              <div className="flex flex-col gap-0.5">
                {rows.map(({ entry, depth }) => {
                  const path = normalizePath(entry.path)
                  const checked = draftSelected.has(path)
                  const name = baseName(path)
                  const isOpen = entry.isDirectory && expanded.has(path)
                  return (
                    <div key={path}>
                      <div
                        role="option"
                        tabIndex={tabStopPath === path ? 0 : -1}
                        onFocus={() => setFocusedPath(path)}
                        aria-selected={checked}
                        data-workspace-file={path}
                        data-depth={depth}
                        onClick={() => toggleDraft({ path: entry.path, isDirectory: entry.isDirectory })}
                        style={{ paddingLeft: 10 + depth * 16 }}
                        className={cn(
                          'flex h-9 w-full cursor-pointer items-center gap-2 rounded-lg pr-2.5 text-left text-[13px] outline-none focus-visible:ring-2 focus-visible:ring-black/10 focus-visible:ring-inset',
                          checked
                            ? 'bg-[#f2f3f5] font-medium text-[#171717]'
                            : 'text-[#3f3f3f] hover:bg-[#f7f7f7] hover:text-[#171717]'
                        )}>
                        {entry.isDirectory && !trimmedQuery ? (
                          <button
                            type="button"
                            tabIndex={-1}
                            aria-expanded={isOpen}
                            aria-label={
                              isOpen
                                ? t('workbench.composer.files.collapse', '收起 {{name}}').replace('{{name}}', name)
                                : t('workbench.composer.files.expand', '展开 {{name}}').replace('{{name}}', name)
                            }
                            className="flex size-4 shrink-0 items-center justify-center rounded text-[#8a8a8a] hover:text-[#171717]"
                            onClick={(event) => {
                              event.stopPropagation()
                              toggleExpanded(entry.path)
                            }}>
                            <ChevronRight
                              size={14}
                              strokeWidth={1.7}
                              className={cn('shrink-0 transition-transform', isOpen && 'rotate-90')}
                            />
                          </button>
                        ) : (
                          <span className="size-4 shrink-0" />
                        )}
                        <Checkbox
                          checked={checked}
                          size="sm"
                          tabIndex={-1}
                          aria-hidden="true"
                          className="pointer-events-none"
                        />
                        {entry.isDirectory ? (
                          <DocumentFolderIcon className="size-4 shrink-0" />
                        ) : (
                          <DocumentFileIcon type={workspaceFileIconType(name)} className="size-4 shrink-0" />
                        )}
                        <span className="truncate">{name}</span>
                        {trimmedQuery ? (
                          <span className="min-w-0 flex-1 truncate text-[12px] font-normal text-[#8a8a8a]">
                            {parentPath(relativeToRoot(root, path))}
                          </span>
                        ) : null}
                      </div>
                      {isOpen && directoryStates[path] === 'loading' && (
                        <p
                          role="status"
                          style={{ paddingLeft: 26 + depth * 16 }}
                          className="py-2 text-[12px] text-[#8a8a8a]">
                          {t('common.loading', '加载中')}
                        </p>
                      )}
                      {isOpen && directoryStates[path] === 'error' && (
                        <div
                          style={{ paddingLeft: 26 + depth * 16 }}
                          className="flex items-center gap-2 py-2 pr-2.5 text-[12px]">
                          <p role="alert" className="min-w-0 flex-1 text-[#b42318]">
                            {name}：{t('workbench.composer.files.load_failed', '无法加载文件，请重试。')}
                          </p>
                          <button
                            type="button"
                            aria-label={t('workbench.composer.files.retry_directory', '重试 {{name}}').replace(
                              '{{name}}',
                              name
                            )}
                            className="h-[30px] shrink-0 rounded-lg px-2 text-[#171717] hover:bg-[#f7f7f7]"
                            onClick={() => {
                              loadDirectory(entry.path)
                              listRef.current
                                ?.querySelector<HTMLElement>(`[data-workspace-file="${CSS.escape(path)}"]`)
                                ?.focus()
                            }}>
                            {t('common.retry', '重试')}
                          </button>
                        </div>
                      )}
                    </div>
                  )
                })}
              </div>
            )}
          </Scrollbar>
          <div className="flex shrink-0 items-center justify-end gap-2 border-t border-[#efefef] px-2.5 py-2">
            <button
              type="button"
              onClick={() => onOpenChange(false)}
              className="h-[30px] rounded-lg border border-[#e5e5e5] bg-white px-3 text-[13px] text-[#171717] hover:bg-[#fafafa]">
              {t('workbench.composer.files.cancel', '取消')}
            </button>
            <button
              type="button"
              disabled={unchanged || !root}
              onClick={commit}
              className="h-[30px] rounded-lg bg-[#171717] px-3 text-[13px] text-white disabled:opacity-40">
              {t('workbench.composer.files.apply', '添加 {{count}} 项').replace('{{count}}', String(draft.length))}
            </button>
          </div>
        </div>
      )}
    </SelectorShell>
  )
}
