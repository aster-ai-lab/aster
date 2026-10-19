/**
 * Local plan drafts. Markdown is the document. The cover reads the same
 * title, author, and organization fields. Each template follows the
 * same section outline as the public document kind it names.
 */

export const PLAN_DRAFTS_KEY = 'a3s.studio.plan-drafts.v1'

const DRAFT_LIMIT = 30
const MARKDOWN_LIMIT = 200_000
const LOGO_LIMIT = 180 * 1024
const FIELD_LIMIT = 160

export const PLAN_CATEGORIES = [
  'all',
  'blank',
  'guide',
  'development',
  'design',
  'delivery',
  'business',
  'meeting',
  'academic'
] as const

export type PlanCategory = (typeof PLAN_CATEGORIES)[number]
export type PlanTemplateCategory = Exclude<PlanCategory, 'all'>

export const PLAN_COVER_IDS = [
  'classic',
  'tech',
  'formal',
  'mono',
  'gradient',
  'academic',
  'approval',
  'brief'
] as const

export type PlanCoverId = (typeof PLAN_COVER_IDS)[number]
export type PlanLayout = 'single' | 'double'
export type PlanBodySize = 'sm' | 'md' | 'lg'
export type PlanMarkup =
  | 'h1'
  | 'h2'
  | 'h3'
  | 'h4'
  | 'bold'
  | 'italic'
  | 'list'
  | 'ordered'
  | 'quote'
  | 'code'
  | 'rule'
  | 'table'
  | 'link'
  | 'break'
  | 'sup'
  | 'sub'
export type PlanView = 'split' | 'edit' | 'preview'
export type PlanPanel = 'cover' | 'header' | 'toc' | 'color' | 'style' | 'other'

export type PlanMeta = {
  title: string
  subtitle: string
  number: string
  version: string
  date: string
  author: string
  organization: string
  department: string
  header: string
  watermark: string
  logo: string
}

export type PlanAlign = 'left' | 'center' | 'right'

/** Preview and print settings. Markdown stays the document. */
export type PlanStyle = {
  pagination: 'manual' | 'auto'
  bullet: 'dot' | 'square' | 'check' | 'arrow'
  numbers: 'decimal' | 'paren' | 'cjk'
  code: 'light' | 'dark'
  table: 'minimal' | 'striped' | 'bordered'
  imageFrame: 'none' | 'shadow' | 'line' | 'deep' | 'card' | 'round'
  imagePrefix: string
  imageAlign: PlanAlign
  imageCaption: boolean
  imageNumber: boolean
  tablePlace: 'above' | 'below'
  tablePrefix: string
  tableAlign: PlanAlign
  tableCaption: boolean
  tableNumber: boolean
  watermarkOn: boolean
  watermarkKind: 'text' | 'image'
  watermarkImage: string
  headerOn: boolean
  headerLeft: string
  headerRight: string
  headerRule: 'solid' | 'accent' | 'double' | 'none'
  footerOn: boolean
  footerLeft: string
  footerCenter: string
  footerRight: string
  pageFormat: 'page' | 'total' | 'hyphen' | 'simple' | 'none'
  pagePlace: PlanAlign
  tocOn: boolean
  tocTitle: string
  tocDepth: 1 | 2 | 3 | 4
  tocLeader: 'dots' | 'dashes' | 'line' | 'none'
  tocPages: boolean
  tocCenter: boolean
  tocForm: 'underline' | 'accent' | 'badge' | 'minimal'
  font: 'sans' | 'serif' | 'kai' | 'hei' | 'mono'
  numbering: 'none' | 'decimal' | 'skip' | 'cjk' | 'mixed'
  h1Form: 'underline' | 'accent' | 'badge' | 'minimal'
  h2Form: 'bar' | 'line' | 'plain'
  h1Break: boolean
  h1Center: boolean
  leading: 'tight' | 'normal' | 'loose'
  indent: boolean
  ink: 'cover' | 'ink' | 'pine' | 'wine' | 'violet' | 'amber'
  mermaid: 'neutral' | 'default' | 'dark' | 'forest' | 'base'
}

export type PlanDraft = {
  id: string
  templateId: string
  category: PlanTemplateCategory
  markdown: string
  /** Local pictures referenced from the body as `plan-image:<id>`. */
  images: Record<string, string>
  meta: PlanMeta
  coverId: PlanCoverId
  layout: PlanLayout
  bodySize: PlanBodySize
  /** When false, the preview starts on the first body sheet. */
  showCover: boolean
  style: PlanStyle
  updatedAt: number
}

export type PlanTemplate = {
  id: string
  category: PlanTemplateCategory
  titleKey: string
  englishKey: string
  badgeKey: string
  subtitleKey: string
  number: string
  version: string
  coverId: PlanCoverId
  layout: PlanLayout
  markdown: string
}

export type PlanCover = {
  id: PlanCoverId
  labelKey: string
  paper: string
  ink: string
  accent: string
  mute: string
}

export const PLAN_COVERS: readonly PlanCover[] = [
  {
    id: 'classic',
    labelKey: 'workbench.plan.cover.classic',
    paper: '#f7f4ef',
    ink: '#1f3d32',
    accent: '#1f3d32',
    mute: '#6d645c'
  },
  {
    id: 'tech',
    labelKey: 'workbench.plan.cover.tech',
    paper: '#f4f6f8',
    ink: '#171717',
    accent: '#171717',
    mute: '#5c6570'
  },
  {
    id: 'formal',
    labelKey: 'workbench.plan.cover.formal',
    paper: '#ffffff',
    ink: '#1a1a1a',
    accent: '#8f1d1d',
    mute: '#5c5c5c'
  },
  {
    id: 'mono',
    labelKey: 'workbench.plan.cover.mono',
    paper: '#ffffff',
    ink: '#111111',
    accent: '#111111',
    mute: '#444444'
  },
  {
    id: 'gradient',
    labelKey: 'workbench.plan.cover.gradient',
    paper: '#f3f1fb',
    ink: '#241c4b',
    accent: '#4c3d8f',
    mute: '#6b6288'
  },
  {
    id: 'academic',
    labelKey: 'workbench.plan.cover.academic',
    paper: '#ffffff',
    ink: '#1a1a1a',
    accent: '#1a1a1a',
    mute: '#555555'
  },
  {
    id: 'approval',
    labelKey: 'workbench.plan.cover.approval',
    paper: '#fbfbfb',
    ink: '#171717',
    accent: '#171717',
    mute: '#666666'
  },
  {
    id: 'brief',
    labelKey: 'workbench.plan.cover.brief',
    paper: '#f6f7f4',
    ink: '#1d3b2a',
    accent: '#2f6b4f',
    mute: '#5e6b62'
  }
]

const COVER_IDS = new Set<string>(PLAN_COVER_IDS)
const TEMPLATE_CATEGORIES = new Set<string>(PLAN_CATEGORIES.filter((item) => item !== 'all'))
const LAYOUTS = new Set<string>(['single', 'double'])
const SIZES = new Set<string>(['sm', 'md', 'lg'])
const PREFIX_LIMIT = 8
const CJK_DIGITS = '一二三四五六七八九十'
const CJK_ENUM = '、'
const CJK_OPEN = '（'
const CJK_CLOSE = '）'
const PAGE_BUDGET = 860
const SLOT_LIMIT = 80
/** Headings kept on one contents sheet. HTML export uses the same size. */
export const PLAN_TOC_CHUNK = 26

export const PLAN_STYLE_DEFAULT: PlanStyle = {
  pagination: 'manual',
  bullet: 'dot',
  numbers: 'decimal',
  code: 'light',
  table: 'minimal',
  imageFrame: 'shadow',
  imagePrefix: '图',
  imageAlign: 'center',
  imageCaption: true,
  imageNumber: true,
  tablePlace: 'above',
  tablePrefix: '表',
  tableAlign: 'center',
  tableCaption: true,
  tableNumber: true,
  watermarkOn: true,
  watermarkKind: 'text',
  watermarkImage: '',
  headerOn: true,
  headerLeft: '',
  headerRight: '',
  headerRule: 'solid',
  footerOn: true,
  footerLeft: '',
  footerCenter: '',
  footerRight: '',
  pageFormat: 'page',
  pagePlace: 'right',
  tocOn: false,
  tocTitle: '',
  tocDepth: 3,
  tocLeader: 'dots',
  tocPages: true,
  tocCenter: false,
  tocForm: 'underline',
  font: 'sans',
  numbering: 'none',
  h1Form: 'underline',
  h2Form: 'plain',
  h1Break: false,
  h1Center: false,
  leading: 'normal',
  indent: false,
  ink: 'cover',
  mermaid: 'neutral'
}

const PAGINATION = ['manual', 'auto'] as const
const BULLETS = ['dot', 'square', 'check', 'arrow'] as const
const NUMBERS = ['decimal', 'paren', 'cjk'] as const
const CODE_THEMES = ['light', 'dark'] as const
const TABLE_STYLES = ['minimal', 'striped', 'bordered'] as const
const IMAGE_FRAMES = ['none', 'shadow', 'line', 'deep', 'card', 'round'] as const
const ALIGNS = ['left', 'center', 'right'] as const
const TABLE_PLACES = ['above', 'below'] as const
const WATERMARK_KINDS = ['text', 'image'] as const
const HEADER_RULES = ['solid', 'accent', 'double', 'none'] as const
const PAGE_FORMATS = ['page', 'total', 'hyphen', 'simple', 'none'] as const
const TOC_LEADERS = ['dots', 'dashes', 'line', 'none'] as const
const TOC_FORMS = ['underline', 'accent', 'badge', 'minimal'] as const
const FONTS = ['sans', 'serif', 'kai', 'hei', 'mono'] as const
const NUMBERINGS = ['none', 'decimal', 'skip', 'cjk', 'mixed'] as const
const H1_FORMS = ['underline', 'accent', 'badge', 'minimal'] as const
const H2_FORMS = ['bar', 'line', 'plain'] as const
const LEADINGS = ['tight', 'normal', 'loose'] as const
const INKS = ['cover', 'ink', 'pine', 'wine', 'violet', 'amber'] as const
const MERMAID_THEMES = ['neutral', 'default', 'dark', 'forest', 'base'] as const
const INK_COLOR: Record<Exclude<PlanStyle['ink'], 'cover'>, string> = {
  ink: '#171717',
  pine: '#1f3d32',
  wine: '#8f1d1d',
  violet: '#4c3d8f',
  amber: '#8a5a12'
}

