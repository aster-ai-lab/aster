// Two distinct inputs for every turn class the workbench conversation projects.
// Plain data: the render suite and the end-to-end page share this table.

export interface TaskConversationPart {
  type: string
  text?: string
  state?: string
  toolCallId?: string
  toolName?: string
  input?: Record<string, unknown>
  output?: unknown
  errorText?: string
  mediaType?: string
  url?: string
  sourceId?: string
  title?: string
  files?: string[]
}

export interface LocaleCopy {
  language: 'zh-CN' | 'en-US'
  running: string
  done: string
  failed: string
  diagramFailed: string
  diffPending: string
  flowchart: string
  sequence: string
  products: string
  copy: string
  split: string
  unified: string
}

export const localeCopy: readonly LocaleCopy[] = [
  {
    language: 'zh-CN',
    running: '执行中',
    done: '已完成',
    failed: '失败',
    diagramFailed: '图表未能渲染',
    diffPending: '差异待生成',
    flowchart: '流程图',
    sequence: '时序图',
    products: '工作产物',
    copy: '复制代码',
    split: '分栏',
    unified: '统一'
  },
  {
    language: 'en-US',
    running: 'Running',
    done: 'Done',
    failed: 'Failed',
    diagramFailed: 'The diagram could not be rendered',
    diffPending: 'Diff pending',
    flowchart: 'Flowchart',
    sequence: 'Sequence',
    products: 'Work products',
    copy: 'Copy code',
    split: 'Split',
    unified: 'Unified'
  }
]

function text(value: string): TaskConversationPart {
  return { type: 'text', text: value }
}

function bash(
  id: string,
  command: string,
  state: 'input-streaming' | 'output-available' | 'output-error',
  stdout?: string
): TaskConversationPart {
  return {
    type: 'tool-bash',
    toolCallId: id,
    state,
    input: { command },
    ...(stdout !== undefined ? { output: { stdout } } : {}),
    ...(state === 'output-error' ? { errorText: `${command} failed` } : {})
  }
}

export const arrivalCases = [
  {
    id: 'alpha-directory',
    leading: '先看目录。',
    command: 'pwd',
    trailing: '目录在这。',
    parts: [text('先看目录。'), bash('bash-alpha', 'pwd', 'output-available', 'alpha-dir'), text('目录在这。')]
  },
  {
    id: 'beta-log',
    leading: 'Check the log.',
    command: 'whoami',
    trailing: 'Log is ready.',
    parts: [text('Check the log.'), bash('bash-beta', 'whoami', 'output-available', 'beta-user'), text('Log is ready.')]
  }
] as const

export const toolStateCases = [
  { id: 'running-pwd', state: 'running' as const, command: 'pwd', parts: [bash('run-pwd', 'pwd', 'input-streaming')] },
  {
    id: 'running-whoami',
    state: 'running' as const,
    command: 'whoami',
    parts: [bash('run-who', 'whoami', 'input-streaming')]
  },
  {
    id: 'done-alpha',
    state: 'success' as const,
    command: 'ls alpha',
    firstLine: 'alpha.txt',
    rest: 'alpha-more.txt',
    parts: [bash('done-alpha', 'ls alpha', 'output-available', 'alpha.txt\nalpha-more.txt')]
  },
  {
    id: 'done-beta',
    state: 'success' as const,
    command: 'cat beta',
    firstLine: 'beta.log',
    rest: 'beta-more.log',
    parts: [bash('done-beta', 'cat beta', 'output-available', 'beta.log\nbeta-more.log')]
  },
  {
    id: 'failed-git',
    state: 'error' as const,
    command: 'git status',
    parts: [bash('fail-git', 'git status', 'output-error')]
  },
  {
    id: 'failed-npm',
    state: 'error' as const,
    command: 'npm test',
    parts: [bash('fail-npm', 'npm test', 'output-error')]
  }
]

export const fenceCases = [
  {
    id: 'ts-alpha',
    language: 'ts',
    source: 'const alpha = 1',
    parts: [text('甲段代码。\n\n```ts\nconst alpha = 1\n```')]
  },
  { id: 'py-beta', language: 'py', source: 'beta = 2', parts: [text('乙段代码。\n\n```py\nbeta = 2\n```')] }
]

export const incompleteFenceCases = [
  { id: 'open-ts', source: 'const alpha = 1', parts: [text('```ts\nconst alpha = 1')] },
  { id: 'open-py', source: 'beta = 2', parts: [text('```py\nbeta = 2')] }
]

export const emptyFenceCases = [
  { id: 'empty-ts', leading: '开头甲', trailing: '结尾甲', parts: [text('开头甲\n\n```ts\n\n```\n\n结尾甲')] },
  { id: 'empty-py', leading: '开头乙', trailing: '结尾乙', parts: [text('开头乙\n\n```py\n\n```\n\n结尾乙')] }
]

