import { describe, expect, it } from 'vitest'

import { codeBranchName, latestTurnChanges, resolveCodeBranch, type CodeTurn } from '../codeMode'

const edit = (path: string, after: string): CodeTurn['tools'][number] => ({
  name: 'edit_file',
  part: { input: { file_path: path, old_string: 'a', new_string: after } }
})

describe('code work mode', () => {
  it('builds an ASCII branch and falls back when the topic has no ASCII words', () => {
    expect(codeBranchName('Fix the login button now', 'AB12')).toBe('code/fix-the-login-button-ab12')
    expect(codeBranchName('修复登录', 'ab12')).toBe('code/task-ab12')
    expect(codeBranchName('..', '../')).toBe('code/task-task')
    expect(codeBranchName('Fix login', 'zz99')).not.toContain('..')
  })

  it('keeps a typed branch and rejects one git would refuse', () => {
    expect(resolveCodeBranch('Fix login', 'ab12', '')).toBe('code/fix-login-ab12')
    expect(resolveCodeBranch('Fix login', 'ab12', '  feature/billing  ')).toBe('feature/billing')
    expect(resolveCodeBranch('Fix login', 'ab12', 'bad..name')).toBeNull()
    expect(resolveCodeBranch('修复登录', 'ab12', '')).toBe('code/task-ab12')
  })

  it('keeps edits from the latest turn and drops a git pull', () => {
    const turns: CodeTurn[] = [
      { role: 'user', tools: [] },
      { role: 'assistant', tools: [edit('src/old.ts', 'old')] },
      { role: 'user', tools: [] },
      {
        role: 'assistant',
        tools: [
          edit('src/new.ts', 'first'),
          edit('src/new.ts', 'second'),
          {
            name: 'bash',
            part: {
              output:
                'diff --git a/src/pulled.ts b/src/pulled.ts\n--- a/src/pulled.ts\n+++ b/src/pulled.ts\n@@ -1 +1 @@\n-a\n+b\n'
            }
          }
        ]
      }
    ]

    const changes = latestTurnChanges(turns)
    expect(changes.map((diff) => diff.path)).toEqual(['src/new.ts'])
    expect(changes[0]?.lines.some((line) => line.text === 'second')).toBe(true)
    expect(changes[0]?.lines.some((line) => line.text === 'first')).toBe(false)
  })
})
