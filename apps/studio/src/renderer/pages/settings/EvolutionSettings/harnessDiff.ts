export type HarnessDiffKind = 'same' | 'added' | 'removed'

export interface HarnessDiffRow {
  kind: HarnessDiffKind
  text: string
}

/** Ordered assemble diff. Equal names stay in place; inserts and drops keep their neighbors. */
export function diffComponents(base: readonly string[], next: readonly string[]): HarnessDiffRow[] {
  const width = base.length
  const height = next.length
  const score: number[][] = Array.from({ length: width + 1 }, () => Array(height + 1).fill(0))
  for (let row = width - 1; row >= 0; row -= 1) {
    for (let column = height - 1; column >= 0; column -= 1) {
      score[row][column] =
        base[row] === next[column]
          ? score[row + 1][column + 1] + 1
          : Math.max(score[row + 1][column], score[row][column + 1])
    }
  }

  const rows: HarnessDiffRow[] = []
  let row = 0
  let column = 0
  while (row < width && column < height) {
    if (base[row] === next[column]) {
      rows.push({ kind: 'same', text: base[row] })
      row += 1
      column += 1
    } else if (score[row + 1][column] >= score[row][column + 1]) {
      rows.push({ kind: 'removed', text: base[row] })
      row += 1
    } else {
      rows.push({ kind: 'added', text: next[column] })
      column += 1
    }
  }
  while (row < width) {
    rows.push({ kind: 'removed', text: base[row] })
    row += 1
  }
  while (column < height) {
    rows.push({ kind: 'added', text: next[column] })
    column += 1
  }
  return rows
}
