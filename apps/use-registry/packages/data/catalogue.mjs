export const DATA_PACKAGE_ID = 'use/aster/data'

export const dataWorkbenchPackage = {
  slug: 'aster-data',
  packageId: DATA_PACKAGE_ID,
  generation: 1,
  revision: `sha256:${'ab'.repeat(32)}`,
  containers: [
    {
      id: 'data',
      title: '数据分析',
      icon: 'database',
      order: 45,
      location: 'activitybar'
    }
  ],
  views: [
    {
      id: 'browser',
      title: '数据',
      description: '连接数据库，列出表，运行 SQL，并保存数据源。',
      icon: 'database',
      order: 10,
      container: 'data',
      point: 'studio.sidebar',
      presentation: 'webview',
      contractVersion: 1,
      mode: 'append'
    },
    {
      id: 'analysis',
      title: '数据分析',
      description: '用已保存的数据源查询，并显示这些行的数值合计。',
      icon: 'database',
      order: 20,
      container: null,
      point: 'studio.applets',
      presentation: 'webview',
      contractVersion: 1,
      mode: 'append'
    }
  ],
  composerModes: [
    {
      id: 'analysis',
      contractVersion: 1,
      title: '数据分析',
      description: '查询已配置的数据源',
      icon: 'database',
      agent: 'analyst',
      editor: 'analysis',
      scope: 'workspace',
      context: ['workspace'],
      order: 30,
      prompt: '查询已配置的数据源',
      tasks: [
        { id: 'query-source', title: '查询数据源', prompt: '查询已配置的数据源' },
        { id: 'aggregate', title: '统计合计', prompt: '统计已配置数据源返回行的数值合计' },
        { id: 'list-tables', title: '列出数据表', prompt: '列出已配置数据源中的表' }
      ]
    }
  ]
}
