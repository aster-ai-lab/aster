import { studioProductMark } from '@renderer/tenantBrand'
import { cn } from '@renderer/utils/style'

/** 未设置头像时使用当前应用图标。尺寸由调用方决定，外形跟图标本身走。 */
export function DefaultAvatar({ label, className }: { label?: string; className?: string }) {
  return (
    <span
      data-ui="default-avatar"
      role="img"
      aria-label={label || undefined}
      aria-hidden={label ? undefined : true}
      className={cn('inline-flex shrink-0', className)}>
      <img src={studioProductMark()} alt="" draggable={false} className="h-full w-full object-contain" />
    </span>
  )
}
