import { createElement as h } from 'react';
import { PeleDiagram } from '../../../../scripts/docs_diagrams.mjs';

export function OfficeArchitectureDiagram() {
  return h(PeleDiagram, {
    name: 'office-architecture',
    label: 'Aster Office 的双执行平面',
    description: '宿主产品同时控制浏览器编辑与原生自动化。浏览器支持五种格式，原生文件命令覆盖三种 OOXML 格式，协作副本覆盖五种格式。',
  });
}
