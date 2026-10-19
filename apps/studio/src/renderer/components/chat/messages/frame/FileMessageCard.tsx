import { DocumentFileIcon, documentFileIconType } from '@renderer/components/icons/DocumentFileIcon'
import { formatFileSize } from '@renderer/im/imView'
import { cn } from '@renderer/utils/style'

/** Same mark the file-manager list uses: 32×40, to the left of the name. */
export const MESSAGE_FILE_ICON_CLASS = 'h-9 w-8'

/** The same document mark the files page and the messages file row use. */
export function MessageFileMark({ name, className }: { name: string; className?: string }) {
  const type = documentFileIconType(name)
  return (
    <span data-file-icon={type} className="contents">
      <DocumentFileIcon type={type} className={className} />
    </span>
  )
}

/** One file row: type icon, name, and size. The messages page and task transcript share it. */
export function FileMessageCard({
  name,
  iconName,
  sizeBytes,
  onOpen,
  busy = false,
  label
}: {
  name: string
  /** Extension source when the visible name does not carry one. */
  iconName?: string
  sizeBytes?: number
  onOpen?: () => void
  busy?: boolean
  label?: string
}) {
  return (
    <button
      type="button"
      data-ui="messages.file"
      aria-label={label ?? name}
      disabled={!onOpen || busy}
      onClick={onOpen}
      className={cn(
        'flex w-[220px] max-w-full items-center gap-2.5 rounded-[8px] bg-[#f5f5f5] px-3 py-2.5 text-left outline-none focus-visible:ring-2 focus-visible:ring-black/10',
        onOpen && !busy ? 'cursor-pointer hover:bg-[#ececec]' : 'cursor-default',
        busy && 'opacity-70'
      )}>
      <MessageFileMark name={iconName ?? name} className={MESSAGE_FILE_ICON_CLASS} />
      <span className="min-w-0 flex-1">
        <span className="block truncate text-[13px] text-[#171717]">{name}</span>
        {typeof sizeBytes === 'number' ? (
          <span className="mt-1 block text-[12px] text-[#8a8a8a]">{formatFileSize(sizeBytes)}</span>
        ) : null}
      </span>
    </button>
  )
}
