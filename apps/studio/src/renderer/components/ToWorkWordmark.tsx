import { cn } from '@renderer/utils/style'

/** Geometric ToWork wordmark: one circle, one stroke, flat terminals, #303030. */
export function ToWorkWordmark({ className, drawn = false }: { className?: string; drawn?: boolean }) {
  const stroke = (name: string) => (drawn ? { className: `towork-stroke ${name}`, pathLength: 1 } : {})
  return (
    <svg
      data-ui={drawn ? 'page.loader.wordmark' : 'workbench.wordmark.mark'}
      aria-hidden="true"
      viewBox="140 116 2164 512"
      className={cn('block h-12 w-auto', drawn && 'towork-wordmark-draw', className)}
      fill="none"
      stroke={drawn ? 'currentColor' : '#303030'}
      strokeWidth={76}
      strokeLinecap="butt"
      strokeLinejoin="miter"
      strokeMiterlimit={16}>
      <path {...stroke('is-t')} d="M156 170H532M344 170V612" />
      <circle {...stroke('is-o1')} cx="606" cy="432" r="142" transform={drawn ? 'rotate(-90 606 432)' : undefined} />
      <path {...stroke('is-w')} d="M868 233.4L980 510.6L1091.9 233.4L1203.9 510.6L1315.9 233.4" />
      <circle
        {...stroke('is-o2')}
        cx="1577.9"
        cy="432"
        r="142"
        transform={drawn ? 'rotate(-90 1577.9 432)' : undefined}
      />
      <path {...stroke('is-r')} d="M1839.9 612V290H1957.7" />
      <path {...stroke('is-k')} d="M2053.7 132V612M2053.7 396L2249.7 288M2053.7 396L2249.7 582" />
    </svg>
  )
}