function lines(parts: readonly string[]): string {
  return parts.join('\n')
}

export const PLAN_TEMPLATES: readonly PlanTemplate[] = [
  {
    id: 'blank',
    category: 'blank',
    titleKey: 'workbench.plan.template.blank.title',
    englishKey: 'workbench.plan.template.blank.english',
    badgeKey: 'workbench.plan.template.blank.badge',
    subtitleKey: 'workbench.plan.template.blank.subtitle',
    number: '',
    version: 'v1.0.0',
    coverId: 'mono',
    layout: 'single',
    markdown: lines(['# 开始', '', '1. 填写标题、作者和机构。', '2. 选择封面。', '3. 在左侧写下正文。'])
  },
  {
    id: 'guide',
    category: 'guide',
    titleKey: 'workbench.plan.template.guide.title',
    englishKey: 'workbench.plan.template.guide.english',
    badgeKey: 'workbench.plan.template.guide.badge',
    subtitleKey: 'workbench.plan.template.guide.subtitle',
    number: 'PLAN-GUIDE',
    version: 'v1.0',
    coverId: 'tech',
    layout: 'single',
    markdown: lines([
      '# 概述',
      '',
      '这份范本演示当前编辑器能排进正文的内容：标题、列表、表格、题注、代码和分页。',
      '',
      '# 高效编辑与预览',
      '',
      '左侧写正文。中间看封面和分页。右侧改标题、机构和封面。',
      '',
      '## 定位与分页检查',
      '',
      '右侧目录按标题跳转。正文里的 `<!-- pagebreak -->` 会另起一页。',
      '',
      '## 封面与页眉页脚动态字段',
      '',
      '封面读取标题、副标题、编号、版本、日期、作者、机构和部门。页眉写在右侧「页眉」栏。',
      '',
      '## 文档水印',
      '',
      '水印写在右侧「其他」里。留空则不显示。',
      '',
      '# 高阶使用说明',
      '',
      '## 文本与列表',
      '',
      '正文可以使用 **粗体**、*斜体* 和行内代码 `Ctrl+B`。',
      '',
      '- 先写结论。',
      '- 再写依据。',
      '- 最后写下一步。',
      '',
      '## 表格与题注',
      '',
      '<!-- caption: 交付前核对 -->',
      '| 项目 | 要求 |',
      '| --- | --- |',
      '| 封面 | 标题、编号、日期已填写 |',
      '| 正文 | 每一节都有一句结论 |',
      '',
      '## 图表、代码与分页',
      '',
      '图和公式先写在代码块里。预览按代码原样显示。',
      '',
      '```',
      '入口 --> 正文 --> 封面',
      '```',
      '',
      '<!-- pagebreak -->',
      '',
      '# 保存与打印',
      '',
      '## 保存、草稿与文档历史',
      '',
      '草稿保存在这台电脑上。换一台电脑时，这里看不到同一份草稿。'
    ])
  },
  {
    id: 'delivery',
    category: 'delivery',
    titleKey: 'workbench.plan.template.delivery.title',
    englishKey: 'workbench.plan.template.delivery.english',
    badgeKey: 'workbench.plan.template.delivery.badge',
    subtitleKey: 'workbench.plan.template.delivery.subtitle',
    number: 'PLAN-DELIVERY',
    version: 'v1.0',
    coverId: 'classic',
    layout: 'double',
    markdown: lines([
      '# 企业标准技术交付文档',
      '',
      '## 1. 建设背景',
      '',
      '写下现在哪一条链路完不成，以及谁在使用它。',
      '',
      '## 2. 建设目标',
      '',
      '- 关键路径可以增加实例。',
      '- 一个服务失败时，其余服务继续接受请求。',
      '- 发布、回滚、备份和告警各有一份记录。',
      '',
      '## 3. 总体架构',
      '',
      '```',
      '使用方 --> 入口 --> 业务服务 --> 数据',
      '业务服务 --> 观测',
      '```',
      '',
      '## 4. 关键设计',
      '',
      '### 4.1 高可用设计',
      '',
      '写下实例数量、健康检查，以及失败时先停止哪一段。',
      '',
      '### 4.2 数据一致性',
      '',
      '写下哪一段必须同时成功，哪一段可以稍后补齐。',
      '',
      '## 5. 部署与运维',
      '',
      '<!-- caption: 部署、观测与备份 -->',
      '| 项目 | 交付要求 | 验收方式 |',
      '| --- | --- | --- |',
      '| 部署 | 可以回滚 | 演练记录 |',
      '| 监控 | 指标、日志、调用链 | 告警演练 |',
      '| 备份 | 有恢复步骤 | 恢复演练 |',
      '',
      '## 6. 交付清单',
      '',
      '1. 架构说明。',
      '2. 部署清单。',
      '3. 验收记录。',
      '4. 运维步骤。'
    ])
  },
  {
    id: 'whitepaper',
    category: 'design',
    titleKey: 'workbench.plan.template.whitepaper.title',
    englishKey: 'workbench.plan.template.whitepaper.english',
    badgeKey: 'workbench.plan.template.whitepaper.badge',
    subtitleKey: 'workbench.plan.template.whitepaper.subtitle',
    number: 'PLAN-DESIGN',
    version: 'v1.0',
    coverId: 'mono',
    layout: 'single',
    markdown: lines([
      '# 极简产品设计白皮书',
      '',
      '## 设计原则',
      '',
      '一屏只完成一件事。标题先写结论。',
      '',
      '## 信息层级',
      '',
      '- 一页只有一个主任务。',
      '- 标题、正文和辅助信息的字号不同。',
      '- 用留白分组，少用装饰线。',
      '',
      '## 布局规范',
      '',
      '<!-- caption: 页面间距 -->',
      '| 场景 | 建议 |',
      '| --- | --- |',
      '| 页边距 | 各页相同 |',
      '| 正文宽度 | 一行可以读完 |',
      '| 组件间距 | 使用同一套间距 |',
      '',
      '## 交互反馈',
      '',
      '加载、成功和失败都要看得见。反馈不挡住正在填写的内容。',
      '',
      '## 可访问性',
      '',
      '文字和背景要能分开。键盘焦点要看得见。只有图标的按钮要有名称。'
    ])
  },
  {
    id: 'architecture',
    category: 'development',
    titleKey: 'workbench.plan.template.architecture.title',
    englishKey: 'workbench.plan.template.architecture.english',
    badgeKey: 'workbench.plan.template.architecture.badge',
    subtitleKey: 'workbench.plan.template.architecture.subtitle',
    number: 'PLAN-ARCH',
    version: 'v1.0',
    coverId: 'tech',
    layout: 'single',
    markdown: lines([
      '# 项目概述与设计目标',
      '',
      '## 背景说明',
      '',
      '写下系统现在服务谁，以及这次要改变的一个限制。',
      '',
      '## 关键性能指标 (KPI)',
      '',
      '<!-- caption: 关键性能指标交付与对比 -->',
      '| 指标 | 当前 | 目标 |',
      '| --- | --- | --- |',
      '| 可用性 |  |  |',
      '| 响应时间 |  |  |',
      '',
      '<!-- pagebreak -->',
      '',
      '# 系统整体架构设计',
      '',
      '## 逻辑架构图视图',
      '',
      '```',
      '调用方 --> 网关 --> 业务服务 --> 数据',
      '业务服务 --> 消息',
      '```',
      '',
      '## 核心模块职责分配',
      '',
      '<!-- caption: 模块职责 -->',
      '| 模块 | 只做的一件事 | 不做的事 |',
      '| --- | --- | --- |',
      '| 入口 | 校验并转发 | 保存业务数据 |',
      '| 业务服务 | 完成一项业务 | 签发身份 |',
      '',
      '# 核心 API 接口与数据模型',
      '',
      '## 接口规范',
      '',
      '写下路径、谁能调用、成功时返回什么、失败时返回什么。',
      '',
      '## 数据表',
      '',
      '<!-- caption: 核心字段 -->',
      '| 字段 | 含义 | 约束 |',
      '| --- | --- | --- |',
      '| 标识 | 一条记录 | 唯一 |',
      '',
      '<!-- pagebreak -->',
      '',
      '# 实施计划与交付验收标准',
      '',
      '## 阶段里程碑规划',
      '',
      '1. 确认边界。',
      '2. 打通一条调用。',
      '3. 按指标验收。',
      '',
      '## 交付产物清单',
      '',
      '- 架构说明。',
      '- 接口说明。',
      '- 验收记录。'
    ])
  },
  {
    id: 'approval',
    category: 'delivery',
    titleKey: 'workbench.plan.template.approval.title',
    englishKey: 'workbench.plan.template.approval.english',
    badgeKey: 'workbench.plan.template.approval.badge',
    subtitleKey: 'workbench.plan.template.approval.subtitle',
    number: 'PLAN-SIGN',
    version: 'v1.0',
    coverId: 'approval',
    layout: 'double',
    markdown: lines([
      '# 企业信息化项目签审交付方案',
      '',
      '## 文档签审',
      '',
      '<!-- caption: 签审栏 -->',
      '| 角色 | 姓名 / 部门 | 审批意见 | 日期 |',
      '| --- | --- | --- | --- |',
      '| 编制 |  |  |  |',
      '| 审核 |  |  |  |',
      '| 批准 |  |  |  |',
      '',
      '## 1. 项目概述',
      '',
      '用一段话写下要建设的能力和使用它的人。',
      '',
      '## 2. 建设范围',
      '',
      '- 本期包含的能力。',
      '- 本期不包含的能力。',
      '',
      '## 3. 总体方案',
      '',
      '```',
      '使用方 --> 门户 --> 流程',
      '门户 --> 数据',
      '```',
      '',
      '## 4. 交付清单',
      '',
      '<!-- caption: 交付物 -->',
      '| 序号 | 交付物 | 验收依据 | 责任方 |',
      '| --- | --- | --- | --- |',
      '| 1 |  |  |  |',
      '',
      '## 5. 验收标准',
      '',
      '1. 主路径有验收记录。',
      '2. 权限、日志和备份有检查记录。',
      '3. 签审和版本可以回溯。',
      '',
      '## 6. 版本记录',
      '',
      '<!-- caption: 版本 -->',
      '| 版本 | 日期 | 变更内容 | 编制人 |',
      '| --- | --- | --- | --- |',
      '| v1.0 |  | 首次提交 |  |'
    ])
  },
  {
    id: 'research',
    category: 'academic',
    titleKey: 'workbench.plan.template.research.title',
    englishKey: 'workbench.plan.template.research.english',
    badgeKey: 'workbench.plan.template.research.badge',
    subtitleKey: 'workbench.plan.template.research.subtitle',
    number: 'PLAN-NOTE',
    version: 'v1.0',
    coverId: 'academic',
    layout: 'single',
    markdown: lines([
      '# 调研与实验报告',
      '',
      '## 一、研究背景与目标',
      '',
      '用一句话写下要回答的问题，以及怎样算回答完。',
      '',
      '## 二、研究方法',
      '',
      '1. 写下样本从哪里来，以及哪些不纳入。',
      '2. 写下实验条件和对照条件。',
      '3. 用同一口径记录结果。',
      '',
      '## 三、实验结果',
      '',
      '<!-- caption: 实验组与对照组指标对比 -->',
      '| 指标 | 对照组 | 实验组 | 说明 |',
      '| --- | --- | --- | --- |',
      '|  |  |  |  |',
      '',
      '## 四、分析与讨论',
      '',
      '只解释表里已经写下的差异。写明还不能下结论的地方。',
      '',
      '## 五、结论与建议',
      '',
      '先写结论，再写一条后续实验。'
    ])
  },
  {
    id: 'tokens',
    category: 'design',
    titleKey: 'workbench.plan.template.tokens.title',
    englishKey: 'workbench.plan.template.tokens.english',
    badgeKey: 'workbench.plan.template.tokens.badge',
    subtitleKey: 'workbench.plan.template.tokens.subtitle',
    number: 'PLAN-TOKEN',
    version: 'v1.0',
    coverId: 'gradient',
    layout: 'single',
    markdown: lines([
      '# 体验设计原则与 Token 系统',
      '',
      '## 设计视觉语言',
      '',
      '- 间距按 8 像素递增。',
      '- 用对比区分层级，少用装饰。',
      '- 状态变化要短，并且看得到。',
      '',
      '## 品牌色彩 Token 定义',
      '',
      '<!-- caption: 色彩 Token -->',
      '| 名称 | 色值 | 用途 |',
      '| --- | --- | --- |',
      '| 文字 | #171717 | 标题和正文 |',
      '| 输入底 | #f5f5f5 | 输入框 |',
      '| 选中 | #f2f3f5 | 选中行和选中页签 |',
      '',
      '# 核心组件布局与规则',
      '',
      '> 可点击区域至少 24 像素。只有图标的按钮要有名称。',
      '',
      '## 按钮变体规范',
      '',
      '1. 主按钮：一页只有一个确认。',
      '2. 次按钮：取消和返回。',
      '3. 幽灵按钮：行内操作和图标。',
      '',
      '## 状态提示与弹窗',
      '',
      '<!-- caption: 组件尺寸 -->',
      '| 属性 | 默认值 | 说明 |',
      '| --- | --- | --- |',
      '| 圆角 | 8px | 弹窗 12px |',
      '| 输入高度 | 36px | 搜索 30px |'
    ])
  },
  {
    id: 'proposal',
    category: 'business',
    titleKey: 'workbench.plan.template.proposal.title',
    englishKey: 'workbench.plan.template.proposal.english',
    badgeKey: 'workbench.plan.template.proposal.badge',
    subtitleKey: 'workbench.plan.template.proposal.subtitle',
    number: 'PLAN-BIZ',
    version: 'v1.0',
    coverId: 'brief',
    layout: 'double',
    markdown: lines([
      '# 商业立项与可行性研究报告',
      '',
      '## 1. 项目执行摘要',
      '',
      '谁在什么情况下完不成哪一步。完成后，这个人能做什么。',
      '',
      '## 2. 目标市场与客户痛点分析',
      '',
      '<!-- caption: 痛点与对应做法 -->',
      '| 对象 | 痛点 | 做法 |',
      '| --- | --- | --- |',
      '|  |  |  |',
      '',
      '<!-- pagebreak -->',
      '',
      '# 3. 商业模式与收益测算',
      '',
      '写下谁付费，以及按什么计量。',
      '',
      '## 3.1 财务收益预测',
      '',
      '<!-- caption: 三年收支 -->',
      '| 年度 | 收入 | 支出 | 结果 |',
      '| --- | --- | --- | --- |',
      '| 第一年 |  |  |  |',
      '| 第二年 |  |  |  |',
      '| 第三年 |  |  |  |',
      '',
      '## 4. 关键里程碑与退出机制',
      '',
      '1. 确认范围。',
      '2. 完成第一版。',
      '3. 对照结果，决定继续或停止。'
    ])
  },
  {
    id: 'thesis',
    category: 'academic',
    titleKey: 'workbench.plan.template.thesis.title',
    englishKey: 'workbench.plan.template.thesis.english',
    badgeKey: 'workbench.plan.template.thesis.badge',
    subtitleKey: 'workbench.plan.template.thesis.subtitle',
    number: 'PLAN-THESIS',
    version: 'v1.0',
    coverId: 'academic',
    layout: 'single',
    markdown: lines([
      '# 学术论文与学位开题报告',
      '',
      '## 摘要',
      '',
      '用一段话写下问题、方法和预期结果。',
      '',
      '关键词：在这里写下三到五个词。',
      '',
      '<!-- pagebreak -->',
      '',
      '# 1. 绪论与选题背景',
      '',
      '## 1.1 研究背景与意义',
      '',
      '写下这个问题现在为什么需要回答。',
      '',
      '## 1.2 国内外研究现状',
      '',
      '只保留和这个问题直接相关的工作，并写下出处。',
      '',
      '<!-- pagebreak -->',
      '',
      '# 2. 系统核心架构与方法',
      '',
      '## 2.1 方法',
      '',
      '写下输入、处理步骤和输出。每一步都可以由别人重复。',
      '',
      '## 3. 实验结果与对比分析',
      '',
      '<!-- caption: 基线与本方案对照 -->',
      '| 方案 | 指标 | 结果 |',
      '| --- | --- | --- |',
      '| 基线 |  |  |',
      '| 本方案 |  |  |'
    ])
  },
  {
    id: 'minutes',
    category: 'meeting',
    titleKey: 'workbench.plan.template.minutes.title',
    englishKey: 'workbench.plan.template.minutes.english',
    badgeKey: 'workbench.plan.template.minutes.badge',
    subtitleKey: 'workbench.plan.template.minutes.subtitle',
    number: 'PLAN-MEET',
    version: 'v1.0',
    coverId: 'formal',
    layout: 'single',
    markdown: lines([
      '# 重点项目会议纪要与行动看板',
      '',
      '## 一、会议基本信息',
      '',
      '- **会议主题**：',
      '- **会议时间**：',
      '- **会议地点**：',
      '- **主持人**：',
      '- **记录人**：',
      '- **参会人员**：',
      '',
      '---',
      '',
      '## 二、核心讨论议题与决议',
      '',
      '### 议题一',
      '',
      '- **现状**：',
      '- **决议**：',
      '',
      '### 议题二',
      '',
      '- **决议**：',
      '',
      '<!-- pagebreak -->',
      '',
      '## 三、关键决策行动清单',
      '',
      '<!-- caption: 待办分派 -->',
      '| 序号 | 事项 | 负责人 | 配合 | 日期 | 状态 |',
      '| --- | --- | --- | --- | --- | --- |',
      '| 1 |  |  |  |  | 未开始 |'
    ])
  },
  {
    id: 'prd',
    category: 'development',
    titleKey: 'workbench.plan.template.prd.title',
    englishKey: 'workbench.plan.template.prd.english',
    badgeKey: 'workbench.plan.template.prd.badge',
    subtitleKey: 'workbench.plan.template.prd.subtitle',
    number: 'PLAN-PRD',
    version: 'v1.0',
    coverId: 'tech',
    layout: 'single',
    markdown: lines([
      '# 产品需求规格说明书 (PRD)',
      '',
      '## 一、文档基本信息与版本修订历史',
      '',
      '### 1.1 文档控制信息',
      '',
      '- 产品名称：',
      '- 读者：',
      '- 状态：草稿',
      '',
      '### 1.2 版本修订记录',
      '',
      '<!-- caption: 版本修订 -->',
      '| 版本 | 日期 | 作者 | 说明 |',
      '| --- | --- | --- | --- |',
      '| v1.0 |  |  | 初稿 |',
      '',
      '<!-- pagebreak -->',
      '',
      '## 二、产品背景与业务目标',
      '',
      '### 2.1 业务背景',
      '',
      '谁在什么情况下完不成哪一步。',
      '',
      '### 2.2 核心业务目标',
      '',
      '1. 本期要完成的一个结果。',
      '2. 怎样核对这个结果。',
      '',
      '## 三、目标用户画像与核心场景',
      '',
      '<!-- caption: 用户与场景 -->',
      '| 用户 | 场景 | 要完成的事 |',
      '| --- | --- | --- |',
      '|  |  |  |',
      '',
      '<!-- pagebreak -->',
      '',
      '## 四、功能架构与特性清单',
      '',
      '### 4.1 整体功能架构',
      '',
      '```',
      '用户 --> 入口 --> 主任务 --> 结果',
      '```',
      '',
      '### 4.2 特性需求优先级矩阵',
      '',
      '<!-- caption: 优先级 -->',
      '| 特性 | 优先级 | 验收 |',
      '| --- | --- | --- |',
      '|  | 高 |  |',
      '',
      '<!-- pagebreak -->',
      '',
      '## 五、核心业务流程与交互状态',
      '',
      '### 5.1 状态',
      '',
      '1. 草稿。',
      '2. 进行中。',
      '3. 完成。',
      '',
      '### 5.2 异常',
      '',
      '写下失败时用户看到什么，以及可以重试还是必须停止。',
      '',
      '## 六、数据埋点与指标监控需求',
      '',
      '<!-- caption: 需要记录的行为 -->',
      '| 行为 | 何时记录 | 用来判断什么 |',
      '| --- | --- | --- |',
      '|  |  |  |'
    ])
  },
  {
    id: 'incident',
    category: 'development',
    titleKey: 'workbench.plan.template.incident.title',
    englishKey: 'workbench.plan.template.incident.english',
    badgeKey: 'workbench.plan.template.incident.badge',
    subtitleKey: 'workbench.plan.template.incident.subtitle',
    number: 'PLAN-INCIDENT',
    version: 'v1.0',
    coverId: 'formal',
    layout: 'single',
    markdown: lines([
      '# 生产重大故障复盘与根因分析 (RCA)',
      '',
      '## 一、事故核心概要与影响定级',
      '',
      '### 1.1 事故基本事实',
      '',
      '- 发现时间：',
      '- 恢复时间：',
      '- 影响对象：',
      '- 等级：',
      '',
      '### 1.2 故障关键度量指标',
      '',
      '<!-- caption: 响应与恢复 -->',
      '| 指标 | 时间 |',
      '| --- | --- |',
      '| 发现到响应 |  |',
      '| 响应到恢复 |  |',
      '',
      '### 1.3 业务影响面评估',
      '',
      '哪些人不能完成哪一步。哪些能力仍然可用。',
      '',
      '<!-- pagebreak -->',
      '',
      '## 二、完整时间线',
      '',
      '<!-- caption: 时间线 -->',
      '| 时间 | 事件 | 操作人 |',
      '| --- | --- | --- |',
      '|  | 发现 |  |',
      '|  | 恢复 |  |',
      '',
      '<!-- pagebreak -->',
      '',
      '## 三、故障根本原因分析',
      '',
      '### 3.1 传播过程',
      '',
      '从第一个失败点写到用户看见的现象。每一步只写一个原因。',
      '',
      '### 3.2 逐层追问',
      '',
      '1. 为什么出现这个现象。',
      '2. 为什么那一层没有挡住。',
      '3. 为什么发现得晚。',
      '',
      '## 四、纠正与预防措施',
      '',
      '<!-- caption: 改进项 -->',
      '| 措施 | 负责人 | 日期 | 状态 |',
      '| --- | --- | --- | --- |',
      '|  |  |  | 未开始 |'
    ])
  },
  {
    id: 'sop',
    category: 'delivery',
    titleKey: 'workbench.plan.template.sop.title',
    englishKey: 'workbench.plan.template.sop.english',
    badgeKey: 'workbench.plan.template.sop.badge',
    subtitleKey: 'workbench.plan.template.sop.subtitle',
    number: 'PLAN-SOP',
    version: 'v1.0',
    coverId: 'classic',
    layout: 'single',
    markdown: lines([
      '# 生产运维应急排障标准化手册 (SOP)',
      '',
      '## 一、规程概述与适用范围',
      '',
      '### 1.1 目的与适用边界',
      '',
      '写下触发条件。不符合条件时，不要执行后面的步骤。',
      '',
      '## 二、应急响应等级与通报机制',
      '',
      '<!-- caption: 等级与通报 -->',
      '| 等级 | 现象 | 通报对象 |',
      '| --- | --- | --- |',
      '| 高 | 主路径不可用 | 负责人 |',
      '| 中 | 部分人受影响 | 值班 |',
      '',
      '<!-- pagebreak -->',
      '',
      '## 三、应急决策分流',
      '',
      '```',
      '现象 --> 确认范围 --> 恢复或升级',
      '```',
      '',
      '<!-- pagebreak -->',
      '',
      '## 四、核心标准处置步骤',
      '',
      '### 4.1 场景 A',
      '',
      '#### 步骤 1：确认现象',
      '',
      '写下查看哪里，以及看到什么才继续。',
      '',
      '#### 步骤 2：处置',
      '',
      '写下操作、期望结果和可以回退的方法。',
      '',
      '### 4.2 场景 B',
      '',
      '写下第二种现象的判断和处置。不要把两种场景混在同一步。',
      '',
      '## 五、验证检查清单与服务恢复确认',
      '',
      '<!-- caption: 恢复确认 -->',
      '| 检查项 | 期望 | 结果 |',
      '| --- | --- | --- |',
      '| 主路径 | 可以完成 |  |',
      '| 告警 | 已恢复或已说明 |  |'
    ])
  },
  {
    id: 'weekly',
    category: 'meeting',
    titleKey: 'workbench.plan.template.weekly.title',
    englishKey: 'workbench.plan.template.weekly.english',
    badgeKey: 'workbench.plan.template.weekly.badge',
    subtitleKey: 'workbench.plan.template.weekly.subtitle',
    number: 'PLAN-WEEK',
    version: 'v1.0',
    coverId: 'brief',
    layout: 'single',
    markdown: lines([
      '# 项目核心概况与健康度看板',
      '',
      '<!-- caption: 健康度 -->',
      '| 维度 | 状态 | 说明 |',
      '| --- | --- | --- |',
      '| 进度 |  |  |',
      '| 质量 |  |  |',
      '| 风险 |  |  |',
      '',
      '# 本周核心交付成果与进展亮点',
      '',
      '## 关键业务与技术进展',
      '',
      '- 完成的第一项。',
      '',
      '## 核心质量与交付指标对比',
      '',
      '<!-- caption: 本周与上周 -->',
      '| 指标 | 上周 | 本周 |',
      '| --- | --- | --- |',
      '|  |  |  |',
      '',
      '# 关键阻塞风险与应对策略',
      '',
      '<!-- caption: 风险 -->',
      '| 风险 | 影响 | 负责人 | 下一步 |',
      '| --- | --- | --- | --- |',
      '|  |  |  |  |',
      '',
      '# 下周重点攻坚任务清单',
      '',
      '<!-- caption: 下周任务 -->',
      '| 任务 | 负责人 | 日期 |',
      '| --- | --- | --- |',
      '|  |  |  |'
    ])
  },
  {
    id: 'metrics',
    category: 'business',
    titleKey: 'workbench.plan.template.metrics.title',
    englishKey: 'workbench.plan.template.metrics.english',
    badgeKey: 'workbench.plan.template.metrics.badge',
    subtitleKey: 'workbench.plan.template.metrics.subtitle',
    number: 'PLAN-METRIC',
    version: 'v1.0',
    coverId: 'formal',
    layout: 'double',
    markdown: lines([
      '# 业务核心指标分析与经营洞察报告',
      '',
      '## 一、报告执行摘要',
      '',
      '用三句话写下：本期最好的一项、最差的一项、下一期只改的一件事。',
      '',
      '<!-- pagebreak -->',
      '',
      '## 二、核心大盘指标概览与对比',
      '',
      '<!-- caption: 本期与上期 -->',
      '| 指标 | 上期 | 本期 | 变化 |',
      '| --- | --- | --- | --- |',
      '|  |  |  |  |',
      '',
      '## 三、用户全生命周期转化漏斗分析',
      '',
      '<!-- caption: 漏斗 -->',
      '| 阶段 | 人数 | 进入下一阶段 |',
      '| --- | --- | --- |',
      '|  |  |  |',
      '',
      '### 3.1 损耗',
      '',
      '只解释掉得最多的那一层。写下已经核对过的原因。',
      '',
      '<!-- pagebreak -->',
      '',
      '## 四、用户群体分层留存',
      '',
      '<!-- caption: 留存 -->',
      '| 人群 | 第一期 | 第二期 | 第三期 |',
      '| --- | --- | --- | --- |',
      '|  |  |  |  |',
      '',
      '## 五、商业增长与下阶段策略建议',
      '',
      '<!-- caption: 下一步 -->',
      '| 发现 | 动作 | 负责人 | 核对日期 |',
      '| --- | --- | --- | --- |',
      '|  |  |  |  |'
    ])
  }
]

