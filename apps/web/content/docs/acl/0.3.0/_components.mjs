import { createElement as h } from 'react';
import { PeleDiagram } from '../../../../scripts/docs_diagrams.mjs';

export function AclArchitectureDiagram() {
  return h(PeleDiagram, {
    name: 'acl-architecture',
    label: 'ACL 从文本到宿主接纳',
    description: '有界解析产生 AST，Schema 检查结构，宿主核对语义与权限。规范字节与摘要用于标识内容，不替代接纳检查。',
  });
}
