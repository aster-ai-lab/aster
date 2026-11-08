import { analyzeExpression, type FormExpression, type JsonObject, type JsonValue } from '@aster-ai-lab/form/core';
import { ASTER_FLOW_EXPRESSION_API_VERSION } from '../integrations/aster-flow-core';

export type AsterFlowExpressionPurpose =
  | 'condition'
  | 'datetime'
  | 'error'
  | 'input'
  | 'output'
  | 'run-id'
  | 'token';

export const ASTER_FLOW_COMPARISON_OPERATORS = [
  'eq',
  'ne',
  'gt',
  'gte',
  'lt',
  'lte',
  'contains',
  'in',
] as const;

export type AsterFlowComparisonOperator = (typeof ASTER_FLOW_COMPARISON_OPERATORS)[number];
export type AsterFlowComparisonExpression = FormExpression & {
  op: AsterFlowComparisonOperator;
  left: FormExpression;
  right: FormExpression;
};

export function isJsonObjectValue(value: JsonValue | undefined): value is JsonObject {
  return Boolean(value) && typeof value === 'object' && !Array.isArray(value);
}

export function asterFlowExpressionFrom(value: JsonValue | undefined): FormExpression | undefined {
  if (
    !isJsonObjectValue(value) ||
    value.apiVersion !== ASTER_FLOW_EXPRESSION_API_VERSION ||
    Object.keys(value).some((key) => key !== 'apiVersion' && key !== 'expression')
  ) {
    return undefined;
  }
  try {
    analyzeExpression(value.expression);
    return value.expression as FormExpression;
  } catch {
    return undefined;
  }
}

export function isAsterFlowComparisonExpression(
  expression: FormExpression,
): expression is AsterFlowComparisonExpression {
  return ASTER_FLOW_COMPARISON_OPERATORS.some((operator) => operator === expression.op);
}

export function asterFlowExpressionLiteralText(value: JsonValue): string {
  if (typeof value === 'string') return value;
  return JSON.stringify(value) ?? '';
}

export function asterFlowExpressionToTemplate(expression: FormExpression): string {
  if (expression.op !== 'concat') return '';
  return expression.values
    .map((part) => {
      if (part.op === 'field') return `{{${part.path}}}`;
      if (part.op === 'literal') return String(part.value ?? '');
      return '{{advanced}}';
    })
    .join('');
}

export function asterFlowComparisonOperatorLabel(
  operator: AsterFlowComparisonOperator,
  chinese: boolean,
): string {
  const labels = chinese
    ? {
        eq: '等于',
        ne: '不等于',
        gt: '大于',
        gte: '大于等于',
        lt: '小于',
        lte: '小于等于',
        contains: '包含',
        in: '属于',
      }
    : {
        eq: 'Equals',
        ne: 'Does not equal',
        gt: 'Greater than',
        gte: 'At least',
        lt: 'Less than',
        lte: 'At most',
        contains: 'Contains',
        in: 'Is in',
      };
  return labels[operator];
}

export function asterFlowExpressionPreviewText(
  expression: FormExpression,
  purpose: AsterFlowExpressionPurpose,
  chinese: boolean,
  style: 'sentence' | 'compact' = 'sentence',
): string {
  if (purpose === 'run-id' && expression.op === 'literal' && expression.value === null) {
    if (style === 'compact') return chinese ? '自动生成' : 'Generated automatically';
    return chinese
      ? '每次启动工作流时自动创建运行 ID。'
      : 'A run ID is created whenever the workflow starts.';
  }
  if (expression.op === 'field') {
    if (style === 'compact') return expression.path;
    return chinese
      ? `使用字段 ${expression.path} 的值。`
      : `Use the value from ${expression.path}.`;
  }
  if (expression.op === 'literal') {
    const value = asterFlowExpressionLiteralText(expression.value);
    if (style === 'compact') return value;
    return chinese ? `使用固定值：${value}` : `Use fixed value: ${value}`;
  }
  if (isAsterFlowComparisonExpression(expression)) {
    const left = expression.left.op === 'field' ? expression.left.path : '…';
    const right =
      expression.right.op === 'literal'
        ? asterFlowExpressionLiteralText(expression.right.value)
        : '…';
    return `${left} ${asterFlowComparisonOperatorLabel(expression.op, chinese)} ${right}`;
  }
  if (expression.op === 'concat') {
    const template = asterFlowExpressionToTemplate(expression);
    if (style === 'compact') return template;
    return chinese ? `生成文本：${template}` : `Build text: ${template}`;
  }
  if (style === 'compact') return chinese ? '高级表达式' : 'Advanced expression';
  return chinese ? '使用高级表达式计算结果。' : 'Evaluate the advanced expression.';
}