export type PlanBlock =
  | { kind: 'h'; level: 1 | 2 | 3 | 4; text: string }
  | { kind: 'p'; text: string }
  | { kind: 'quote'; text: string }
  | { kind: 'ul'; items: string[] }
  | { kind: 'ol'; items: string[] }
  | { kind: 'code'; lang: string; text: string }
  | { kind: 'math'; text: string }
  | { kind: 'image'; alt: string; ref: string }
  | { kind: 'hr' }
  | { kind: 'table'; rows: string[][] }
  | { kind: 'break' }

type DraftStorage = Pick<Storage, 'getItem' | 'setItem'>

export function planCover(id: PlanCoverId): PlanCover {
  return PLAN_COVERS.find((item) => item.id === id) ?? PLAN_COVERS[0]
}

export function planTemplate(id: string): PlanTemplate | undefined {
  return PLAN_TEMPLATES.find((item) => item.id === id)
}

export function planTemplatesIn(category: PlanCategory): readonly PlanTemplate[] {
  if (category === 'all') return PLAN_TEMPLATES
  return PLAN_TEMPLATES.filter((item) => item.category === category)
}

export function planDate(now: number): string {
  const date = new Date(now)
  const month = String(date.getMonth() + 1).padStart(2, '0')
  const day = String(date.getDate()).padStart(2, '0')
  return `${date.getFullYear()}-${month}-${day}`
}

