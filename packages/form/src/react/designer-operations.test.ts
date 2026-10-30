import { describe, expect, it, vi } from 'vitest';

import { compileForm } from '../core/compiler';
import { digestDocument } from '../core/canonical';
import type { FormDocument, JsonSchema, UiOption } from '../core/types';
import {
  allocateId,
  collectDescendants,
  compileMutation,
  matrixColumnsFromText,
  matrixRowsFromText,
  nextId,
  nextMatrixColumnValue,
  nextOptionValue,
  optionsFromText,
  reconcileOptionDefault,
  syncMatrixColumns,
  syncMatrixRows,
  syncOptions,
} from './designer-operations';

function minimalDocument(): FormDocument {
  return {
    kind: 'a3s.form',
    apiVersion: 'a3s.dev/form/v1alpha1',
    revision: 0,
    metadata: { title: '操作测试', locale: 'zh-CN', tags: [] },
    schema: {
      $schema: 'https://json-schema.org/draft/2020-12/schema',
      type: 'object',
      properties: {},
      required: [],
      additionalProperties: false,
    },
    ui: {
      root: 'root',
      nodes: [{ id: 'root', kind: 'root', label: '根节点', columns: 12, children: [] }],
    },
    rules: [],
    dataSources: [],
    actions: [],
  };
}

function optionDocument(widget: string, schema: JsonSchema, options: UiOption[]): FormDocument {
  const document = minimalDocument();
  document.schema.properties = { choice: schema };
  document.ui.nodes.push({
    id: 'choice',
    kind: 'field',
    schemaPath: '/properties/choice',
    widget,
    options,
  });
  return document;
}

function matrixDocument(widget: 'matrix-single' | 'matrix-multiple'): FormDocument {
  const document = minimalDocument();
  const rowSchema = (title: string): JsonSchema =>
    widget === 'matrix-multiple'
      ? { type: 'array', title, items: { type: 'string' } }
      : { type: 'string', title };
  document.schema.properties = {
    matrix: {
      type: 'object',
      properties: { 'row-1': rowSchema('第一行'), 'row-2': rowSchema('第二行') },
      required: ['row-1', 'row-2'],
    },
  };
  document.ui.nodes.push({
    id: 'matrix',
    kind: 'field',
    schemaPath: '/properties/matrix',
    widget,
    matrix: {
      rows: [
        { id: 'row-1', label: '第一行' },
        { id: 'row-2', label: '第二行' },
      ],
      columns: [
        { label: '否', value: 'no' },
        { label: '是', value: 'yes' },
      ],
    },
  });
  return document;
}

function schemaProperty(document: FormDocument, property: string): JsonSchema {
  const schema = document.schema.properties?.[property];
  if (!schema) throw new Error(`missing schema property: ${property}`);
  return schema;
}

describe('designer id allocation', () => {
  it('uses the first available deterministic id and reserves each allocation', () => {
    const existing = new Set(['field-1', 'field-3', 'section-1']);

    expect(allocateId(existing, 'field')).toBe('field-2');
    expect(allocateId(existing, 'field')).toBe('field-4');
    expect(existing).toEqual(new Set(['field-1', 'field-2', 'field-3', 'field-4', 'section-1']));
  });

  it('derives the next id from every document node without changing the document', () => {
    const document = minimalDocument();
    document.ui.nodes.push(
      { id: 'tab-1', kind: 'group' },
      { id: 'tab-3', kind: 'group' },
    );
    const before = structuredClone(document);

    expect(nextId(document, 'tab')).toBe('tab-2');
    expect(document).toEqual(before);
  });
});

describe('designer option value allocation', () => {
  it('allocates the remaining boolean value and reports exhaustion', () => {
    const options: UiOption[] = [{ label: '否', value: false }];
    const usedValues = new Set([JSON.stringify(false)]);

    expect(nextOptionValue(options, usedValues, 2)).toBe(true);
    expect(nextOptionValue(options, new Set(['false', 'true']), 2)).toBeUndefined();
    expect(usedValues).toEqual(new Set(['false']));
  });

  it('skips numeric conflicts from the preferred index', () => {
    const options: UiOption[] = [{ label: '一', value: 1 }];
    const usedValues = new Set(['2', '3']);

    expect(nextOptionValue(options, usedValues, 2)).toBe(4);
    expect(usedValues).toEqual(new Set(['2', '3']));
  });

  it('fills the first available string suffix without changing the options', () => {
    const options: UiOption[] = [{ label: '首项', value: 'option-1' }];
    const before = structuredClone(options);
    const usedValues = new Set(['"option-2"', '"option-4"']);

    expect(nextOptionValue(options, usedValues, 2)).toBe('option-3');
    expect(options).toEqual(before);
    expect(usedValues).toEqual(new Set(['"option-2"', '"option-4"']));
  });
});

