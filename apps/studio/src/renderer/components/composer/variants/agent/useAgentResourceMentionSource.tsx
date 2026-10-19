import type { Editor } from '@tiptap/core'
import { ChevronLeft, ChevronRight, CornerDownLeft } from 'lucide-react'
import { useCallback, useEffect, useMemo, useRef } from 'react'
import { useTranslation } from 'react-i18next'

import {
  DocumentFileIcon,
  DocumentFolderIcon,
  workspaceFileIconType
} from '@renderer/components/icons/DocumentFileIcon'
import { FILE_TYPE } from '@renderer/types/file'
import type { ComposerAttachment } from '@renderer/utils/message/composerAttachment'
import { createComposerFileTokenSourceId } from '@renderer/utils/message/composerFileTokenSource'
import { type AbsoluteFilePath, AbsoluteFilePathSchema, type DirectoryEntry } from '@shared/types/file'
import { getFileTypeByExt } from '@shared/utils/file'

import { serializeComposerDocument } from '../../composerDraft'
import { createComposerFolderToken } from '../../folderToken'
import type { ComposerSuggestionItem, ComposerSuggestionSource } from '../../quickPanel'
import { agentComposerTokenId, agentFileToComposerToken } from '../agentComposerTokens'
import { getAccessiblePathRelativePath, getPathComparisonKey } from './accessiblePath'

const normalizePathSeparators = (filePath: string) => filePath.replace(/\\/g, '/')

const getBaseName = (filePath: string) => {
  const normalized = normalizePathSeparators(filePath)
  return normalized.split('/').pop() || normalized
}

const getParentPath = (filePath: string) => {
  const normalized = normalizePathSeparators(filePath)
  const index = normalized.lastIndexOf('/')
  return index > 0 ? normalized.slice(0, index) : ''
}

const getFileExtension = (fileName: string) => {
  const lastDotIndex = fileName.lastIndexOf('.')
  return lastDotIndex > 0 ? fileName.slice(lastDotIndex) : ''
}

const createAttachmentFromPath = (filePath: AbsoluteFilePath): ComposerAttachment => {
  const name = getBaseName(filePath)
  const ext = getFileExtension(name)
  return {
    fileTokenSourceId: createComposerFileTokenSourceId(),
    name,
    origin_name: name,
    path: filePath,
    size: 0,
    ext,
    type: ext ? getFileTypeByExt(ext) : FILE_TYPE.OTHER
  }
}

const createStablePathHash = (value: string) => {
  let hash = 0
  for (let index = 0; index < value.length; index++) {
    hash = (hash * 31 + value.charCodeAt(index)) >>> 0
  }
  return hash.toString(36)
}

// Item id is derived from the file path (not the token's fileTokenSourceId, which is regenerated
// per query) so the QuickPanel keeps a stable identity — and selection highlight — across keystrokes.
const createAgentResourceItemId = (filePath: string) =>
  `agent-resource:${createStablePathHash(filePath.replace(/\\/g, '/'))}`

const EMPTY_QUERY_RESOURCE_LIMIT = 5
// Matches main's own `listDirectory` default depth; the previous 3 hid every file
// nested deeper than three levels in a real workspace.
const RESOURCE_SEARCH_MAX_DEPTH = 10
const RESOURCE_RESULT_LIMIT = 40
const BROWSE_RESULT_LIMIT = 200
const MENTION_ICON_CLASS = 'h-5 w-4'

// The quick panel shrinks direct svg children to 1em, so icons keep their own wrapper.
function mentionFolderIcon(open = false) {
  return (
    <span className="flex w-5 justify-center">
      <DocumentFolderIcon open={open} className={MENTION_ICON_CLASS} />
    </span>
  )
}

function mentionFileIcon(name: string) {
  return (
    <span className="flex w-5 justify-center">
      <DocumentFileIcon type={workspaceFileIconType(name)} className={MENTION_ICON_CLASS} />
    </span>
  )
}
const RESOURCE_SEARCH_DEBOUNCE_MS = 200

type ResourceSearchResult = PromiseSettledResult<DirectoryEntry[]>[] | null

interface PendingResourceSearch {
  resolve: (results: ResourceSearchResult) => void
  timerId: number
}