export function createPlanDraft(input: {
  template: PlanTemplate
  title: string
  subtitle: string
  organization: string
  department: string
  now: number
  id?: string
}): PlanDraft {
  const id = input.id && /^[A-Za-z0-9_-]{1,80}$/.test(input.id) ? input.id : planId(input.now)
  return {
    id,
    templateId: input.template.id,
    category: input.template.category,
    markdown: input.template.markdown,
    meta: {
      title: clip(input.title, FIELD_LIMIT),
      subtitle: clip(input.subtitle, FIELD_LIMIT),
      number: input.template.number,
      version: input.template.version,
      date: planDate(input.now),
      author: '',
      organization: clip(input.organization, FIELD_LIMIT),
      department: clip(input.department, FIELD_LIMIT),
      header: '',
      watermark: '',
      logo: ''
    },
    images: {},
    coverId: input.template.coverId,
    layout: input.template.layout,
    bodySize: 'md',
    showCover: true,
    style: { ...PLAN_STYLE_DEFAULT },
    updatedAt: input.now
  }
}

export function clipPlanMarkdown(value: string): { text: string; clipped: boolean } {
  if (value.length <= MARKDOWN_LIMIT) return { text: value, clipped: false }
  return { text: value.slice(0, MARKDOWN_LIMIT), clipped: true }
}

