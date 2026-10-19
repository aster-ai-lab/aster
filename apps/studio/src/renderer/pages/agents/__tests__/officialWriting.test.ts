import { describe, expect, it } from 'vitest'

import {
  isOfficialWritingHandoff,
  officialWritingCopyRequest,
  OFFICIAL_WRITING_TYPES,
  officialWritingBannerLine,
  officialWritingDraft,
  officialWritingPreview,
  officialWritingTaskText,
  writingChoiceOn
} from '../officialWriting'
import { officialWritingFindSlots, officialWritingTemplateHtml } from '../officialWritingTemplate'

function visible(html: string | null): string {
  return (html ?? '').replace(/<[^>]+>/g, '')
}

describe('officialWriting', () => {
  it('keeps section headings and fixed captions out of the slot list', () => {
    expect(officialWritingFindSlots('事由')).toEqual([{ key: 'subject', index: 0, text: '事由' }])
    expect(officialWritingFindSlots('一、事由')).toEqual([])
    expect(officialWritingFindSlots('一、事由与依据')).toEqual([])
    expect(officialWritingFindSlots('发文机关关于事由的通知')).toEqual([
      { key: 'issuer', index: 0, text: '发文机关' },
      { key: 'subject', index: '发文机关关于'.length, text: '事由' }
    ])
    expect(officialWritingFindSlots('待填写：事由与依据。')).toEqual([
      { key: 'section.事由与依据', index: 0, text: '待填写：事由与依据' }
    ])
    expect(officialWritingFindSlots('签发人：签发人')).toEqual([
      { key: 'signer', index: '签发人：'.length, text: '签发人' }
    ])
    expect(officialWritingFindSlots('出席：出席')).toEqual([{ key: 'attendees', index: '出席：'.length, text: '出席' }])
    expect(officialWritingFindSlots('来文事由')).toEqual([{ key: 'incoming_subject', index: 0, text: '来文事由' }])
  })

  it('builds a notice from the brief and keeps the closer', () => {
    const draft = officialWritingDraft('notice', '请教育局通知各校周五开会')
    expect(draft).toContain('请教育局通知各校周五开会')
    expect(draft).toContain('特此通知')
    expect(draft).toContain('主送机关')
  })

  it('closes a request with the approval line', () => {
    expect(officialWritingDraft('request', '申请经费')).toContain('妥否，请批示')
  })

  it('keeps minutes on the decision list', () => {
    const draft = officialWritingDraft('minutes', '本次会议')
    expect(draft).toContain('议定以下事项')
    expect(draft).not.toContain('特此通知')
  })

  it('returns null for an empty brief or an unknown type', () => {
    expect(officialWritingDraft('notice', '   ')).toBeNull()
    expect(officialWritingDraft('unknown', '事项')).toBeNull()
    expect(officialWritingTaskText('notice', '')).toBeNull()
  })

  it('puts the brief and the type name in the task', () => {
    const text = officialWritingTaskText('notice', '周五开会')
    expect(text).toContain('写作说明')
    expect(text).toContain('通知')
    expect(text).toContain('周五开会')
    expect(text).toContain('特此通知')
  })

  it('uses a red header and a document number on a notice unless the user turns the header off', () => {
    const draft = officialWritingDraft('notice', '周五开会')
    expect(draft).toContain('发文机关文件')
    expect(draft).toContain('文号：待填写')
    expect(draft).not.toContain('签发人')
    const plain = officialWritingDraft('notice', '周五开会', { banner: false })
    expect(plain).not.toContain('发文机关文件')
    expect(plain).toContain('特此通知')
  })

  it('keeps the signer on a request and the amount only when chosen', () => {
    expect(officialWritingDraft('request', '申请经费')).toContain('签发人：待填写')
    expect(officialWritingDraft('request', '申请经费')).not.toContain('金额：')
    expect(officialWritingDraft('request', '申请经费', { amount: true })).toContain('金额：待填写')
  })

  it('leaves the red header off a report until the user turns it on', () => {
    expect(officialWritingDraft('report', '本季度完成情况')).not.toContain('发文机关文件')
    expect(officialWritingDraft('report', '本季度完成情况', { banner: true })).toContain('发文机关文件')
  })

  it('lists attendance on minutes only while that choice stays on', () => {
    expect(officialWritingDraft('minutes', '本次会议')).toContain('出席：待填写')
    expect(officialWritingDraft('minutes', '本次会议')).toContain('发文机关会议纪要')
    expect(officialWritingDraft('minutes', '本次会议', { attendance: false, banner: false })).not.toContain('出席：')
    expect(officialWritingDraft('minutes', '本次会议', { banner: false })).not.toContain('发文机关会议纪要')
  })

  it('ignores a choice that does not belong to the type', () => {
    expect(writingChoiceOn('notice', 'amount', { amount: true })).toBe(false)
    expect(writingChoiceOn('notice', 'signer', {})).toBe(false)
  })

  it('names the chosen layout in the task', () => {
    const text = officialWritingTaskText('notice', '周五开会', { banner: false })
    expect(text).toContain('不使用红头')
    expect(text).toContain('保留文号')
  })

  it('writes the office, the recipient, and the document number into the draft', () => {
    const draft = officialWritingDraft(
      'letter',
      '商请协助',
      {},
      { org: '市教育局', mainTo: '市财政局', docNumber: '教函〔2026〕12号', subject: '协助培训' }
    )
    expect(draft).toContain('市教育局关于协助培训的函')
    expect(draft).toContain('市财政局：')
    expect(draft).toContain('教函〔2026〕12号')
    expect(draft).toContain('市教育局')
    expect(draft).not.toContain('发文机关文件')
  })

  it('keeps the optional blocks out of the draft until each switch is on', () => {
    const plain = officialWritingDraft('notice', '周五开会')
    expect(plain).not.toContain('000001')
    expect(plain).not.toContain('机密')
    expect(plain).not.toContain('印发机关')
    expect(plain).not.toContain('附注')
    const full = officialWritingDraft(
      'notice',
      '周五开会',
      { copyNo: true, secret: true, imprint: true, note: true },
      {
        copyNoText: '000018',
        secretText: '秘密★1年',
        printOrg: '市政府办公室',
        printDate: '2026年10月7日',
        noteText: '联系人：办公室'
      }
    )
    expect(full).toContain('000018')
    expect(full).toContain('秘密★1年')
    expect(full).toContain('印发机关：市政府办公室')
    expect(full).toContain('印发日期：2026年10月7日')
    expect(full).toContain('（联系人：办公室）')
  })

  it('seeds a placeholder template and fills the slots that have values', () => {
    const html = officialWritingTemplateHtml('notice', '')
    expect(visible(html)).toContain('发文机关文件')
    expect(html).toContain('data-writing-slot="issuer"')
    expect(html).toContain('data-office-run-border')
    expect(html).toContain('border: 1px dashed #595959')
    expect(html).not.toContain('【')
    expect(html).not.toContain('{{')
    expect(html).not.toContain('margin-top: 99pt')
    expect(html).toContain('data-section-margin-top="14"')
    expect(html).toContain('data-section-show-page-numbers="false"')
    expect(visible(html)).toContain('发文字号')
    expect(visible(html)).toContain('发文机关关于事由的通知')
    expect(visible(html)).toContain('主送机关：')
    expect(visible(html)).toContain('待填写：事由与依据')
    expect(html).toContain('特此通知。')
    expect(visible(html)).toContain('发文机关署名')
    expect(html).not.toContain('data-writing-slot="cc"')
    expect(html).not.toContain('data-writing-slot="copy_no"')

    const filled = officialWritingTemplateHtml(
      'notice',
      '周五开会',
      { banner: false },
      { org: '市教育局', mainTo: '各学校' }
    )
    expect(visible(filled)).not.toContain('发文机关文件')
    expect(visible(filled)).not.toContain('市教育局文件')
    expect(visible(filled)).toContain('市教育局关于事由的通知')
    expect(visible(filled)).toContain('各学校：')
    expect(visible(filled)).toContain('周五开会')
    expect(visible(filled)).not.toContain('待填写：事由与依据')
    expect(visible(filled)).toContain('待填写：具体事项')
    expect(filled).not.toContain('data-writing-slot="issuer"')
    expect(filled).toContain('data-writing-slot="subject"')
    expect(filled).toMatch(/<span style="[^"]*font-size: 22pt">市教育局<\/span>/)
    expect(visible(officialWritingTemplateHtml('notice', '', {}, { org: '<局>' }))).toContain('&lt;局&gt;文件')
    for (const type of OFFICIAL_WRITING_TYPES) {
      const page = officialWritingTemplateHtml(type.id, '')
      expect(page).not.toContain('{{')
      expect(page).not.toContain('【')
      expect(page).toContain('border: 1px dashed #595959')
    }
  })

  it('keeps letter, request, and minutes slots in the work template shape', () => {
    const letter = officialWritingTemplateHtml('letter', '')
    expect(visible(letter)).not.toContain('发文机关文件')
    expect(letter).toContain('data-writing-slot="letter_lead"')
    expect(visible(letter)).toContain('发文机关关于事由的函')
    expect(visible(letter)).toContain('待填写：事由')
    expect(letter).toContain('特此函达，请予支持为荷。')
    const banner = officialWritingTemplateHtml('letter', '', { banner: true })
    expect(banner).toContain('font-size: 48pt')
    expect(visible(banner)).not.toContain('发文机关文件')

    const request = officialWritingTemplateHtml('request', '')
    expect(visible(request)).toContain('签发人：')
    expect(request).toContain('data-writing-slot="signer"')
    expect(visible(request)).toContain('为请示目的，现请示如下：')
    expect(request).toContain('妥否，请批示。')
    expect(request).not.toContain('data-writing-slot="amount"')
    expect(visible(officialWritingTemplateHtml('request', '', { amount: true }))).toContain('金额：金额')

    const minutes = officialWritingTemplateHtml('minutes', '')
    expect(visible(minutes)).toContain('发文机关纪要')
    expect(visible(minutes)).toContain('关于事由的会议纪要')
    expect(visible(minutes)).toContain('会议于会议时间召开')
    expect(minutes).toContain('data-writing-slot="attendees"')
    expect(minutes).toContain('data-writing-slot="absentees"')
    expect(minutes).toContain('data-writing-slot="observers"')

    const edition = officialWritingTemplateHtml('notice', '', { copyTo: true, imprint: true, secret: true, note: true })
    expect(visible(edition)).toContain('密级和保密期限')
    expect(visible(edition)).toContain('抄送：抄送机关。')
    expect(edition).toContain('data-writing-slot="publisher"')
    expect(visible(edition)).toContain('印发日期印发')
    expect(visible(edition)).toContain('（附注）')
    expect(officialWritingTemplateHtml('unknown', '')).toBeNull()
  })

  it('previews the skeleton before a brief is written', () => {
    const preview = officialWritingPreview('letter', '  ')
    expect(preview).toContain('文号：待填写')
    expect(preview).toContain('（待填写）')
    expect(preview).not.toContain('发文机关文件')
    expect(officialWritingBannerLine('notice')).toBe('发文机关文件')
    expect(officialWritingBannerLine('notice', { banner: false })).toBeNull()
    expect(officialWritingBannerLine('minutes')).toBe('发文机关会议纪要')
    expect(officialWritingBannerLine('notice', {}, { org: '市教育局' })).toBe('市教育局文件')
  })

  it('carries a filled recipient into the task', () => {
    const text = officialWritingTaskText('notice', '周五开会', {}, { mainTo: '各学校' })
    expect(text).toContain('各学校：')
    expect(text).toContain('加盖印章')
  })

  it('accepts a real handoff and rejects an empty or oversized one', () => {
    expect(isOfficialWritingHandoff({ text: '一份通知' })).toBe(true)
    expect(isOfficialWritingHandoff({ text: '  ' })).toBe(false)
    expect(isOfficialWritingHandoff({ text: 'x'.repeat(8001) })).toBe(false)
    expect(isOfficialWritingHandoff(null)).toBe(false)
    expect(OFFICIAL_WRITING_TYPES).toHaveLength(11)
    const handoff = {
      text: '一份通知',
      typeId: 'notice',
      brief: '周五开会',
      choices: { banner: false, unknown: true },
      fields: { org: '市教育局', extra: 1 }
    }
    expect(officialWritingCopyRequest(handoff)).toEqual({
      typeId: 'notice',
      brief: '周五开会',
      choices: { banner: false },
      fields: { org: '市教育局' }
    })
    expect(officialWritingCopyRequest({ text: '一份通知' })).toBeNull()
    expect(officialWritingCopyRequest({ text: '一份通知', typeId: 'missing', brief: '周五开会' })).toBeNull()
  })
})
