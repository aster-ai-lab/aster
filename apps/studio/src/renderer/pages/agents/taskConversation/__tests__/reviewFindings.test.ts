import { describe, expect, it } from 'vitest'

import {
  docxPathFromReviewPrompt,
  parseReviewFindings,
  publishReviewDocument,
  getReviewDocument,
  subscribeReviewDocument,
  reviewDocumentFromConversation
} from '../reviewFindings'

describe('review findings', () => {
  it('uses only validated host progress for the bound source and preserves partial results on stop', () => {
    const source = {
      type: 'data-review-source' as const,
      data: { kind: 'document' as const, path: '/tmp/合同.docx', sha256: 'a'.repeat(64) }
    }
    const progress = { type: 'data-review-progress' as const, data: { index: 1, total: 2, quote: '甲方付款' } }
    const messages = [
      { id: 'u', role: 'user' },
      { id: 'a', role: 'assistant', status: 'pending' }
    ]
    const parts = {
      u: [{ type: 'text' as const, text: '使用智能体 合同审查：\n/tmp/合同.docx' }],
      a: [source, progress]
    }
    expect(reviewDocumentFromConversation(messages, parts)).toMatchObject({
      phase: 'reviewing',
      progress: progress.data,
      findings: []
    })
    expect(
      reviewDocumentFromConversation([{ id: 'a', role: 'assistant', metadata: { status: 'pending' } }], parts)
    ).toMatchObject({ phase: 'reviewing' })
    expect(reviewDocumentFromConversation([{ ...messages[1], status: 'paused' }], parts)).toMatchObject({
      phase: 'cancelled',
      path: source.data.path
    })
    expect(
      reviewDocumentFromConversation(messages, {
        ...parts,
        a: [source, { ...progress, data: { ...progress.data, index: 3 } }]
      })?.progress
    ).toBeUndefined()
  })
  it('keeps the failed source visible without binding old bytes or findings', () => {
    for (const next of ['/tmp/old.docx', '/tmp/missing.docx']) {
      expect(
        reviewDocumentFromConversation(
          [
            { id: 'u1', role: 'user' },
            { id: 'a1', role: 'assistant' },
            { id: 'u2', role: 'user' },
            { id: 'a2', role: 'assistant' }
          ],
          {
            u1: [{ type: 'text', text: '/review Check\n/tmp/old.docx' }],
            a1: [
              { type: 'data-review-source', data: { kind: 'document', path: '/tmp/old.docx', sha256: 'a'.repeat(64) } }
            ],
            u2: [{ type: 'text', text: `/review Check\n${next}` }],
            a2: [
              { type: 'data-error', data: { name: 'Error', message: 'The selected Word document could not be read.' } }
            ]
          }
        )
      ).toMatchObject({ path: next, phase: 'failed', findings: [] })
    }
  })

  it('keeps accepted partial findings bound to their verified source when a later segment fails', () => {
    const finding = { id: 'f', title: 'Typo', evidenceQuote: '以经', summary: 'Typo', guidance: '已经' }
    expect(
      reviewDocumentFromConversation(
        [
          { id: 'u', role: 'user' },
          { id: 'a', role: 'assistant' }
        ],
        {
          u: [{ type: 'text', text: '/review Check\n/tmp/source.docx' }],
          a: [
            {
              type: 'data-review-source',
              data: { kind: 'document', path: '/tmp/source.docx', sha256: 'b'.repeat(64) }
            },
            { type: 'text', text: `\`\`\`a3s-review-finding\n${JSON.stringify(finding)}\n\`\`\`` },
            { type: 'data-error', data: { name: 'Error', message: 'A later segment failed.' } }
          ]
        }
      )
    ).toEqual({ path: '/tmp/source.docx', revision: 'b'.repeat(64), findings: [finding] })
  })

  it('publishes a new version at the same path even when the findings are unchanged', () => {
    publishReviewDocument({ path: '/tmp/B.docx', revision: 'a'.repeat(64), findings: [] })
    const revisions: Array<string | undefined> = []
    const unsubscribe = subscribeReviewDocument(() => revisions.push(getReviewDocument()?.revision))
    publishReviewDocument({ path: '/tmp/B.docx', revision: 'b'.repeat(64), findings: [] })
    expect(revisions).toEqual(['b'.repeat(64)])
    expect(getReviewDocument()?.revision).toBe('b'.repeat(64))
    unsubscribe()
    publishReviewDocument(null)
  })
  it('opens only the host-selected document after ambiguity, including a clean review', () => {
    const messages = [
      { id: 'old', role: 'user' },
      { id: 'choose', role: 'user' },
      { id: 'answer', role: 'assistant' }
    ]
    const source = {
      type: 'data-review-source' as const,
      data: { kind: 'document' as const, path: '/tmp/B/合同.docx', sha256: 'b'.repeat(64) }
    }
    const parts = {
      old: [{ type: 'text' as const, text: '/review 检查\n/tmp/old.docx' }],
      choose: [{ type: 'text' as const, text: '/review 请选择\n/tmp/A/合同.docx\n/tmp/B/合同.docx' }],
      answer: [source, { type: 'text' as const, text: '[]' }]
    }
    expect(reviewDocumentFromConversation(messages, parts)).toEqual({
      path: '/tmp/B/合同.docx',
      revision: 'b'.repeat(64),
      findings: []
    })
    const finding = { id: 'b', title: '错字', evidenceQuote: '以经', summary: '错字', guidance: '已经' }
    expect(
      reviewDocumentFromConversation(messages, {
        ...parts,
        answer: [source, { type: 'text', text: `\`\`\`a3s-review-finding\n${JSON.stringify(finding)}\n\`\`\`` }]
      })
    ).toEqual({ path: '/tmp/B/合同.docx', revision: 'b'.repeat(64), findings: [finding] })
  })

  it('clears earlier findings when the same path is reviewed again', () => {
    expect(
      reviewDocumentFromConversation(
        [
          { id: 'u', role: 'user' },
          { id: 'a', role: 'assistant' },
          { id: 'again', role: 'assistant' }
        ],
        {
          u: [{ type: 'text', text: '/review 检查\n/tmp/B.docx' }],
          a: [
            {
              type: 'text',
              text: '```a3s-review-finding\n{"id":"old","title":"旧问题","evidenceQuote":"定金","summary":"旧","guidance":"修改"}\n```'
            }
          ],
          again: [
            { type: 'data-review-source', data: { kind: 'document', path: '/tmp/B.docx', sha256: 'b'.repeat(64) } },
            { type: 'text', text: '[]' }
          ]
        }
      )
    ).toEqual({ path: '/tmp/B.docx', revision: 'b'.repeat(64), findings: [] })
  })

  it('removes Word actions when a new pasted-text review follows a document', () => {
    expect(
      reviewDocumentFromConversation(
        [
          { id: 'old', role: 'user' },
          { id: 'text', role: 'user' },
          { id: 'answer', role: 'assistant' }
        ],
        {
          old: [{ type: 'text', text: '/review 检查\n/tmp/old.docx' }],
          text: [
            { type: 'text', text: '只做基础校核' },
            { type: 'data-review-source', data: { kind: 'text', text: '以经完成。', sha256: 'a'.repeat(64) } }
          ],
          answer: [
            {
              type: 'text',
              text: '```a3s-review-finding\n{"id":"f","title":"错字","evidenceQuote":"以经","summary":"用字错误","guidance":"已经"}\n```'
            }
          ]
        }
      )
    ).toBeNull()
  })
  it('preserves an exact quote containing Markdown fence characters', () => {
    const finding = {
      id: 'code',
      title: '示例',
      evidenceQuote: '请删除示例```口令```。',
      summary: '含口令示例',
      guidance: '删除口令示例。'
    }
    expect(parseReviewFindings(`\`\`\`a3s-review-finding\n${JSON.stringify(finding)}\n\`\`\``)).toEqual([finding])
  })

  it('reads one finding fence and ignores prose around it', () => {
    const findings = parseReviewFindings(
      [
        '审查结果如下。',
        '```a3s-review-finding',
        '{"id":"f1","title":"自动作废","evidenceQuote":"合同自动作废","summary":"解除过重","guidance":"改为书面通知"}',
        '```'
      ].join('\n')
    )
    expect(findings).toEqual([
      {
        id: 'f1',
        title: '自动作废',
        evidenceQuote: '合同自动作废',
        summary: '解除过重',
        guidance: '改为书面通知'
      }
    ])
  })

  it('keeps the latest review document and its findings', () => {
    const document = reviewDocumentFromConversation(
      [
        { id: 'u1', role: 'user' },
        { id: 'a1', role: 'assistant' }
      ],
      {
        u1: [{ type: 'text', text: '/review 审查这个文件\n/tmp/合同.docx' }],
        a1: [
          {
            type: 'text',
            text: '```a3s-review-finding\n{"id":"f1","title":"风险","evidenceQuote":"定金","summary":"未区分","guidance":"补定义"}\n```'
          }
        ]
      }
    )
    expect(document).toEqual({
      path: '/tmp/合同.docx',
      findings: [
        {
          id: 'f1',
          title: '风险',
          evidenceQuote: '定金',
          summary: '未区分',
          guidance: '补定义'
        }
      ]
    })
    expect(docxPathFromReviewPrompt('/help')).toBeNull()
    expect(docxPathFromReviewPrompt('使用智能体 写作助手：\n/tmp/合同.docx')).toBeNull()
  })

  it('does not open a review document for another agent mention without findings', () => {
    const text = '使用智能体 写作助手：\n请看这份稿\n/tmp/合同.docx'
    expect(
      reviewDocumentFromConversation([{ id: 'u1', role: 'user' }], {
        u1: [{ type: 'text', text }]
      })
    ).toBeNull()
    expect(
      reviewDocumentFromConversation(
        [
          { id: 'u1', role: 'user' },
          { id: 'a1', role: 'assistant' }
        ],
        {
          u1: [{ type: 'text', text }],
          a1: [{ type: 'text', text: '这是一份写作稿。' }]
        }
      )
    ).toBeNull()
  })

  it('opens the Word path when an agent mention is followed by finding fences', () => {
    const document = reviewDocumentFromConversation(
      [
        { id: 'u1', role: 'user' },
        { id: 'a1', role: 'assistant' }
      ],
      {
        u1: [{ type: 'text', text: '使用智能体 写作助手：\n请看\n/tmp/合同.docx' }],
        a1: [
          {
            type: 'text',
            text: '```a3s-review-finding\n{"id":"f1","title":"风险","evidenceQuote":"定金","summary":"未区分","guidance":"补定义"}\n```'
          }
        ]
      }
    )
    expect(document?.path).toBe('/tmp/合同.docx')
    expect(document?.findings).toHaveLength(1)
  })

  it('does not attach ambiguous new findings to the first candidate or an older review', () => {
    const finding =
      '```a3s-review-finding\n{"id":"f1","title":"风险","evidenceQuote":"定金","summary":"未区分","guidance":"补定义"}\n```'
    const ambiguous = '/review 检查这个文件\n/tmp/甲.docx\n/tmp/乙.docx'
    expect(docxPathFromReviewPrompt(ambiguous)).toBeNull()
    expect(docxPathFromReviewPrompt('/review 检查这个文件\n/tmp/甲.docx\n/tmp/甲.docx')).toBe('/tmp/甲.docx')
    expect(
      reviewDocumentFromConversation(
        [
          { id: 'u1', role: 'user' },
          { id: 'a1', role: 'assistant' },
          { id: 'u2', role: 'user' },
          { id: 'a2', role: 'assistant' }
        ],
        {
          u1: [{ type: 'text', text: '/review 审查\n/tmp/旧合同.docx' }],
          a1: [{ type: 'text', text: finding }],
          u2: [{ type: 'text', text: ambiguous }],
          a2: [{ type: 'text', text: finding }]
        }
      )
    ).toBeNull()
    const selected = reviewDocumentFromConversation(
      [
        { id: 'u1', role: 'user' },
        { id: 'a1', role: 'assistant' },
        { id: 'u2', role: 'user' },
        { id: 'a2', role: 'assistant' }
      ],
      {
        u1: [{ type: 'text', text: '/review 审查\n/tmp/旧合同.docx' }],
        a1: [{ type: 'text', text: finding }],
        u2: [{ type: 'text', text: '使用智能体 法务专家：\n审查新文件\n/tmp/新合同.docx' }],
        a2: [{ type: 'text', text: finding }]
      }
    )
    expect(selected?.path).toBe('/tmp/新合同.docx')
    expect(selected?.findings).toHaveLength(1)
  })
})