export const diagramCases = [
  {
    id: 'flowchart-alpha',
    kind: 'flowchart' as const,
    source: 'Start-->End',
    header: 'flowchart LR',
    parts: [text('```mermaid\nflowchart LR\nStart-->End\n```')]
  },
  {
    id: 'sequence-beta',
    kind: 'sequence' as const,
    source: 'Alice->>Bob: Ping',
    header: 'sequenceDiagram',
    parts: [text('```mermaid\nsequenceDiagram\nAlice->>Bob: Ping\n```')]
  }
]

export const openDiagramCases = [
  { id: 'open-flow', source: 'Start-->End', parts: [text('```mermaid\nflowchart LR\nStart-->End')] },
  { id: 'open-sequence', source: 'Alice->>Bob: Ping', parts: [text('```mermaid\nsequenceDiagram\nAlice->>Bob: Ping')] }
]

export const diffCases = [
  {
    id: 'alpha-ts',
    file: 'alpha.ts',
    path: 'src/alpha.ts',
    removed: 'let n = 1',
    added: 'let n = 2',
    parts: [
      {
        type: 'tool-edit',
        toolCallId: 'diff-alpha',
        state: 'output-available',
        input: { path: 'src/alpha.ts', old_string: 'let n = 1', new_string: 'let n = 2' },
        output: 'ok'
      }
    ]
  },
  {
    id: 'beta-py',
    file: 'beta.py',
    path: 'lib/beta.py',
    removed: 'value = 0',
    added: 'value = 1',
    parts: [
      {
        type: 'tool-edit',
        toolCallId: 'diff-beta',
        state: 'output-available',
        input: { path: 'lib/beta.py', old_string: 'value = 0', new_string: 'value = 1' },
        output: 'ok'
      }
    ]
  }
]

export const pathOnlyCases = [
  {
    id: 'alpha-md',
    file: 'alpha.md',
    path: 'docs/alpha.md',
    parts: [
      {
        type: 'tool-edit',
        toolCallId: 'path-alpha',
        state: 'output-available',
        input: { path: 'docs/alpha.md' },
        output: 'ok'
      }
    ]
  },
  {
    id: 'beta-txt',
    file: 'beta.txt',
    path: 'notes/beta.txt',
    parts: [
      {
        type: 'tool-edit',
        toolCallId: 'path-beta',
        state: 'output-available',
        input: { path: 'notes/beta.txt' },
        output: 'ok'
      }
    ]
  }
]

export const productCases = [
  {
    id: 'write-input-file-path',
    path: '/tmp/alpha/src/alpha.ts',
    name: 'alpha.ts',
    hidden: '/tmp/alpha/secret.md',
    parts: [
      {
        type: 'dynamic-tool',
        toolName: 'Write',
        toolCallId: 'write-alpha',
        state: 'output-available',
        input: { file_path: '/tmp/alpha/src/alpha.ts', content: 'let n = 2' },
        output: 'ok'
      },
      {
        type: 'dynamic-tool',
        toolName: 'Read',
        toolCallId: 'read-alpha',
        state: 'output-available',
        input: { file_path: '/tmp/alpha/secret.md' },
        output: { filePath: '/tmp/alpha/secret.md' }
      }
    ],
    failed: [
      {
        type: 'dynamic-tool',
        toolName: 'Edit',
        toolCallId: 'fail-alpha',
        state: 'output-error',
        input: { file_path: '/tmp/alpha/src/alpha.ts' },
        errorText: 'disk full'
      }
    ]
  },
  {
    id: 'edit-output-file-path',
    path: 'lib/beta.py',
    name: 'beta.py',
    hidden: '/tmp/beta/unread.log',
    parts: [
      {
        type: 'tool-Edit',
        toolCallId: 'edit-beta',
        state: 'output-available',
        input: { old_string: 'value = 0', new_string: 'value = 1' },
        output: { filePath: 'lib/beta.py' }
      },
      {
        type: 'tool-Read',
        toolCallId: 'read-beta',
        state: 'output-available',
        input: { file_path: '/tmp/beta/unread.log' },
        output: { filePath: '/tmp/beta/unread.log' }
      }
    ],
    failed: [
      {
        type: 'tool-Edit',
        toolCallId: 'fail-beta',
        state: 'output-error',
        input: { old_string: 'value = 0', new_string: 'value = 1' },
        output: { filePath: 'lib/beta.py' },
        errorText: 'disk full'
      }
    ]
  }
]

