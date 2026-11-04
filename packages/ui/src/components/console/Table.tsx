import type { ReactNode } from 'react'
import { cn } from '../../lib/cn'
import {
  Table as PrimitiveTable,
  TableBody,
  TableHead,
  TableHeader,
  TableRow,
} from '../primitives/table'
import { Card } from './Card'

/**
 * Compact column-header table on shared Arc table primitives + Console Card.
 * Prefer composing `Table` / `TableHeader` / `TableRow` from primitives for new surfaces.
 */
export function Table({
  columns,
  children,
  className = '',
}: {
  columns: string[]
  children: ReactNode
  className?: string
}) {
  return (
    <Card className={cn('overflow-x-auto', className)}>
      <PrimitiveTable className="min-w-[640px] border-collapse text-left">
        <TableHeader className="bg-bg-elevated text-xs text-ink-faint [&_tr]:border-b [&_tr]:border-[color:var(--color-hairline)]">
          <TableRow className="hover:bg-transparent data-[state=selected]:bg-transparent">
            {columns.map((col) => (
              <TableHead key={col} className="h-9 px-3 font-normal text-ink-faint">
                {col}
              </TableHead>
            ))}
          </TableRow>
        </TableHeader>
        <TableBody className="text-ink-muted [&>tr+tr]:border-t [&>tr+tr]:border-[color:var(--color-hairline)] [&>tr:hover]:bg-surface-2">
          {children}
        </TableBody>
      </PrimitiveTable>
    </Card>
  )
}
