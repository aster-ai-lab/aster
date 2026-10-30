import {
  type CompileOptions,
  compileForm,
  type FormDocument,
  type JsonPrimitive,
  type JsonSchema,
  type UiMatrixColumn,
  type UiMatrixRow,
  type UiMatrixValue,
  type UiOption,
} from '../core';
import { schemaBindingForNode } from './designer-schema';

export function allocateId(existing: Set<string>, prefix: string): string {
  let index = 1;
  while (existing.has(`${prefix}-${index}`)) index += 1;
  const id = `${prefix}-${index}`;
  existing.add(id);
  return id;
}

export function nextId(document: FormDocument, prefix: string): string {
  return allocateId(new Set(document.ui.nodes.map((node) => node.id)), prefix);
}

function nextChoiceValue<T extends JsonPrimitive>(
  options: readonly { value: T }[],
  usedValues: ReadonlySet<string>,
  preferredIndex: number,
): T | undefined {
  const valueType = typeof options[0]?.value;
  if (valueType === 'boolean') {
    return [false, true].find((value) => !usedValues.has(JSON.stringify(value))) as T | undefined;
  }
  if (valueType === 'number') {
    let value = preferredIndex;
    while (usedValues.has(JSON.stringify(value))) value += 1;
    return value as T;
  }
  let suffix = preferredIndex;
  let value = `option-${suffix}`;
  while (usedValues.has(JSON.stringify(value))) {
    suffix += 1;
    value = `option-${suffix}`;
  }
  return value as T;
}

export function nextOptionValue(
  options: readonly UiOption[],
  usedValues: ReadonlySet<string>,
  preferredIndex: number,
): JsonPrimitive | undefined {
  return nextChoiceValue(options, usedValues, preferredIndex);
}

export function nextMatrixColumnValue(
  columns: readonly UiMatrixColumn[],
  usedValues: ReadonlySet<string>,
  preferredIndex: number,
): UiMatrixValue | undefined {
  return nextChoiceValue(columns, usedValues, preferredIndex);
}

function inputLabels(text: string): string[] {
  return text
    .split('\n')
    .map((label) => label.trim())
    .filter(Boolean);
}

export function optionsFromText(text: string, previous: readonly UiOption[]): UiOption[] {
  const usedValues = new Set(previous.map(({ value }) => JSON.stringify(value)));
  return inputLabels(text).flatMap((label, index) => {
    const existing = previous[index];
    if (existing) return [{ ...existing, label }];
    const value = nextOptionValue(previous, usedValues, index + 1);
    if (value === undefined) return [];
    usedValues.add(JSON.stringify(value));
    return [{ label, value }];
  });
}

export function matrixRowsFromText(
  text: string,
  previousRows: readonly UiMatrixRow[],
): UiMatrixRow[] {
  const usedIds = new Set(previousRows.map(({ id }) => id));
  return inputLabels(text).map((label, index) => {
    const previous = previousRows[index];
    if (previous) return { ...previous, label };
    let suffix = index + 1;
    while (usedIds.has(`row-${suffix}`)) suffix += 1;
    const id = `row-${suffix}`;
    usedIds.add(id);
    return { id, label };
  });
}

export function matrixColumnsFromText(
  text: string,
  previousColumns: readonly UiMatrixColumn[],
): UiMatrixColumn[] {
  const usedValues = new Set(previousColumns.map(({ value }) => JSON.stringify(value)));
  return inputLabels(text).flatMap((label, index) => {
    const previous = previousColumns[index];
    if (previous) return [{ ...previous, label }];
    const value = nextMatrixColumnValue(previousColumns, usedValues, index + 1);
    if (value === undefined) return [];
    usedValues.add(JSON.stringify(value));
    return [{ label, value }];
  });
}

function isJsonPrimitive(value: unknown): value is JsonPrimitive {
  return value === null || ['string', 'number', 'boolean'].includes(typeof value);
}

export function reconcileOptionDefault(
  schema: JsonSchema,
  previous: readonly UiOption[],
  options: readonly UiOption[],
  multiple: boolean,
): void {
  const values = options.map(({ value }) => value);
  const resolve = (value: JsonPrimitive): JsonPrimitive | undefined => {
    if (values.some((candidate) => Object.is(candidate, value))) return value;
    const previousIndex = previous.findIndex((candidate) => Object.is(candidate.value, value));
    const previousOption = previous[previousIndex];
    const nextOption = options[previousIndex];
    return previousOption && nextOption && previousOption.label === nextOption.label
      ? nextOption.value
      : undefined;
  };

  if (multiple) {
    if (!Array.isArray(schema.default)) return;
    const next = schema.default.filter(isJsonPrimitive).flatMap((value) => {
      const resolved = resolve(value);
      return resolved === undefined ? [] : [resolved];
    });
    if (next.length > 0) schema.default = next;
    else delete schema.default;
    return;
  }

  if (!isJsonPrimitive(schema.default)) return;
  const next = resolve(schema.default);
  if (next === undefined) delete schema.default;
  else schema.default = next;
}