const PLAN_IMAGE_CAP = 8
const PLAN_IMAGE_ID = /^[a-z0-9]{1,40}$/
const PLAN_IMAGE_DATA = /^data:image\/(?:png|jpeg|webp|gif);base64,[A-Za-z0-9+/=\r\n]+$/

export function acceptPlanImage(bytes: number): boolean {
  return acceptPlanLogo(bytes)
}

/** Adds one local picture. Returns nothing when the picture or the cap is not accepted. */
export function planImageSlot(
  images: Record<string, string>,
  dataUrl: string,
  now: number
): { id: string; images: Record<string, string> } | null {
  if (!PLAN_IMAGE_DATA.test(dataUrl) || dataUrl.length > LOGO_LIMIT * 2) return null
  if (Object.keys(images).length >= PLAN_IMAGE_CAP) return null
  const id = `img${now.toString(36)}${Object.keys(images).length}`
  if (!PLAN_IMAGE_ID.test(id)) return null
  return { id, images: { ...images, [id]: dataUrl } }
}

export function insertPlanImage(
  source: string,
  start: number,
  alt: string,
  id: string
): { value: string; cursor: number } {
  const from = clamp(start, 0, source.length)
  const safeAlt = alt.replace(/[[\]]/g, '').slice(0, 80)
  const lead = from > 0 && source[from - 1] !== '\n' ? '\n' : ''
  const tail = from < source.length && source[from] !== '\n' ? '\n' : ''
  const insert = `${lead}![${safeAlt}](plan-image:${id})${tail}`
  return { value: source.slice(0, from) + insert + source.slice(from), cursor: from + insert.length }
}

/** Drops one stored picture and the Markdown line that points at it. */
export function removePlanImage(
  source: string,
  images: Record<string, string>,
  id: string
): { markdown: string; images: Record<string, string> } {
  const next = { ...images }
  delete next[id]
  const markdown = source
    .split('\n')
    .filter((line) => !line.includes(`(plan-image:${id})`))
    .join('\n')
  return { markdown, images: next }
}

export function resolvePlanImage(ref: string, images: Record<string, string>): string | null {
  if (ref.startsWith('plan-image:')) {
    const value = images[ref.slice('plan-image:'.length)]
    return value && PLAN_IMAGE_DATA.test(value) ? value : null
  }
  return /^https:\/\/[^\s]{1,480}$/.test(ref) ? ref : null
}

export function planStats(markdown: string): { chars: number; lines: number } {
  const chars = [...markdown.replace(/\s/g, '')].length
  const lines = markdown.length === 0 ? 0 : markdown.split('\n').length
  return { chars, lines }
}

export function planEditedPhrase(updatedAt: number, now: number): { key: string; count?: number } {
  const minutes = Math.floor(Math.max(0, now - updatedAt) / 60_000)
  if (minutes < 1) return { key: 'workbench.plan.draft.edited_just' }
  if (minutes < 60) return { key: 'workbench.plan.draft.edited_minutes', count: minutes }
  const hours = Math.floor(minutes / 60)
  if (hours < 24) return { key: 'workbench.plan.draft.edited_hours', count: hours }
  return { key: 'workbench.plan.draft.edited_days', count: Math.floor(hours / 24) }
}

export function parsePlanMarkdown(source: string): PlanBlock[] {
  const rows = source.replace(/\r\n/g, '\n').split('\n')
  const blocks: PlanBlock[] = []
  let index = 0
  while (index < rows.length) {
    const line = rows[index]
    if (line.trim() === '<!-- pagebreak -->') {
      blocks.push({ kind: 'break' })
      index += 1
      continue
    }
    const caption = /^<!--\s*caption:\s*(.*?)\s*-->$/.exec(line.trim())
    if (caption) {
      blocks.push({ kind: 'p', text: caption[1] })
      index += 1
      continue
    }
    if (line.startsWith('```')) {
      const langToken = line.slice(3).trim().toLowerCase()
      const lang = /^[a-z0-9_-]{1,20}$/.test(langToken) ? langToken : ''
      const buf: string[] = []
      index += 1
      while (index < rows.length && !rows[index].startsWith('```')) {
        buf.push(rows[index])
        index += 1
      }
      if (index < rows.length) index += 1
      blocks.push({ kind: 'code', lang, text: buf.join('\n') })
      continue
    }
    if (line.trim() === '$$') {
      const buf: string[] = []
      index += 1
      while (index < rows.length && rows[index].trim() !== '$$') {
        buf.push(rows[index])
        index += 1
      }
      if (index < rows.length) index += 1
      blocks.push({ kind: 'math', text: buf.join('\n').trim() })
      continue
    }
    const mathLine = /^\$\$(.+)\$\$\s*$/.exec(line.trim())
    if (mathLine) {
      blocks.push({ kind: 'math', text: mathLine[1].trim() })
      index += 1
      continue
    }
    if (line.trim() === '') {
      index += 1
      continue
    }
    const heading = /^(#{1,4})\s+(.*)$/.exec(line)
    if (heading) {
      blocks.push({ kind: 'h', level: heading[1].length as 1 | 2 | 3 | 4, text: heading[2] })
      index += 1
      continue
    }
    if (/^(-{3,}|\*{3,})$/.test(line.trim())) {
      blocks.push({ kind: 'hr' })
      index += 1
      continue
    }
    if (line.startsWith('|')) {
      const table: string[][] = []
      while (index < rows.length && rows[index].startsWith('|')) {
        const cells = rows[index]
          .split('|')
          .slice(1, -1)
          .map((cell) => cell.trim())
        if (!cells.every((cell) => /^:?-{3,}:?$/.test(cell))) table.push(cells)
        index += 1
      }
      if (table.length > 0) blocks.push({ kind: 'table', rows: table })
      continue
    }
    if (/^>\s?/.test(line)) {
      const buf: string[] = []
      while (index < rows.length && /^>\s?/.test(rows[index])) {
        buf.push(rows[index].replace(/^>\s?/, ''))
        index += 1
      }
      blocks.push({ kind: 'quote', text: buf.join('\n') })
      continue
    }
    if (/^[-*]\s+/.test(line)) {
      const items: string[] = []
      while (index < rows.length && /^[-*]\s+/.test(rows[index])) {
        items.push(rows[index].replace(/^[-*]\s+/, ''))
        index += 1
      }
      blocks.push({ kind: 'ul', items })
      continue
    }
    if (/^\d+\.\s+/.test(line)) {
      const items: string[] = []
      while (index < rows.length && /^\d+\.\s+/.test(rows[index])) {
        items.push(rows[index].replace(/^\d+\.\s+/, ''))
        index += 1
      }
      blocks.push({ kind: 'ol', items })
      continue
    }
    const image = planImageBlock(line)
    if (image) {
      blocks.push(image)
      index += 1
      continue
    }
    const buf: string[] = []
    while (index < rows.length && !startsBlock(rows[index])) {
      buf.push(rows[index])
      index += 1
    }
    blocks.push({ kind: 'p', text: buf.join('\n') })
  }
  return blocks
}

function startsBlock(line: string): boolean {
  return (
    line.trim() === '' ||
    line.startsWith('#') ||
    line.startsWith('|') ||
    line.startsWith('```') ||
    /^>\s?/.test(line) ||
    /^[-*]\s+/.test(line) ||
    /^\d+\.\s+/.test(line) ||
    line.trim() === '<!-- pagebreak -->' ||
    /^<!--\s*caption:.*-->$/.test(line.trim()) ||
    /^(-{3,}|\*{3,})$/.test(line.trim()) ||
    line.trim() === '$$' ||
    /^\$\$.+\$\$\s*$/.test(line.trim()) ||
    planImageBlock(line) != null
  )
}

function planImageBlock(line: string): PlanBlock | null {
  const match = /^!\[([^\]\n]*)\]\(([^)\s]+)\)\s*$/.exec(line.trim())
  if (!match) return null
  const ref = match[2]
  if (ref.startsWith('plan-image:')) {
    const id = ref.slice('plan-image:'.length)
    if (!PLAN_IMAGE_ID.test(id)) return null
    return { kind: 'image', alt: match[1], ref }
  }
  if (/^https:\/\/[^\s]{1,480}$/.test(ref)) return { kind: 'image', alt: match[1], ref }
  return null
}