describe('designer option default reconciliation', () => {
  const previous: UiOption[] = [
    { label: '甲', value: 'a' },
    { label: '乙', value: 'b' },
  ];
  const remapped: UiOption[] = [
    { label: '甲', value: 'alpha' },
    { label: '乙', value: 'beta' },
  ];

  it('preserves a single default that is still a valid option value', () => {
    const schema: JsonSchema = { default: 'a' };

    reconcileOptionDefault(schema, previous, [{ label: '改名', value: 'a' }], false);

    expect(schema.default).toBe('a');
  });

  it('remaps a single default only when the option keeps its position and label', () => {
    const schema: JsonSchema = { default: 'b' };
    const previousBefore = structuredClone(previous);
    const optionsBefore = structuredClone(remapped);

    reconcileOptionDefault(schema, previous, remapped, false);

    expect(schema.default).toBe('beta');
    expect(previous).toEqual(previousBefore);
    expect(remapped).toEqual(optionsBefore);
  });

  it('clears a single default when its option cannot be identified after editing', () => {
    const schema: JsonSchema = { default: 'b', title: '保留其他 Schema 字段' };

    reconcileOptionDefault(schema, previous, [{ label: '乙', value: 'renamed' }], false);

    expect(schema).toEqual({ title: '保留其他 Schema 字段' });
  });

  it('preserves and remaps multi-select defaults while dropping removed values', () => {
    const schema: JsonSchema = { default: ['a', 'removed', 'b'] };

    reconcileOptionDefault(schema, previous, remapped, true);

    expect(schema.default).toEqual(['alpha', 'beta']);
  });

  it('clears a multi-select default when none of its values survive', () => {
    const schema: JsonSchema = { default: ['a', 'b'] };

    reconcileOptionDefault(schema, previous, [{ label: '新项', value: 'new' }], true);

    expect(schema).not.toHaveProperty('default');
  });
});

describe('designer matrix column value allocation', () => {
  it('allocates the remaining boolean value and reports exhaustion', () => {
    const columns = [{ label: '否', value: false }];

    expect(nextMatrixColumnValue(columns, new Set(['false']), 2)).toBe(true);
    expect(nextMatrixColumnValue(columns, new Set(['false', 'true']), 2)).toBeUndefined();
  });

  it('skips numeric conflicts from the preferred index', () => {
    const columns = [{ label: '一', value: 1 }];

    expect(nextMatrixColumnValue(columns, new Set(['2', '3']), 2)).toBe(4);
  });

  it('skips string value conflicts without changing the columns or used values', () => {
    const columns = [{ label: '首项', value: 'option-1' }];
    const usedValues = new Set(['"option-2"', '"option-3"']);
    const before = structuredClone(columns);

    expect(nextMatrixColumnValue(columns, usedValues, 2)).toBe('option-4');
    expect(columns).toEqual(before);
    expect(usedValues).toEqual(new Set(['"option-2"', '"option-3"']));
  });
});

describe('designer text input reconciliation', () => {
  it('trims option labels, ignores blank lines, and skips conflicts for new values', () => {
    const previous: UiOption[] = [{ label: '旧标签', value: 'option-2' }];
    const before = structuredClone(previous);

    const options = optionsFromText(' 新标签 \n\n 新增项 \n', previous);

    expect(options).toEqual([
      { label: '新标签', value: 'option-2' },
      { label: '新增项', value: 'option-3' },
    ]);
    expect(options[0]).not.toBe(previous[0]);
    expect(previous).toEqual(before);
  });

  it('preserves matrix row ids and allocates unused ids after trimming input', () => {
    const previous = [{ id: 'row-2', label: '旧行' }];

    expect(matrixRowsFromText(' 保留行 \n\n 新增行\n第三行 ', previous)).toEqual([
      { id: 'row-2', label: '保留行' },
      { id: 'row-3', label: '新增行' },
      { id: 'row-4', label: '第三行' },
    ]);
    expect(previous).toEqual([{ id: 'row-2', label: '旧行' }]);
  });

  it('preserves matrix column values and resolves numeric conflicts for additions', () => {
    const previous = [
      { label: '旧一', value: 2 },
      { label: '旧二', value: 3 },
    ];

    expect(matrixColumnsFromText(' 第一列 \n第二列\n\n 第三列 ', previous)).toEqual([
      { label: '第一列', value: 2 },
      { label: '第二列', value: 3 },
      { label: '第三列', value: 4 },
    ]);
  });
});