export function syncOptions(
  document: FormDocument,
  selectedId: string,
  options: UiOption[],
): void {
  const node = document.ui.nodes.find((candidate) => candidate.id === selectedId);
  const previous = node?.options ?? [];
  if (node) node.options = options.map((option) => ({ ...option }));
  const binding = schemaBindingForNode(document, node);
  if (!binding) return;
  const values = options.map(({ value }) => value);
  if (node?.widget === 'multi-select') {
    binding.schema.items ??= { type: 'string' };
    binding.schema.items.enum = values;
    reconcileOptionDefault(binding.schema, previous, options, true);
  } else binding.schema.enum = values;
  if (node?.widget !== 'multi-select') {
    reconcileOptionDefault(binding.schema, previous, options, false);
  }
}

export function syncMatrixRows(
  document: FormDocument,
  selectedId: string,
  rows: UiMatrixRow[],
): void {
  if (rows.length === 0) return;
  const node = document.ui.nodes.find((candidate) => candidate.id === selectedId);
  const binding = schemaBindingForNode(document, node);
  if (!node?.matrix || !binding?.schema.properties) return;
  const previousRows = node.matrix.rows;
  const previousProperties = binding.schema.properties;
  const template = Object.values(previousProperties)[0];
  const previousIds = new Set(previousRows.map(({ id }) => id));
  const previousRequired = new Set(binding.schema.required ?? []);
  const allRowsRequired =
    previousRows.length > 0 && previousRows.every(({ id }) => previousRequired.has(id));
  const properties: Record<string, JsonSchema> = {};
  const required: string[] = [];
  for (const [rowIndex, row] of rows.entries()) {
    const previousRow = previousRows[rowIndex];
    const existing = previousProperties[row.id];
    const renamed = previousRow ? previousProperties[previousRow.id] : undefined;
    const schema = structuredClone(existing ?? renamed ?? template);
    if (!schema) continue;
    schema.title = row.label;
    properties[row.id] = schema;
    if (
      previousRequired.has(row.id) ||
      (previousRow !== undefined && previousRequired.has(previousRow.id)) ||
      (!previousIds.has(row.id) && allRowsRequired)
    ) {
      required.push(row.id);
    }
  }
  node.matrix.rows = rows.map((row) => ({ ...row }));
  binding.schema.properties = properties;
  binding.schema.required = required;
}

export function syncMatrixColumns(
  document: FormDocument,
  selectedId: string,
  columns: UiMatrixColumn[],
): void {
  if (columns.length === 0) return;
  const node = document.ui.nodes.find((candidate) => candidate.id === selectedId);
  const binding = schemaBindingForNode(document, node);
  if (!node?.matrix || !binding?.schema.properties) return;
  const values = columns.map(({ value }) => value);
  node.matrix.columns = columns.map((column) => ({ ...column }));
  for (const rowSchema of Object.values(binding.schema.properties)) {
    if (node.widget === 'matrix-multiple') {
      rowSchema.items ??= { type: 'string' };
      rowSchema.items.enum = values;
      if (rowSchema.maxItems !== undefined) {
        rowSchema.maxItems = Math.min(rowSchema.maxItems, values.length);
      }
      if (rowSchema.minItems !== undefined) {
        rowSchema.minItems = Math.min(rowSchema.minItems, values.length);
      }
    } else rowSchema.enum = values;
  }
}

export function compileMutation(
  document: FormDocument,
  mutate: (draft: FormDocument) => void,
  options?: CompileOptions,
  onInvalid?: (message: string) => void,
): FormDocument | undefined {
  const draft = structuredClone(document);
  mutate(draft);
  draft.revision += 1;
  delete draft.digest;
  const result = compileForm(draft, options);
  if (!result.ok) {
    onInvalid?.(result.diagnostics[0]?.message ?? '这项修改未通过表单编译校验。');
  }
  return result.ok ? result.document : undefined;
}

export function collectDescendants(
  document: FormDocument,
  id: string,
  output = new Set<string>(),
): Set<string> {
  if (output.has(id)) return output;
  output.add(id);
  for (const child of document.ui.nodes.find((node) => node.id === id)?.children ?? [])
    collectDescendants(document, child, output);
  return output;
}
