import { createElement as h } from 'react';
import { PeleDiagram } from '../../../../scripts/docs_diagrams.mjs';

export function OrmArchitectureDiagram() {
  return h(PeleDiagram, {
    name: 'orm-architecture',
    label: 'ORM 的查询编译与执行边界',
    description: '类型化 Schema 组成不可变查询，方言生成 SQL 与绑定参数，异步 Executor 负责连接、事务和结果解码。',
  });
}