describe('designer option schema synchronization', () => {
  it('binds options to the selected node schema and remaps its default', () => {
    const previous: UiOption[] = [
      { label: '甲', value: 'a' },
      { label: '乙', value: 'b' },
    ];
    const document = optionDocument(
      'select',
      { type: 'string', title: '选择', default: 'b' },
      previous,
    );
    const options: UiOption[] = [
      { label: '甲', value: 'alpha' },
      { label: '乙', value: 'beta' },
    ];

    syncOptions(document, 'choice', options);

    const node = document.ui.nodes.find((candidate) => candidate.id === 'choice');
    expect(node?.options).toEqual(options);
    expect(node?.options?.[0]).not.toBe(options[0]);
    expect(schemaProperty(document, 'choice')).toEqual({
      type: 'string',
      title: '选择',
      default: 'beta',
      enum: ['alpha', 'beta'],
    });
  });

  it('synchronizes multi-select items enum and preserves or remaps array defaults', () => {
    const document = optionDocument(
      'multi-select',
      {
        type: 'array',
        items: { type: 'string', title: '选项' },
        default: ['a', 'removed'],
        maxItems: 5,
      },
      [{ label: '甲', value: 'a' }],
    );

    syncOptions(document, 'choice', [{ label: '甲', value: 'alpha' }]);

    expect(schemaProperty(document, 'choice')).toEqual({
      type: 'array',
      items: { type: 'string', title: '选项', enum: ['alpha'] },
      default: ['alpha'],
      maxItems: 5,
    });
  });
});

describe('designer matrix schema synchronization', () => {
  it('remaps row schemas by position and preserves all-rows-required for renamed and new rows', () => {
    const document = matrixDocument('matrix-single');
    const rows = [
      { id: 'renamed-row', label: '改名行' },
      { id: 'row-2', label: '第二行' },
      { id: 'row-4', label: '新增行' },
    ];

    syncMatrixRows(document, 'matrix', rows);

    const schema = schemaProperty(document, 'matrix');
    expect(Object.keys(schema.properties ?? {})).toEqual(['renamed-row', 'row-2', 'row-4']);
    expect(schema.properties?.['renamed-row']).toEqual({ type: 'string', title: '改名行' });
    expect(schema.properties?.['row-4']).toEqual({ type: 'string', title: '新增行' });
    expect(schema.required).toEqual(['renamed-row', 'row-2', 'row-4']);
    const node = document.ui.nodes.find((candidate) => candidate.id === 'matrix');
    expect(node?.matrix?.rows).toEqual(rows);
    expect(node?.matrix?.rows[0]).not.toBe(rows[0]);
  });

  it('drops removed required rows without making new rows required when only some were required', () => {
    const document = matrixDocument('matrix-single');
    schemaProperty(document, 'matrix').required = ['row-1'];

    syncMatrixRows(document, 'matrix', [
      { id: 'row-1', label: '保留行' },
      { id: 'row-3', label: '新增行' },
    ]);

    const schema = schemaProperty(document, 'matrix');
    expect(schema.required).toEqual(['row-1']);
    expect(Object.keys(schema.properties ?? {})).toEqual(['row-1', 'row-3']);
  });

  it('updates matrix-multiple items enums and clamps minItems and maxItems to column count', () => {
    const document = matrixDocument('matrix-multiple');
    const matrixSchema = schemaProperty(document, 'matrix');
    Object.assign(matrixSchema.properties?.['row-1'] ?? {}, { minItems: 1, maxItems: 5 });
    Object.assign(matrixSchema.properties?.['row-2'] ?? {}, { minItems: 4, maxItems: 1 });
    const columns = [
      { label: '是', value: 'yes' },
      { label: '未知', value: 'unknown' },
    ];

    syncMatrixColumns(document, 'matrix', columns);

    expect(matrixSchema.properties?.['row-1']).toMatchObject({
      items: { type: 'string', enum: ['yes', 'unknown'] },
      minItems: 1,
      maxItems: 2,
    });
    expect(matrixSchema.properties?.['row-2']).toMatchObject({
      items: { type: 'string', enum: ['yes', 'unknown'] },
      minItems: 2,
      maxItems: 1,
    });
    const node = document.ui.nodes.find((candidate) => candidate.id === 'matrix');
    expect(node?.matrix?.columns).toEqual(columns);
    expect(node?.matrix?.columns[0]).not.toBe(columns[0]);
  });

  it('updates single-select matrix row enums without creating array items', () => {
    const document = matrixDocument('matrix-single');

    syncMatrixColumns(document, 'matrix', [{ label: '是', value: 'yes' }]);

    const matrixSchema = schemaProperty(document, 'matrix');
    expect(matrixSchema.properties?.['row-1']).toEqual({
      type: 'string',
      title: '第一行',
      enum: ['yes'],
    });
    expect(matrixSchema.properties?.['row-2']).toEqual({
      type: 'string',
      title: '第二行',
      enum: ['yes'],
    });
  });
});

