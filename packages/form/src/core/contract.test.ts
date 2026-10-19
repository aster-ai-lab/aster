import { describe, expect, it } from 'vitest';

import { compileForm as referenceCompileForm } from './compiler-reference';
import { canonicalize, digestDocument, sealDocument, sha256 } from './canonical';
import { DEFAULT_COMPILER_LIMITS } from './compiler-contract';
import { compileForm } from './compiler';
import { applyFormPatch, diffFormDocuments } from './patch';
import type { FormDocument } from './types';

// The contract suite pins the observable behavior of the shipped
// core: the embedded rust-wasm semantic compiler, the canonical
// digest discipline, the patch algebra, and the agreement between
// the reference TS implementation and the portable core.

function minimalDocument(title = '契约测试'): FormDocument {
  return {
    kind: 'a3s.form',
    apiVersion: 'a3s.dev/form/v1alpha1',
    revision: 0,
    metadata: { title, locale: 'zh-CN', tags: [] },
    schema: {
      $schema: 'https://json-schema.org/draft/2020-12/schema',
      type: 'object',
      properties: {},
      required: [],
      additionalProperties: false,
    },
    ui: {
      root: 'root',
      nodes: [
        { id: 'root', kind: 'root', label: title, columns: 12, children: [] },
      ],
    },
    rules: [],
    dataSources: [],
    actions: [],
  };
}

describe('the semantic compiler contract', () => {
  it('compiles a minimal document through the embedded core', () => {
    const result = compileForm(minimalDocument());
    expect(result.ok).toBe(true);
    if (!result.ok) return;
    expect(result.compilerRevision).toBeTruthy();
    expect(result.document?.kind).toBe('a3s.form');
    expect(result.plan).toBeDefined();
    expect(Array.isArray(result.diagnostics)).toBe(true);
  });

  it('refuses non-document input fail-closed with diagnostics', () => {
    for (const garbage of [null, 42, 'form', {}, { kind: 'a3s.form' }]) {
      const result = compileForm(garbage);
      expect(result.ok, `input ${JSON.stringify(garbage)}`).toBe(false);
      expect(result.diagnostics.length).toBeGreaterThan(0);
    }
  });

  it('enforces the frozen serialized-bytes limit', () => {
    const oversized = minimalDocument();
    oversized.metadata.description = 'x'.repeat(
      DEFAULT_COMPILER_LIMITS.maxSerializedBytes + 1,
    );
    expect(compileForm(oversized).ok).toBe(false);
  });

  it('enforces the frozen depth limit', () => {
    let deep: Record<string, unknown> = {};
    for (let i = 0; i < DEFAULT_COMPILER_LIMITS.maxDepth + 2; i += 1) {
      deep = { nested: deep };
    }
    const document = minimalDocument();
    document.schema = deep as never;
    expect(compileForm(document).ok).toBe(false);
  });
});

describe('the canonical digest discipline', () => {
  it('hashes the standard vector', () => {
    expect(sha256('abc')).toBe(
      'ba7816bf8f01cfea414140de5dae2223b00361a396177a9cb410ff61f20015ad',
    );
  });

  it('canonicalizes independent of key order', () => {
    const a = { z: 1, a: { y: [3, 2], x: null } };
    const b = { a: { x: null, y: [3, 2] }, z: 1 };
    expect(canonicalize(a as never)).toBe(canonicalize(b as never));
  });

  it('treats array order as significant', () => {
    const first = minimalDocument('数组有序');
    first.metadata.tags = ['a', 'b'];
    const second = minimalDocument('数组有序');
    second.metadata.tags = ['b', 'a'];
    expect(digestDocument(first)).not.toBe(digestDocument(second));
  });

  it('seals a document with its digest', () => {
    const sealed = sealDocument(minimalDocument('封存'));
    expect(sealed.digest).toBeTruthy();
    expect(sealed.digest).toBe(digestDocument(sealed));
  });
});

describe('the patch algebra', () => {
  // Both sides go through the compiler so they are normalized the
  // same way — patch application recompiles, and comparing against
  // an un-normalized document would diverge on compiler-injected
  // fields (e.g. the ui node `width`).
  const compiled = (title: string, revision: number): FormDocument => {
    const result = compileForm({ ...minimalDocument(title), revision });
    if (!result.ok || !result.document) {
      throw new Error('contract fixture failed to compile');
    }
    return result.document;
  };

  it('round-trips a diff between compiled revisions', () => {
    const before = compiled('初版', 0);
    const after = compiled('再版', 1);
    const patch = diffFormDocuments(before, after);
    expect(patch.baseRevision).toBe(before.revision);
    const applied = applyFormPatch(before, patch);
    expect(applied.ok).toBe(true);
    if (!applied.ok) return;
    expect(digestDocument(applied.document)).toBe(digestDocument(after));
  });

  it('refuses a patch against a stale revision', () => {
    const before = compiled('乐观锁', 0);
    const after = compiled('新稿', 2);
    const stale = { ...diffFormDocuments(before, after), baseRevision: 7 };
    const applied = applyFormPatch(before, stale as never);
    expect(applied.ok).toBe(false);
  });
});

describe('reference and portable cores agree', () => {
  it('produces the same digest for the same document', () => {
    const document = minimalDocument('差分一致性');
    const portable = compileForm(document);
    const reference = referenceCompileForm(document);
    expect(portable.ok).toBe(true);
    expect(reference.ok).toBe(true);
    if (!portable.ok || !reference.ok) return;
    expect(portable.document?.digest).toBe(reference.document?.digest);
    expect(canonicalize(portable.document as never)).toBe(
      canonicalize(reference.document as never),
    );
  });
});