export const mediaCases = [
  {
    id: 'prose-then-image',
    prose: '答案甲在这',
    reasoning: '先想甲',
    dropped: '[cite:alpha-drop]',
    kept: '[cite:keep-alpha]',
    image: 'https://files.example/alpha.png',
    order: ['prose', 'reasoning', 'image', 'answer'] as const,
    parts: [
      text('前文甲。'),
      { type: 'reasoning', text: '先想甲 [cite:alpha-drop]', state: 'done' },
      { type: 'file', mediaType: 'image/png', url: 'https://files.example/alpha.png' },
      text('答案甲在这 [cite:alpha-drop]。\n\n```ts\nconst marker = "[cite:keep-alpha]"\n```')
    ]
  },
  {
    id: 'image-then-prose',
    prose: '答案乙在这',
    reasoning: '先想乙',
    dropped: '[cite:beta-drop]',
    kept: '[cite:keep-beta]',
    image: 'https://files.example/beta.png',
    order: ['image', 'prose', 'reasoning', 'answer'] as const,
    parts: [
      { type: 'file', mediaType: 'image/png', url: 'https://files.example/beta.png' },
      text('前文乙 [cite:beta-drop]。'),
      { type: 'reasoning', text: '先想乙 [cite:beta-drop]', state: 'done' },
      text('答案乙在这。\n\n```py\nmarker = "[cite:keep-beta]"\n```')
    ]
  }
]

export const workspaceOpenCases = [
  {
    id: 'alpha-notes',
    workspacePath: '/tmp/alpha',
    productPath: '/tmp/alpha/src/notes.md',
    productName: 'notes.md',
    citation: 'docs/alpha.md',
    citationPath: '/tmp/alpha/docs/alpha.md',
    diffPath: 'src/alpha.ts',
    diffEditorPath: '/tmp/alpha/src/alpha.ts',
    http: 'https://alpha.example/doc',
    parts: [
      text('看 `docs/alpha.md` 和 [说明甲](https://alpha.example/doc)。'),
      {
        type: 'tool-edit',
        toolCallId: 'open-diff-alpha',
        state: 'output-available',
        input: { path: 'src/alpha.ts', old_string: 'let n = 1', new_string: 'let n = 2' },
        output: 'ok'
      },
      {
        type: 'dynamic-tool',
        toolName: 'Write',
        toolCallId: 'open-write-alpha',
        state: 'output-available',
        input: { file_path: '/tmp/alpha/src/notes.md', content: 'alpha notes' },
        output: 'ok'
      }
    ]
  },
  {
    id: 'beta-report',
    workspacePath: '/tmp/beta',
    productPath: '/tmp/beta/report.ts',
    productName: 'report.ts',
    citation: 'lib/beta.py',
    citationPath: '/tmp/beta/lib/beta.py',
    diffPath: 'notes/beta.txt',
    diffEditorPath: '/tmp/beta/notes/beta.txt',
    http: 'https://beta.example/report',
    parts: [
      text('看 `lib/beta.py` 和 [说明乙](https://beta.example/report)。'),
      {
        type: 'tool-edit',
        toolCallId: 'open-diff-beta',
        state: 'output-available',
        input: { path: 'notes/beta.txt', old_string: 'value = 0', new_string: 'value = 1' },
        output: 'ok'
      },
      {
        type: 'dynamic-tool',
        toolName: 'Write',
        toolCallId: 'open-write-beta',
        state: 'output-available',
        input: { file_path: '/tmp/beta/report.ts', content: 'beta report' },
        output: 'ok'
      }
    ]
  }
]

export const fileTreeCases = [
  {
    id: 'alpha-tree',
    root: '/tmp/alpha',
    folder: 'Box',
    fileName: 'a.txt',
    created: 'a-new.txt',
    renamed: 'a-renamed.txt'
  },
  {
    id: 'beta-tree',
    root: '/tmp/beta',
    folder: 'Drawer',
    fileName: 'b.md',
    created: 'b-new.md',
    renamed: 'b-renamed.md'
  }
]

export const terminalCases = [
  { id: 'alpha-shell', cwd: '/tmp/alpha', sessionId: 'session-alpha' },
  { id: 'beta-shell', cwd: '/tmp/beta', sessionId: 'session-beta' }
]

export const mixedTurn = {
  sessionName: 'Alpha mixed turn',
  workspacePath: '/tmp/alpha',
  prose: '先看目录，记下结果。',
  command: 'pwd',
  stdoutFirst: 'alpha-dir',
  code: 'const alpha = 1',
  language: 'ts',
  diagramHeader: 'flowchart LR',
  diagramSource: 'Start-->End',
  diffFile: 'alpha.ts',
  productName: 'notes.md',
  productPath: '/tmp/alpha/src/notes.md',
  parts: [
    text('先看目录，记下结果。'),
    bash('mix-bash', 'pwd', 'output-available', 'alpha-dir\nalpha-rest'),
    text('```ts\nconst alpha = 1\n```\n\n```mermaid\nflowchart LR\nStart-->End\n```'),
    {
      type: 'tool-edit',
      toolCallId: 'mix-edit',
      state: 'output-available',
      input: { path: 'src/alpha.ts', old_string: 'let n = 1', new_string: 'let n = 2' },
      output: 'ok'
    },
    {
      type: 'dynamic-tool',
      toolName: 'Write',
      toolCallId: 'mix-write',
      state: 'output-available',
      input: { file_path: '/tmp/alpha/src/notes.md', content: 'alpha notes' },
      output: 'ok'
    }
  ] satisfies TaskConversationPart[]
}