function escapeHtml(value: string): string {
  return value.replace(/&/g, '&amp;').replace(/</g, '&lt;').replace(/>/g, '&gt;').replace(/"/g, '&quot;')
}

function blockHtml(block: PlanBlock, images: Record<string, string>): string {
  if (block.kind === 'h') return `<h${block.level}>${escapeHtml(block.text)}</h${block.level}>`
  if (block.kind === 'p') return `<p>${escapeHtml(block.text)}</p>`
  if (block.kind === 'quote') return `<blockquote>${escapeHtml(block.text)}</blockquote>`
  if (block.kind === 'ul') return `<ul>${block.items.map((item) => `<li>${escapeHtml(item)}</li>`).join('')}</ul>`
  if (block.kind === 'ol') return `<ol>${block.items.map((item) => `<li>${escapeHtml(item)}</li>`).join('')}</ol>`
  if (block.kind === 'code' || block.kind === 'math') return `<pre>${escapeHtml(block.text)}</pre>`
  if (block.kind === 'hr') return '<hr>'
  if (block.kind === 'table') {
    const rows = block.rows
      .map((row) => `<tr>${row.map((cell) => `<td>${escapeHtml(cell)}</td>`).join('')}</tr>`)
      .join('')
    return `<table>${rows}</table>`
  }
  if (block.kind === 'image') {
    const src = resolvePlanImage(block.ref, images)
    if (!src) return `<p>${escapeHtml(block.alt)}</p>`
    return `<img alt="${escapeHtml(block.alt)}" src="${escapeHtml(src)}">`
  }
  return ''
}

function watermarkHtml(draft: PlanDraft): string {
  if (!draft.style.watermarkOn) return ''
  if (draft.style.watermarkKind === 'image') {
    const src = draft.style.watermarkImage
    if (!src) return ''
    return `<img class="wm" alt="" src="${escapeHtml(src)}">`
  }
  if (!draft.meta.watermark) return ''
  return `<p class="wm">${escapeHtml(draft.meta.watermark)}</p>`
}

function renderPlanBlocks(
  blocks: PlanBlock[],
  images: Record<string, string>,
  style: PlanStyle,
  counters: { image: number; table: number; headings: PlanHeadingCounter }
): string {
  const parts: string[] = []
  for (let index = 0; index < blocks.length; index += 1) {
    const block = blocks[index]
    const next = blocks[index + 1]
    if (!block) continue
    if (block.kind === 'p' && next?.kind === 'table' && style.tableCaption) continue
    if (block.kind === 'h') {
      counters.headings = planBumpHeading(counters.headings, block.level)
      const label = planHeadingNumber(style.numbering, counters.headings, block.level)
      const text = label ? label + ' ' + block.text : block.text
      parts.push(`<h${block.level}>${escapeHtml(text)}</h${block.level}>`)
      continue
    }
    if (block.kind === 'ul' || block.kind === 'ol') {
      const items = block.items
        .map(
          (item, itemIndex) =>
            `<li><span>${escapeHtml(planListMark(block.kind, style, itemIndex))}</span> ${escapeHtml(item)}</li>`
        )
        .join('')
      parts.push(`<${block.kind === 'ul' ? 'ul' : 'ol'} class="plain">${items}</${block.kind === 'ul' ? 'ul' : 'ol'}>`)
      continue
    }
    if (block.kind === 'code') {
      parts.push(`<pre class="${style.code}">${escapeHtml(block.text)}</pre>`)
      continue
    }
    if (block.kind === 'image') {
      counters.image += 1
      const src = resolvePlanImage(block.ref, images)
      if (!src) {
        parts.push(`<p>${escapeHtml(block.alt)}</p>`)
        continue
      }
      const figure = `<img alt="${escapeHtml(block.alt)}" src="${escapeHtml(src)}">`
      if (!style.imageCaption) {
        parts.push(figure)
        continue
      }
      const caption = planCaptionLabel(style.imagePrefix, counters.image, block.alt, style.imageNumber)
      parts.push(
        caption
          ? `<figure class="${style.imageAlign}">${figure}<figcaption>${escapeHtml(caption)}</figcaption></figure>`
          : figure
      )
      continue
    }
    if (block.kind === 'table') {
      counters.table += 1
      const previous = index > 0 ? blocks[index - 1] : undefined
      const captionText =
        style.tableCaption && previous?.kind === 'p'
          ? planCaptionLabel(style.tablePrefix, counters.table, previous.text, style.tableNumber)
          : ''
      const rows = block.rows
        .map((row) => `<tr>${row.map((cell) => `<td>${escapeHtml(cell)}</td>`).join('')}</tr>`)
        .join('')
      const caption = captionText ? `<p class="cap ${style.tableAlign}">${escapeHtml(captionText)}</p>` : ''
      const table = `<table class="${style.table}">${rows}</table>`
      parts.push(style.tablePlace === 'below' ? table + caption : caption + table)
      continue
    }
    parts.push(blockHtml(block, images))
  }
  return parts.join('')
}

function blockWeight(block: PlanBlock): number {
  if (block.kind === 'h') return block.level === 1 ? 56 : 40
  if (block.kind === 'p' || block.kind === 'quote') return 28 + 22 * Math.ceil(block.text.length / 38)
  if (block.kind === 'ul' || block.kind === 'ol') return 12 + 26 * block.items.length
  if (block.kind === 'code' || block.kind === 'math') {
    const lines = block.text.length === 0 ? 1 : block.text.split('\n').length
    return 36 + 20 * lines
  }
  if (block.kind === 'image') return 220
  if (block.kind === 'table') return 36 + 28 * block.rows.length
  if (block.kind === 'hr') return 20
  return 24
}

/** A self-contained page for download and print. Body text is escaped. */
export function planDocumentHtml(draft: PlanDraft): string {
  const cover = planCover(draft.coverId)
  const title = escapeHtml(draft.meta.title || 'Plan')
  const facts = [draft.meta.number, draft.meta.version, draft.meta.date].filter(Boolean).join(' · ')
  const counters = { image: 0, table: 0, headings: [0, 0, 0, 0] as PlanHeadingCounter }
  const flow = planFlowPages(draft.markdown, draft.style.pagination, draft.style.tableCaption, draft.style.h1Break)
  const tocEntries = draft.style.tocOn ? planTocEntries(flow, draft.style.tocDepth) : []
  const tocChunks = draft.style.tocOn ? planTocChunks(tocEntries) : []
  const tocCount = tocChunks.length
  const pages = flow
    .map((blocks, index) => {
      const body = renderPlanBlocks(blocks, draft.images, draft.style, counters)
      return `<article class="page">${watermarkHtml(draft)}${body}<p class="num">${index + 2 + tocCount}</p></article>`
    })
    .join('')
  const tocTitle = draft.style.tocTitle ? `<h2>${escapeHtml(draft.style.tocTitle)}</h2>` : ''
  const tocArticle = tocChunks
    .map((chunk, index) => {
      const rows = chunk
        .map((entry) => {
          const page = entry.sheet + 2 + tocCount
          const mark = draft.style.tocPages ? ' ' + String(page) : ''
          return `<p style="padding-left:${(entry.level - 1) * 12}px">${escapeHtml(entry.text)}${mark}</p>`
        })
        .join('')
      return `<article class="page">${watermarkHtml(draft)}${tocTitle}${rows}<p class="num">${index + 2}</p></article>`
    })
    .join('')
  return `<!DOCTYPE html><html><head><meta charset="utf-8"><title>${title}</title><style>
    body{margin:0;background:#f5f5f5;color:#171717;font:13px/1.6 -apple-system,BlinkMacSystemFont,sans-serif}
    article{position:relative;width:460px;margin:16px auto;padding:32px;background:#fff;border:1px solid #e5e5e5}
    h1{font-size:28px;line-height:1.3;font-weight:500} h2{font-size:20px} h3,h4{font-size:16px}
    table{width:100%;border-collapse:collapse} td{border-bottom:1px solid #efefef;padding:4px}
    table.bordered td{border:1px solid #e5e5e5} table.striped tr:nth-child(even){background:#f7f7f7}
    ul.plain,ol.plain{list-style:none;padding:0}
    pre{white-space:pre-wrap;background:#f5f5f5;padding:12px;border-radius:8px}
    pre.dark{background:#171717;color:#f5f5f5}
    img{max-width:100%;border-radius:8px} .num{text-align:right;color:#8a8a8a}
    .cap{margin:0 0 6px;color:#8a8a8a;font-size:12px;text-align:center}
    .cap.left{text-align:left}.cap.right{text-align:right}
    figure{margin:0} figure.left{text-align:left} figure.center{text-align:center} figure.right{text-align:right}
    figcaption{color:#8a8a8a;font-size:12px}
    .wm{position:absolute;left:50%;top:50%;transform:translate(-50%,-50%) rotate(-12deg);color:rgba(0,0,0,.12);font-size:28px;pointer-events:none}
    img.wm{max-width:240px;opacity:.2}
  </style></head><body><article style="background:${cover.paper};color:${cover.ink}">${watermarkHtml(draft)}<p>${escapeHtml(facts)}</p><h1>${title}</h1><p>${escapeHtml(draft.meta.subtitle)}</p></article>${tocArticle}${pages}</body></html>`
}

export function planPages(source: string): PlanBlock[][] {
  const pages: PlanBlock[][] = [[]]
  for (const block of parsePlanMarkdown(source)) {
    if (block.kind === 'break') {
      pages.push([])
      continue
    }
    pages[pages.length - 1].push(block)
  }
  return pages.filter((page) => page.length > 0)
}

/** Manual keeps each page break. Auto also packs a long page by estimated height. */
export function planFlowPages(
  source: string,
  mode: PlanStyle['pagination'],
  tableCaption = true,
  h1Break = false
): PlanBlock[][] {
  const split = h1Break ? planPages(source).flatMap((page) => splitOnHeading(page)) : planPages(source)
  const logical = split.filter((page) => page.length > 0)
  if (mode !== 'auto') return logical
  const packed: PlanBlock[][] = []
  for (const page of logical) {
    let sheet: PlanBlock[] = []
    let used = 0
    for (let index = 0; index < page.length; index += 1) {
      const block = page[index]
      const next = page[index + 1]
      if (!block) continue
      const pair = tableCaption && block.kind === 'p' && next?.kind === 'table'
      const weight = blockWeight(block) + (pair && next ? blockWeight(next) : 0)
      if (sheet.length > 0 && used + weight > PAGE_BUDGET) {
        packed.push(sheet)
        sheet = []
        used = 0
      }
      sheet.push(block)
      used += blockWeight(block)
      if (pair && next) {
        sheet.push(next)
        used += blockWeight(next)
        index += 1
      }
    }
    if (sheet.length > 0) packed.push(sheet)
  }
  return packed
}

export function planCaptionLabel(prefix: string, index: number, text: string, numbered: boolean): string {
  const head = numbered ? prefix + String(index) : ''
  if (head && text) return head + '  ' + text
  if (head) return head
  return text
}

export function planCjkOrdinal(index: number): string {
  if (index >= 1 && index <= CJK_DIGITS.length) return CJK_DIGITS[index - 1] ?? String(index)
  return String(index)
}

export function planListMark(kind: 'ul' | 'ol', style: Pick<PlanStyle, 'bullet' | 'numbers'>, index: number): string {
  if (kind === 'ul') {
    if (style.bullet === 'square') return '\u25A0'
    if (style.bullet === 'check') return '\u2713'
    if (style.bullet === 'arrow') return '\u25B8'
    return '\u2022'
  }
  const number = index + 1
  if (style.numbers === 'paren') return '(' + String(number) + ')'
  if (style.numbers === 'cjk') return planCjkOrdinal(number) + CJK_ENUM
  return String(number) + '.'
}

export function planAssetUrl(value: string): string {
  return logoField(value) ?? ''
}

export type PlanHeadingCounter = [number, number, number, number]

export function planBumpHeading(counters: PlanHeadingCounter, level: 1 | 2 | 3 | 4): PlanHeadingCounter {
  const next: PlanHeadingCounter = [counters[0], counters[1], counters[2], counters[3]]
  next[level - 1] += 1
  if (level < 2) next[1] = 0
  if (level < 3) next[2] = 0
  if (level < 4) next[3] = 0
  return next
}

export function planHeadingNumber(
  mode: PlanStyle['numbering'],
  counters: PlanHeadingCounter,
  level: 1 | 2 | 3 | 4
): string {
  if (mode === 'none' || (mode === 'skip' && level === 1)) return ''
  if ((mode === 'cjk' || mode === 'mixed') && level === 1) return planCjkOrdinal(counters[0]) + CJK_ENUM
  if (mode === 'cjk' && level === 2) return CJK_OPEN + planCjkOrdinal(counters[1]) + CJK_CLOSE
  const start = mode === 'decimal' ? 0 : mode === 'cjk' ? 2 : 1
  const parts: string[] = []
  for (let index = start; index < level; index += 1) parts.push(String(counters[index]))
  return parts.join('.')
}

export function planTocEntries(
  pages: PlanBlock[][],
  depth: PlanStyle['tocDepth']
): { level: number; text: string; sheet: number }[] {
  const entries: { level: number; text: string; sheet: number }[] = []
  pages.forEach((blocks, sheet) => {
    for (const block of blocks) {
      if (block.kind === 'h' && block.level <= depth) entries.push({ level: block.level, text: block.text, sheet })
    }
  })
  return entries
}

/** One contents sheet stays a sheet even when it has no headings. */
export function planTocChunks<T>(entries: readonly T[]): T[][] {
  if (entries.length === 0) return [[]]
  const chunks: T[][] = []
  for (let index = 0; index < entries.length; index += PLAN_TOC_CHUNK) {
    chunks.push(entries.slice(index, index + PLAN_TOC_CHUNK))
  }
  return chunks
}

export function planAccent(coverAccent: string, ink: PlanStyle['ink']): string {
  if (ink === 'cover') return coverAccent
  return INK_COLOR[ink]
}

function splitOnHeading(page: PlanBlock[]): PlanBlock[][] {
  const groups: PlanBlock[][] = [[]]
  for (const block of page) {
    const current = groups[groups.length - 1]
    if (block.kind === 'h' && block.level === 1 && current && current.length > 0) groups.push([])
    groups[groups.length - 1]?.push(block)
  }
  return groups.filter((group) => group.length > 0)
}

export function planPageCount(source: string): number {
  const body = planPages(source).length
  return 1 + body
}

export function planHeadings(source: string): { level: number; text: string }[] {
  return parsePlanMarkdown(source).flatMap((block) =>
    block.kind === 'h' ? [{ level: block.level, text: block.text }] : []
  )
}

export function insertPlanMarkup(
  source: string,
  start: number,
  end: number,
  kind: PlanMarkup
): { value: string; cursor: number } {
  const from = clamp(Math.min(start, end), 0, source.length)
  const to = clamp(Math.max(start, end), 0, source.length)
  const selected = source.slice(from, to)
  let insert = ''
  let cursor = from
  if (kind === 'rule') {
    const lead = from > 0 && source[from - 1] !== '\n' ? '\n' : ''
    const tail = to < source.length && source[to] !== '\n' ? '\n' : ''
    insert = `${lead}---${tail}`
    cursor = from + insert.length
  } else if (
    kind === 'h1' ||
    kind === 'h2' ||
    kind === 'h3' ||
    kind === 'h4' ||
    kind === 'quote' ||
    kind === 'list' ||
    kind === 'ordered'
  ) {
    const prefix =
      kind === 'h1'
        ? '# '
        : kind === 'h2'
          ? '## '
          : kind === 'h3'
            ? '### '
            : kind === 'h4'
              ? '#### '
              : kind === 'quote'
                ? '> '
                : kind === 'ordered'
                  ? '1. '
                  : '- '
    const lead = from > 0 && source[from - 1] !== '\n' ? '\n' : ''
    insert = `${lead}${prefix}${selected}`
    cursor = from + insert.length
  } else if (kind === 'table' || kind === 'break') {
    const lead = from > 0 && source[from - 1] !== '\n' ? '\n' : ''
    const tail = to < source.length && source[to] !== '\n' ? '\n' : ''
    insert =
      kind === 'break'
        ? `${lead}<!-- pagebreak -->${tail}`
        : `${lead}<!-- caption:  -->\n|  |  |  |\n| --- | :---: | ---: |\n|  |  |  |${tail}`
    cursor = from + insert.length
  } else if (kind === 'link') {
    const label = selected.replace(/[[\]]/g, '')
    insert = `[${label}](https://)`
    cursor = from + label.length + 3
  } else if (kind === 'sup' || kind === 'sub') {
    insert = `<${kind}>${selected}</${kind}>`
    cursor = from + kind.length + 2 + selected.length
  } else {
    const mark = kind === 'bold' ? '**' : kind === 'code' ? '`' : '*'
    insert = `${mark}${selected}${mark}`
    cursor = from + mark.length + selected.length
  }
  return { value: source.slice(0, from) + insert + source.slice(to), cursor }
}

export function loadPlanDrafts(storage: Pick<Storage, 'getItem'>): PlanDraft[] {
  try {
    const raw = storage.getItem(PLAN_DRAFTS_KEY)
    if (!raw) return []
    const parsed = JSON.parse(raw) as unknown
    if (!Array.isArray(parsed)) return []
    return byRecent(
      parsed.flatMap((item) => {
        const draft = parseDraft(item)
        return draft ? [draft] : []
      })
    ).slice(0, DRAFT_LIMIT)
  } catch {
    return []
  }
}

export function savePlanDrafts(storage: DraftStorage, drafts: readonly PlanDraft[]): void {
  const clean = byRecent(
    drafts.flatMap((item) => {
      const draft = parseDraft(item)
      return draft ? [draft] : []
    })
  ).slice(0, DRAFT_LIMIT)
  storage.setItem(PLAN_DRAFTS_KEY, JSON.stringify(clean))
}

export function acceptPlanLogo(bytes: number): boolean {
  return bytes > 0 && bytes <= LOGO_LIMIT
}

function byRecent(drafts: PlanDraft[]): PlanDraft[] {
  return [...drafts].sort((left, right) => right.updatedAt - left.updatedAt)
}

function parseDraft(value: unknown): PlanDraft | null {
  if (!value || typeof value !== 'object' || Array.isArray(value)) return null
  const record = value as Record<string, unknown>
  if (typeof record.id !== 'string' || !/^[A-Za-z0-9_-]{1,80}$/.test(record.id)) return null
  if (typeof record.templateId !== 'string' || record.templateId.length > 40) return null
  if (typeof record.category !== 'string' || !TEMPLATE_CATEGORIES.has(record.category)) return null
  if (typeof record.markdown !== 'string') return null
  const markdown = record.markdown.length > MARKDOWN_LIMIT ? record.markdown.slice(0, MARKDOWN_LIMIT) : record.markdown
  if (typeof record.coverId !== 'string' || !COVER_IDS.has(record.coverId)) return null
  if (typeof record.layout !== 'string' || !LAYOUTS.has(record.layout)) return null
  if (typeof record.bodySize !== 'string' || !SIZES.has(record.bodySize)) return null
  if (typeof record.updatedAt !== 'number' || !Number.isFinite(record.updatedAt)) return null
  const meta = parseMeta(record.meta)
  if (!meta) return null
  return {
    id: record.id,
    templateId: record.templateId,
    category: record.category as PlanTemplateCategory,
    markdown,
    images: parseImages(record.images),
    meta,
    coverId: record.coverId as PlanCoverId,
    layout: record.layout as PlanLayout,
    bodySize: record.bodySize as PlanBodySize,
    showCover: record.showCover !== false,
    style: parsePlanStyle(record.style),
    updatedAt: record.updatedAt
  }
}

function parsePlanStyle(value: unknown): PlanStyle {
  const record = value && typeof value === 'object' && !Array.isArray(value) ? (value as Record<string, unknown>) : {}
  const image = logoField(record.watermarkImage)
  return {
    pagination: pick(record.pagination, PAGINATION, PLAN_STYLE_DEFAULT.pagination),
    bullet: pick(record.bullet, BULLETS, PLAN_STYLE_DEFAULT.bullet),
    numbers: pick(record.numbers, NUMBERS, PLAN_STYLE_DEFAULT.numbers),
    code: pick(record.code, CODE_THEMES, PLAN_STYLE_DEFAULT.code),
    table: pick(record.table, TABLE_STYLES, PLAN_STYLE_DEFAULT.table),
    imageFrame: pick(record.imageFrame, IMAGE_FRAMES, PLAN_STYLE_DEFAULT.imageFrame),
    imagePrefix: prefixField(record.imagePrefix, PLAN_STYLE_DEFAULT.imagePrefix),
    imageAlign: pick(record.imageAlign, ALIGNS, PLAN_STYLE_DEFAULT.imageAlign),
    imageCaption: flag(record.imageCaption, PLAN_STYLE_DEFAULT.imageCaption),
    imageNumber: flag(record.imageNumber, PLAN_STYLE_DEFAULT.imageNumber),
    tablePlace: pick(record.tablePlace, TABLE_PLACES, PLAN_STYLE_DEFAULT.tablePlace),
    tablePrefix: prefixField(record.tablePrefix, PLAN_STYLE_DEFAULT.tablePrefix),
    tableAlign: pick(record.tableAlign, ALIGNS, PLAN_STYLE_DEFAULT.tableAlign),
    tableCaption: flag(record.tableCaption, PLAN_STYLE_DEFAULT.tableCaption),
    tableNumber: flag(record.tableNumber, PLAN_STYLE_DEFAULT.tableNumber),
    watermarkOn: flag(record.watermarkOn, PLAN_STYLE_DEFAULT.watermarkOn),
    watermarkKind: pick(record.watermarkKind, WATERMARK_KINDS, PLAN_STYLE_DEFAULT.watermarkKind),
    watermarkImage: image ?? '',
    headerOn: flag(record.headerOn, PLAN_STYLE_DEFAULT.headerOn),
    headerLeft: clipText(record.headerLeft, PLAN_STYLE_DEFAULT.headerLeft, SLOT_LIMIT),
    headerRight: clipText(record.headerRight, PLAN_STYLE_DEFAULT.headerRight, SLOT_LIMIT),
    headerRule: pick(record.headerRule, HEADER_RULES, PLAN_STYLE_DEFAULT.headerRule),
    footerOn: flag(record.footerOn, PLAN_STYLE_DEFAULT.footerOn),
    footerLeft: clipText(record.footerLeft, PLAN_STYLE_DEFAULT.footerLeft, SLOT_LIMIT),
    footerCenter: clipText(record.footerCenter, PLAN_STYLE_DEFAULT.footerCenter, SLOT_LIMIT),
    footerRight: clipText(record.footerRight, PLAN_STYLE_DEFAULT.footerRight, SLOT_LIMIT),
    pageFormat: pick(record.pageFormat, PAGE_FORMATS, PLAN_STYLE_DEFAULT.pageFormat),
    pagePlace: pick(record.pagePlace, ALIGNS, PLAN_STYLE_DEFAULT.pagePlace),
    tocOn: flag(record.tocOn, PLAN_STYLE_DEFAULT.tocOn),
    tocTitle: clipText(record.tocTitle, PLAN_STYLE_DEFAULT.tocTitle, SLOT_LIMIT),
    tocDepth: depthField(record.tocDepth),
    tocLeader: pick(record.tocLeader, TOC_LEADERS, PLAN_STYLE_DEFAULT.tocLeader),
    tocPages: flag(record.tocPages, PLAN_STYLE_DEFAULT.tocPages),
    tocCenter: flag(record.tocCenter, PLAN_STYLE_DEFAULT.tocCenter),
    tocForm: pick(record.tocForm, TOC_FORMS, PLAN_STYLE_DEFAULT.tocForm),
    font: pick(record.font, FONTS, PLAN_STYLE_DEFAULT.font),
    numbering: pick(record.numbering, NUMBERINGS, PLAN_STYLE_DEFAULT.numbering),
    h1Form: pick(record.h1Form, H1_FORMS, PLAN_STYLE_DEFAULT.h1Form),
    h2Form: pick(record.h2Form, H2_FORMS, PLAN_STYLE_DEFAULT.h2Form),
    h1Break: flag(record.h1Break, PLAN_STYLE_DEFAULT.h1Break),
    h1Center: flag(record.h1Center, PLAN_STYLE_DEFAULT.h1Center),
    leading: pick(record.leading, LEADINGS, PLAN_STYLE_DEFAULT.leading),
    indent: flag(record.indent, PLAN_STYLE_DEFAULT.indent),
    ink: pick(record.ink, INKS, PLAN_STYLE_DEFAULT.ink),
    mermaid: pick(record.mermaid, MERMAID_THEMES, PLAN_STYLE_DEFAULT.mermaid)
  }
}

function depthField(value: unknown): PlanStyle['tocDepth'] {
  if (value === 1 || value === 2 || value === 3 || value === 4) return value
  return PLAN_STYLE_DEFAULT.tocDepth
}

function clipText(value: unknown, fallback: string, max: number): string {
  if (typeof value !== 'string') return fallback
  return value.slice(0, max)
}

function pick<T extends string>(value: unknown, allowed: readonly T[], fallback: T): T {
  if (typeof value !== 'string') return fallback
  for (const item of allowed) {
    if (item === value) return item
  }
  return fallback
}

function flag(value: unknown, fallback: boolean): boolean {
  return typeof value === 'boolean' ? value : fallback
}

function prefixField(value: unknown, fallback: string): string {
  if (typeof value !== 'string') return fallback
  return value.slice(0, PREFIX_LIMIT)
}

function parseImages(value: unknown): Record<string, string> {
  if (!value || typeof value !== 'object' || Array.isArray(value)) return {}
  const images: Record<string, string> = {}
  for (const [key, item] of Object.entries(value)) {
    if (!PLAN_IMAGE_ID.test(key) || typeof item !== 'string' || !PLAN_IMAGE_DATA.test(item)) continue
    if (item.length > LOGO_LIMIT * 2) continue
    images[key] = item
    if (Object.keys(images).length >= PLAN_IMAGE_CAP) break
  }
  return images
}

function parseMeta(value: unknown): PlanMeta | null {
  if (!value || typeof value !== 'object' || Array.isArray(value)) return null
  const record = value as Record<string, unknown>
  const title = field(record.title)
  const subtitle = field(record.subtitle)
  const number = field(record.number)
  const version = field(record.version)
  const date = field(record.date)
  const author = field(record.author)
  const organization = field(record.organization)
  const department = field(record.department)
  const header = field(record.header)
  const watermark = field(record.watermark)
  const logo = logoField(record.logo)
  if (
    title == null ||
    subtitle == null ||
    number == null ||
    version == null ||
    date == null ||
    author == null ||
    organization == null ||
    department == null ||
    header == null ||
    watermark == null ||
    logo == null
  ) {
    return null
  }
  return { title, subtitle, number, version, date, author, organization, department, header, watermark, logo }
}

function field(value: unknown): string | null {
  if (typeof value !== 'string') return null
  return value.length > FIELD_LIMIT ? value.slice(0, FIELD_LIMIT) : value
}

function logoField(value: unknown): string | null {
  if (typeof value !== 'string') return null
  if (value === '') return ''
  if (/^https:\/\/[^\s]{1,480}$/.test(value)) return value
  if (value.length > 250_000 || !/^data:image\/(?:png|jpeg|webp|gif);base64,[A-Za-z0-9+/=]+$/.test(value)) return ''
  return value
}

function planId(now: number): string {
  const stamp = Math.floor(now).toString(36)
  const noise = Math.floor(Math.random() * 1_679_616).toString(36)
  return `plan-${stamp}-${noise}`
}

function clip(value: string, max: number): string {
  return value.length > max ? value.slice(0, max) : value
}

function clamp(value: number, min: number, max: number): number {
  if (Number.isNaN(value)) return min
  return Math.min(max, Math.max(min, value))
}
