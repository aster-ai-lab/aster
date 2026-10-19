import { describe, expect, it, vi } from 'vitest'

import type { ReviewFinding } from '../reviewFindings'
import { anchorReviewFindings, appendReviewComments } from '../ReviewOfficeDocument'

const payment: ReviewFinding = {
  id: 'pay',
  title: '付款期限',
  summary: '期限过短',
  evidenceQuote: '违约金为合同总额。',
  guidance: '改为书面通知'
}

const repeated: ReviewFinding = {
  id: 'repeat',
  title: '重复条款',
  summary: '同一句出现两次',
  evidenceQuote: '甲方应在三日内付款。',
  guidance: '保留一处'
}

function documentEditor(text: string) {
  const marks: Array<{ id: string }> = []
  const metas: string[] = []
  let transactions = 0
  const perFinding = vi.fn(() => true)
  const editor = {
    isDestroyed: false,
    state: {
      doc: {
        descendants(visitor: (node: { isText: boolean; text?: string }, pos: number) => void) {
          visitor({ isText: true, text }, 1)
        }
      }
    },
    commands: { insertDocumentCommentThread: perFinding },
    chain() {
      const transaction = {
        docChanged: false,
        addMark(_from: number, _to: number, mark: { id: string }) {
          marks.push(mark)
          this.docChanged = true
          return this
        },
        setMeta(key: string) {
          metas.push(key)
        }
      }
      return {
        command(command: (props: unknown) => boolean) {
          return {
            run() {
              transactions += 1
              return command({
                tr: transaction,
                state: {
                  schema: {
                    marks: {
                      documentComment: { create: (attrs: { id: string }) => attrs }
                    }
                  }
                },
                dispatch() {
                  return undefined
                }
              })
            }
          }
        },
        focus() {
          return this
        }
      }
    }
  }
  return { editor, marks, metas, perFinding, transactions: () => transactions }
}

describe('review anchors', () => {
  it('places every unique quote in one transaction and skips a repeated quote', () => {
    const text = '甲方应在三日内付款。违约金为合同总额。甲方应在三日内付款。'
    const { editor, marks, metas, perFinding, transactions } = documentEditor(text)
    const applied = new Set<string>()

    const anchored = anchorReviewFindings(editor, [payment, repeated], applied)

    expect(anchored.map((finding) => finding.id)).toEqual(['pay'])
    expect(marks).toEqual([{ id: 'pay' }])
    expect(metas).toEqual(['preventUpdate'])
    expect(transactions()).toBe(1)
    expect(perFinding).not.toHaveBeenCalled()
    expect(applied).toEqual(new Set(['pay']))

    anchorReviewFindings(editor, [payment, repeated], applied)

    expect(transactions()).toBe(1)
    expect(marks).toHaveLength(1)
  })

  it('keeps the imported document when comments are recorded', () => {
    const model = { revision: 3 }
    const content = {
      type: 'document' as const,
      html: '<p>违约金为合同总额。</p>',
      model,
      comments: [] as Array<{ id: string }>
    }

    const next = appendReviewComments(content, [payment], '2026-10-06T00:00:00.000Z')

    expect(next.html).toBe(content.html)
    expect(next.model).toBe(model)
    expect(next.comments).toEqual([
      {
        id: 'pay',
        author: '风险',
        date: '2026-10-06T00:00:00.000Z',
        resolved: false,
        text: '【付款期限】\n期限过短\n证据：违约金为合同总额。\n建议：改为书面通知'
      }
    ])
    expect(appendReviewComments(next, [payment], '2026-10-06T00:00:00.000Z')).toBe(next)
  })
})
