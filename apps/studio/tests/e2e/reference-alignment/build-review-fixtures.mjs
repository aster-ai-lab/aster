import { mkdir, readFile, writeFile, copyFile } from 'node:fs/promises'
import process from 'node:process'
import { fileURLToPath, URL } from 'node:url'

import { Document, Packer, Paragraph, TextRun } from 'docx'

// Synthetic test data; expected answers stay outside the model's inputs directory.
const base = new URL('./cases/', import.meta.url)
const source = new URL('studio-proofreading/inputs/', base)
const fixtures = [
  [
    'review-basic',
    ['a.txt'],
    '只做基础校核，不润色。本地精确白名单为“按排”；白名单词不报错。',
    [['以经', '已经']],
    ['按排']
  ],
  ['review-clean', ['b.txt'], '只做基础校核，不润色。', [], []],
  [
    'review-official',
    ['notice.txt'],
    '只做公文校核，已选项为公文格式、行文规范。不要检查基础错字或网络安全。未知日期须标【待核实】，不能猜测新日期。',
    [
      ['2026年13月40日', '【待核实】'],
      ['报告', '通知']
    ],
    ['以经', '密码']
  ],
  [
    'review-security',
    ['security.txt'],
    '只做网络与数据安全校核，检查方案的实质问题。不要强加基础错字或公文格式问题。',
    [
      ['账号', ''],
      ['身份证', ''],
      ['备份', ''],
      ['日志', '']
    ],
    []
  ],
  ['review-unrelated', ['park.txt'], '只做网络与数据安全校核。内容不适用时不要强行找错。', [], []],
  [
    'review-mixed',
    ['notice.txt'],
    '混合校核，启用基础、公文、网络与数据安全三项；相同问题去重。未知日期须标【待核实】，不能猜测新日期。',
    [
      ['以经', '已经'],
      ['2026年13月40日', '【待核实】'],
      [['报告', '本通知'], '通知'],
      ['密码', '']
    ],
    []
  ],
  [
    'review-selected-b',
    ['b.txt', 'a.txt'],
    '只做基础校核，不润色。本地精确白名单为“按排”。两个附件尚未选择，请先让我明确选一个。',
    [['以经', '已经']],
    ['按排']
  ],
  [
    'review-ambiguous',
    ['a.txt', 'b.txt'],
    '做基础校核。两个附件尚未选择，请先让我明确选一个，不要自行选取或开始校核。',
    [],
    []
  ]
]
for (const [name, files, request, required, excluded] of fixtures) {
  if (process.argv[2] && process.argv[2] !== name) continue
  const dir = new URL(`${name}/`, base)
  await mkdir(new URL('inputs/', dir), { recursive: true })
  await mkdir(new URL('tests/', dir), { recursive: true })
  const paths = []
  for (const filename of files) {
    const text = await readFile(new URL(filename, source), 'utf8')
    const doc = new Document({
      creator: 'A3S synthetic acceptance fixture',
      styles: { default: { document: { run: { font: 'Noto Sans CJK SC', size: 24 } } } },
      sections: [
        {
          children: text
            .trimEnd()
            .split('\n')
            .map((line) => new Paragraph({ children: [new TextRun(line)] }))
        }
      ]
    })
    const docx =
      name === 'review-selected-b' ? (paths.length === 0 ? 'a.docx' : 'b.docx') : filename.replace('.txt', '.docx')
    await writeFile(new URL(`inputs/${docx}`, dir), await Packer.toBuffer(doc))
    paths.push(`{{WORKSPACE}}/inputs/${docx}`)
  }
  await writeFile(new URL('instruction.md', dir), `${request}\n${paths.join('\n')}\n`)
  await writeFile(
    new URL('tests/expected.json', dir),
    JSON.stringify(
      {
        name,
        required,
        excluded,
        ambiguous: name === 'review-ambiguous',
        ...(name === 'review-selected-b' ? { selected: 'b.docx', unselected: 'a.docx' } : {})
      },
      null,
      2
    ) + '\n'
  )
  if (name === 'review-selected-b') {
    await writeFile(
      new URL('tests/reply.json', dir),
      JSON.stringify({ choiceLabel: '{{WORKSPACE}}/inputs/b.docx' }, null, 2) + '\n'
    )
  }
  await copyFile(new URL('./review-host-test.sh', import.meta.url), new URL('tests/test.sh', dir))
  await copyFile(new URL('./review-host-grader.py', import.meta.url), new URL('tests/test_outputs.py', dir))
  process.stdout.write(`${fileURLToPath(dir)}\n`)
}