const createAccessiblePathsKey = (accessiblePaths: readonly string[]) =>
  accessiblePaths.map((item) => item.replace(/\\/g, '/')).join('\0')

interface BrowseQuery {
  /** Folder path relative to the browse root, `/`-separated, without a trailing slash. */
  dir: string
  /** Lower-cased name filter typed after the last `/`. */
  filter: string
}

/** `apps/studio/` browses `apps/studio`; `apps/st` browses `apps` filtered by `st`. */
export const parseBrowseQuery = (query: string): BrowseQuery | null => {
  const index = query.lastIndexOf('/')
  if (index <= 0) return null
  const dir = query.slice(0, index)
  if (dir.split('/').some((segment) => !segment || segment === '.' || segment === '..')) return null
  return { dir, filter: query.slice(index + 1).toLowerCase() }
}

/** Parent directory of a browse path. The workspace root is an empty string. */
export const parentBrowseDir = (dir: string) => {
  const index = dir.lastIndexOf('/')
  return index === -1 ? '' : dir.slice(0, index)
}

const joinBrowsePath = (root: string, relative: string) => {
  const separator = root.includes('\\') && !root.includes('/') ? '\\' : '/'
  return `${root.replace(/[\\/]+$/, '')}${separator}${relative.split('/').join(separator)}`
}

const openBrowseFolder = (editor: Editor, relativePath: string, trigger: string) => {
  // After the panel has consumed the old query and closed, so the new query re-opens it.
  window.requestAnimationFrame(() => {
    editor
      .chain()
      .focus()
      .insertContent(relativePath ? `${trigger}${relativePath}/` : trigger)
      .run()
  })
}

const createGroupHeaderItem = (id: string, label: string): ComposerSuggestionItem => ({
  id,
  label,
  disabled: true,
  command: () => undefined
})

interface AgentResourceMentionOptions {
  accessiblePaths: readonly AbsoluteFilePath[]
  files: ComposerAttachment[]
  setFiles: React.Dispatch<React.SetStateAction<ComposerAttachment[]>>
  /** Whether the agent session exposes any accessible workspace paths to mention. */
  enabled: boolean
  /** Extra items appended after the workspace resources, sharing the same panel. */
  getAdditionalItems?: (options: { query: string; editor: Editor }) => Promise<ComposerSuggestionItem[]>
  /** Character that opens this panel. Defaults to `@`. */
  char?: string
  /** Panel title; defaults to the resources-and-sessions title. */
  title?: string
  /**
   * `path` shows the relative path with the absolute path beside it; `name` shows the entry name
   * with its parent folder beside it, so long paths never hide the file name.
   */
  labelStyle?: 'path' | 'name'
  /**
   * With a single accessible root, folder rows open the folder instead of inserting it, and a
   * `dir/` query lists that folder with a breadcrumb and a row to reference the folder itself.
   */
  browseFolders?: boolean
}

/**
 * Provides the agent composer's workspace-file suggestion source. Files become managed file
 * tokens; folders become path-only folder tokens. An empty query reads one bounded root page,
 * while a real query delegates bounded fuzzy search to main. Returns an empty list when disabled
 * and no additional source is registered.
 */