describe('designer descendant collection', () => {
  it('collects the source and transitive descendants in traversal order', () => {
    const document = minimalDocument();
    document.ui.nodes = [
      { id: 'root', kind: 'root', children: ['branch', 'other'] },
      { id: 'branch', kind: 'group', children: ['leaf', 'last'] },
      { id: 'leaf', kind: 'field' },
      { id: 'last', kind: 'field' },
      { id: 'other', kind: 'field' },
    ];
    const before = structuredClone(document);

    expect([...collectDescendants(document, 'branch')]).toEqual(['branch', 'leaf', 'last']);
    expect(document).toEqual(before);
  });

  it('terminates when malformed child links contain a cycle', () => {
    const document = minimalDocument();
    document.ui.nodes = [
      { id: 'root', kind: 'root', children: ['a'] },
      { id: 'a', kind: 'group', children: ['b'] },
      { id: 'b', kind: 'group', children: ['a', 'leaf'] },
      { id: 'leaf', kind: 'field' },
    ];

    expect([...collectDescendants(document, 'a')]).toEqual(['a', 'b', 'leaf']);
  });
});

describe('designer compiled mutations', () => {
  it('mutates a clone, then advances revision and replaces a stale digest on success', () => {
    const document = minimalDocument();
    document.revision = 7;
    document.digest = 'stale-digest';
    const before = structuredClone(document);
    const onInvalid = vi.fn();
    let mutationState: { revision: number; digest?: string } | undefined;

    const next = compileMutation(
      document,
      (draft) => {
        mutationState = { revision: draft.revision, digest: draft.digest };
        draft.metadata.title = '更新后的标题';
      },
      undefined,
      onInvalid,
    );

    expect(mutationState).toEqual({ revision: 7, digest: 'stale-digest' });
    expect(next).toBeDefined();
    if (!next) throw new Error('expected a compiled mutation');
    expect(next.revision).toBe(8);
    expect(next.metadata.title).toBe('更新后的标题');
    expect(next.digest).not.toBe('stale-digest');
    expect(digestDocument(next)).toBe(next.digest);
    expect(document).toEqual(before);
    expect(onInvalid).not.toHaveBeenCalled();
  });

  it('rejects an invalid mutation and reports the first compiler diagnostic', () => {
    const document = minimalDocument();
    document.revision = 2;
    document.digest = 'stale-digest';
    const before = structuredClone(document);
    const onInvalid = vi.fn();

    const next = compileMutation(
      document,
      (draft) => {
        draft.ui.root = 'missing-root';
      },
      undefined,
      onInvalid,
    );

    const invalidDraft = structuredClone(document);
    invalidDraft.ui.root = 'missing-root';
    invalidDraft.revision += 1;
    delete invalidDraft.digest;
    const compiled = compileForm(invalidDraft);
    expect(compiled.ok).toBe(false);
    expect(compiled.diagnostics[0]?.message).toEqual(expect.any(String));
    expect(next).toBeUndefined();
    expect(onInvalid).toHaveBeenCalledExactlyOnceWith(compiled.diagnostics[0]?.message);
    expect(document).toEqual(before);
  });
});
