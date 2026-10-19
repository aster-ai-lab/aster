export const CLOUD_KNOWLEDGE_CATEGORIES = ['home', 'engineering', 'product', 'research'] as const

export type CloudKnowledgeCategory = (typeof CLOUD_KNOWLEDGE_CATEGORIES)[number]

type CloudShelf = Exclude<CloudKnowledgeCategory, 'home'>

export interface CloudKnowledgeFile {
  name: string
  markdown: string
}

export interface CloudKnowledgeBundle {
  id: string
  titleKey: string
  descriptionKey: string
  rating: number
  categories: readonly CloudShelf[]
  featured: boolean
  files: readonly CloudKnowledgeFile[]
}

const PRODUCT_INDEX = `---
okf_version: "0.2"
---

# 产品手册

- [概述](./概述.md)
`

const PRODUCT_OVERVIEW = `---
type: Guide
title: 概述
description: 产品手册的入口概念。
---

产品从这里进入。[索引](./index.md)
`

const ENGINEERING_INDEX = `---
okf_version: "0.2"
---

# 工程笔记

- [运行时](./运行时.md)
`

const ENGINEERING_CONCEPT = `---
type: Component
title: 运行时
description: 工程笔记里的运行时概念。
---

运行时负责执行。[索引](./index.md)
`

const RESEARCH_INDEX = `---
okf_version: "0.2"
---

# 研究简报

- [问题](./问题.md)
`

const RESEARCH_CONCEPT = `---
type: Brief
title: 问题
description: 研究简报里的问题概念。
---

先写清问题。[索引](./index.md)
`

export const CLOUD_KNOWLEDGE_BUNDLES: readonly CloudKnowledgeBundle[] = [
  {
    id: 'product-handbook',
    titleKey: 'knowledge.cloud.card.product.title',
    descriptionKey: 'knowledge.cloud.card.product.description',
    rating: 4.8,
    categories: ['product'],
    featured: true,
    files: [
      { name: 'index.md', markdown: PRODUCT_INDEX },
      { name: '概述.md', markdown: PRODUCT_OVERVIEW }
    ]
  },
  {
    id: 'engineering-notes',
    titleKey: 'knowledge.cloud.card.engineering.title',
    descriptionKey: 'knowledge.cloud.card.engineering.description',
    rating: 4.6,
    categories: ['engineering'],
    featured: true,
    files: [
      { name: 'index.md', markdown: ENGINEERING_INDEX },
      { name: '运行时.md', markdown: ENGINEERING_CONCEPT }
    ]
  },
  {
    id: 'research-briefs',
    titleKey: 'knowledge.cloud.card.research.title',
    descriptionKey: 'knowledge.cloud.card.research.description',
    rating: 4.7,
    categories: ['research'],
    featured: true,
    files: [
      { name: 'index.md', markdown: RESEARCH_INDEX },
      { name: '问题.md', markdown: RESEARCH_CONCEPT }
    ]
  }
]

export function cloudKnowledgeBundle(id: string): CloudKnowledgeBundle | undefined {
  return CLOUD_KNOWLEDGE_BUNDLES.find((bundle) => bundle.id === id)
}

export function visibleCloudBundles(category: CloudKnowledgeCategory): readonly CloudKnowledgeBundle[] {
  if (category === 'home') return CLOUD_KNOWLEDGE_BUNDLES.filter((bundle) => bundle.featured)
  return CLOUD_KNOWLEDGE_BUNDLES.filter((bundle) => bundle.categories.includes(category))
}