export function useAgentResourceMentionSource({
  accessiblePaths,
  files,
  setFiles,
  enabled,
  getAdditionalItems,
  title,
  labelStyle = 'path',
  browseFolders = false,
  char = '@'
}: AgentResourceMentionOptions): ComposerSuggestionSource[] {
  const { t } = useTranslation()
  const pendingResourceSearchRef = useRef<PendingResourceSearch | undefined>(undefined)
  const resourceSearchGenerationRef = useRef(0)
  const accessiblePathsKey = createAccessiblePathsKey(accessiblePaths)
  const resourceMentionStateRef = useRef({
    accessiblePaths,
    accessiblePathsKey,
    files,
    setFiles,
    getAdditionalItems,
    labelStyle,
    browseFolders,
    t
  })
  resourceMentionStateRef.current = {
    accessiblePaths,
    accessiblePathsKey,
    files,
    setFiles,
    getAdditionalItems,
    labelStyle,
    browseFolders,
    t
  }

  const invalidateResourceSearch = useCallback(() => {
    resourceSearchGenerationRef.current += 1
    const pending = pendingResourceSearchRef.current
    if (!pending) return resourceSearchGenerationRef.current
    window.clearTimeout(pending.timerId)
    pendingResourceSearchRef.current = undefined
    pending.resolve(null)
    return resourceSearchGenerationRef.current
  }, [])

  const queryWorkspaceResources = useCallback(
    (
      searchPaths: readonly string[],
      normalizedQuery: string,
      listLimit = RESOURCE_RESULT_LIMIT
    ): Promise<ResourceSearchResult> => {
      const requestGeneration = invalidateResourceSearch()
      const options = normalizedQuery
        ? {
            recursive: true,
            maxDepth: RESOURCE_SEARCH_MAX_DEPTH,
            includeHidden: false,
            includeFiles: true,
            includeDirectories: true,
            maxEntries: RESOURCE_RESULT_LIMIT,
            searchPattern: normalizedQuery
          }
        : {
            recursive: false,
            includeHidden: false,
            includeFiles: true,
            includeDirectories: true,
            maxEntries: listLimit,
            searchPattern: '.'
          }
      const execute = async (): Promise<ResourceSearchResult> => {
        const results = await Promise.allSettled(
          searchPaths.map((dirPath) => window.api.file.listDirectoryEntries(dirPath, options))
        )
        return requestGeneration === resourceSearchGenerationRef.current ? results : null
      }

      if (!normalizedQuery) return execute()

      return new Promise<ResourceSearchResult>((resolve) => {
        const timerId = window.setTimeout(() => {
          if (pendingResourceSearchRef.current === pendingSearch) pendingResourceSearchRef.current = undefined
          void execute().then(resolve)
        }, RESOURCE_SEARCH_DEBOUNCE_MS)
        const pendingSearch = { resolve, timerId }
        pendingResourceSearchRef.current = pendingSearch
      })
    },
    [invalidateResourceSearch]
  )

  useEffect(
    () => () => {
      invalidateResourceSearch()
    },
    [accessiblePathsKey, invalidateResourceSearch]
  )

  const resourceMentionSource = useMemo<ComposerSuggestionSource>(
    () => ({
      pluginKey: 'agent-resource-mention-suggestion',
      char,
      title: title ?? t('chat.input.resource_panel.title'),
      allowedPrefixes: [' ', '\n'],
      onExit: () => {
        invalidateResourceSearch()
      },
      onKeyDown: ({ event, view, range }) => {
        if (event.key !== 'Backspace' || event.altKey || event.metaKey || event.ctrlKey || event.shiftKey) return false
        if (event.isComposing) return false
        const { accessiblePaths, browseFolders } = resourceMentionStateRef.current
        if (!browseFolders || accessiblePaths.length !== 1) return false
        const text = view.state.doc.textBetween(range.from, range.to, '\0', '\0')
        if (!text.startsWith(char)) return false
        const browse = parseBrowseQuery(text.slice(char.length))
        if (!browse || browse.filter) return false
        const parent = parentBrowseDir(browse.dir)
        view.dispatch(view.state.tr.insertText(parent ? `${char}${parent}/` : char, range.from, range.to))
        return true
      },
      items: async ({ query, editor }) => {
        const {
          accessiblePaths,
          accessiblePathsKey,
          files,
          setFiles,
          getAdditionalItems,
          labelStyle,
          browseFolders,
          t
        } = resourceMentionStateRef.current
        const normalizedQuery = query.trim()
        // Settled here, not at the await below: a rejection there would reject the whole source
        // and the suggestion wrapper would replace the loaded file results with a single error row.
        const additionalItemsPromise = getAdditionalItems?.({ query, editor }).catch((): ComposerSuggestionItem[] => [
          {
            id: 'agent-resource:sessions-error',
            label: t('common.error'),
            description: t('chat.input.reference_panel.load_failed'),
            disabled: true,
            command: () => undefined
          }
        ])

        const resourceItems: ComposerSuggestionItem[] = []
        if (accessiblePaths.length > 0) {
          // `.` is main's bounded direct-root listing sentinel; a real query switches to
          // bounded recursive fuzzy search without building a renderer-side directory index.
          const browseRoot = browseFolders && accessiblePaths.length === 1 ? accessiblePaths[0] : null
          let browse = browseRoot ? parseBrowseQuery(normalizedQuery) : null
          let results: ResourceSearchResult
          if (browseRoot && browse) {
            results = await queryWorkspaceResources([joinBrowsePath(browseRoot, browse.dir)], '', BROWSE_RESULT_LIMIT)
            // Not a folder in this project: treat the text as an ordinary path search.
            if (results && results.every((result) => result.status === 'rejected')) {
              browse = null
              results = await queryWorkspaceResources(accessiblePaths, normalizedQuery)
            }
          } else {
            results = await queryWorkspaceResources(accessiblePaths, normalizedQuery)
          }
          if (results === null || accessiblePathsKey !== resourceMentionStateRef.current.accessiblePathsKey) return []

          const collectedResources = new Map<AbsoluteFilePath, DirectoryEntry>()
          for (const result of results) {
            if (result.status !== 'fulfilled') continue
            for (const entry of result.value) {
              // `entry.path` is already an `AbsoluteFilePath`; separator normalization drops
              // the brand, so parse it again while preserving main/root order and first-wins dedupe.
              const entryPath = AbsoluteFilePathSchema.parse(normalizePathSeparators(entry.path))
              if (browse?.filter && !getBaseName(entryPath).toLowerCase().includes(browse.filter)) continue
              if (!collectedResources.has(entryPath)) collectedResources.set(entryPath, { ...entry, path: entryPath })
            }
          }

          const allRootsFailed = results.length > 0 && results.every((result) => result.status === 'rejected')
          if (collectedResources.size === 0 && allRootsFailed) {
            resourceItems.push({
              id: 'agent-resource:error',
              label: t('common.error'),
              description: t('chat.input.resource_panel.load_failed'),
              icon: mentionFolderIcon(),
              disabled: true,
              command: () => undefined
            })
          } else {
            const mentionedFolderPathKeys = new Set(
              serializeComposerDocument(editor).tokens.flatMap((token) =>
                token.kind === 'folder' && token.promptText ? [getPathComparisonKey(token.promptText)] : []
              )
            )

            for (const [entryPath, entry] of collectedResources) {
              const relativePath = getAccessiblePathRelativePath(entryPath, accessiblePaths)
              const label = labelStyle === 'name' ? getBaseName(relativePath) : relativePath
              const description = browse
                ? undefined
                : labelStyle === 'name'
                  ? getParentPath(relativePath) || undefined
                  : entryPath
              if (entry.isDirectory && browseRoot) {
                resourceItems.push({
                  id: createAgentResourceItemId(entryPath),
                  label,
                  description,
                  icon: mentionFolderIcon(),
                  suffix: <ChevronRight size={14} className="text-[#8a8a8a]" />,
                  filterText: `${relativePath} ${entryPath}`,
                  command: ({ editor }) => openBrowseFolder(editor, normalizePathSeparators(relativePath), char)
                })
                continue
              }
              if (entry.isDirectory) {
                const entryPathKey = getPathComparisonKey(entryPath)
                resourceItems.push({
                  id: createAgentResourceItemId(entryPath),
                  label,
                  description,
                  icon: mentionFolderIcon(),
                  filterText: `${relativePath} ${entryPath}`,
                  disabled: mentionedFolderPathKeys.has(entryPathKey),
                  command: ({ editor }) => {
                    const exists = serializeComposerDocument(editor).tokens.some(
                      (currentToken) =>
                        currentToken.kind === 'folder' &&
                        getPathComparisonKey(currentToken.promptText ?? '') === entryPathKey
                    )
                    if (!exists) {
                      editor
                        .chain()
                        .focus()
                        .insertComposerToken(createComposerFolderToken(entryPath))
                        .insertContent(' ')
                        .run()
                    }
                  }
                })
                continue
              }

              const filePath = entryPath
              const file = files.find((currentFile) => currentFile.path === filePath)
              const tokenFile = file ?? createAttachmentFromPath(filePath)
              const token = agentFileToComposerToken(tokenFile)
              const isSelectedFile = (currentFile: ComposerAttachment) =>
                currentFile.path === filePath || agentComposerTokenId.file(currentFile) === token.id

              resourceItems.push({
                id: createAgentResourceItemId(filePath),
                label,
                description,
                icon: mentionFileIcon(getBaseName(filePath)),
                filterText: `${relativePath} ${filePath}`,
                disabled: files.some(isSelectedFile),
                command: ({ editor }) => {
                  const exists = serializeComposerDocument(editor).tokens.some(
                    (currentToken) => currentToken.id === token.id
                  )
                  if (!exists) {
                    editor.chain().focus().insertComposerToken(token).insertContent(' ').run()
                  }
                  setFiles((prevFiles) => (prevFiles.some(isSelectedFile) ? prevFiles : [...prevFiles, tokenFile]))
                }
              })
            }
          }
          if (browseRoot && browse) {
            const folderPath = AbsoluteFilePathSchema.parse(
              normalizePathSeparators(joinBrowsePath(browseRoot, browse.dir))
            )
            const folderKey = getPathComparisonKey(folderPath)
            const parentDir = parentBrowseDir(browse.dir)
            const browseRows: ComposerSuggestionItem[] = [
              {
                id: 'agent-resource:breadcrumb',
                label: [getBaseName(browseRoot), ...browse.dir.split('/')].join(' / '),
                description: t('chat.input.resource_panel.browse_back'),
                icon: (
                  <span className="flex w-5 justify-center">
                    <ChevronLeft size={16} />
                  </span>
                ),
                alwaysVisible: true,
                command: ({ editor }) => openBrowseFolder(editor, parentDir, char)
              }
            ]
            if (!browse.filter) {
              const mentioned = serializeComposerDocument(editor).tokens.some(
                (token) => token.kind === 'folder' && getPathComparisonKey(token.promptText ?? '') === folderKey
              )
              browseRows.push({
                id: `agent-resource:use-folder:${createStablePathHash(folderPath)}`,
                label: t('chat.input.resource_panel.use_folder'),
                description: browse.dir,
                icon: <CornerDownLeft size={16} />,
                disabled: mentioned,
                command: ({ editor }) => {
                  editor
                    .chain()
                    .focus()
                    .insertComposerToken(createComposerFolderToken(folderPath))
                    .insertContent(' ')
                    .run()
                }
              })
            }
            if (resourceItems.length === 0) {
              browseRows.push({
                id: 'agent-resource:browse-empty',
                label: t(
                  browse.filter ? 'chat.input.resource_panel.browse_no_match' : 'chat.input.resource_panel.browse_empty'
                ),
                disabled: true,
                command: () => undefined
              })
            }
            resourceItems.unshift(...browseRows)
          }
        }

        const additionalItems = additionalItemsPromise ? await additionalItemsPromise : []
        if (resourceItems.length === 0 && additionalItems.length === 0 && accessiblePaths.length === 0) {
          return [
            {
              id: 'agent-resource:no-paths',
              label: t('chat.input.resource_panel.no_resources_found.label'),
              description: t('chat.input.resource_panel.no_resources_found.description'),
              icon: mentionFolderIcon(),
              disabled: true,
              command: () => undefined
            }
          ]
        }

        // With no query, an uncapped resource list buries the appended items below the fold,
        // so group both under header rows and keep only the first few resources; typing
        // searches both sides in full.
        if (!normalizedQuery && additionalItems.length > 0) {
          const grouped: ComposerSuggestionItem[] = []
          if (resourceItems.length > 0) {
            grouped.push(
              createGroupHeaderItem(
                'agent-resource:resources-header',
                t('chat.input.resource_panel.categories.resources')
              ),
              ...resourceItems.slice(0, EMPTY_QUERY_RESOURCE_LIMIT)
            )
          }
          grouped.push(
            createGroupHeaderItem('agent-resource:sessions-header', t('chat.input.reference_panel.session.title')),
            ...additionalItems
          )
          return grouped
        }
        return [...resourceItems, ...additionalItems]
      }
    }),
    [char, invalidateResourceSearch, queryWorkspaceResources, t, title]
  )

  return useMemo(
    () => (enabled || getAdditionalItems ? [resourceMentionSource] : []),
    [enabled, getAdditionalItems, resourceMentionSource]
  )
}
