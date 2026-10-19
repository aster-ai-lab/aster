import { describe, expect, it } from 'vitest';

import type { FieldError as A3SFieldError } from './core/types';
import {
  toA3SFormErrors,
  toReactHookFormErrors,
} from './react-hooks';

// The React Hook Form binding contract: the error mappers are the
// typed boundary between RHF's nested FieldErrors and the core's
// flat A3SFieldError list. They are pure, so the contract pins them
// without a DOM.

const issue = (path: string, code: string, message: string): A3SFieldError =>
  ({ path, code, message }) as A3SFieldError;

describe('the react-hook-form error mappers', () => {
  it('maps a flat issue onto a nested FieldErrors tree', () => {
    const errors = toReactHookFormErrors([
      issue('name.first', 'required', '必填姓氏'),
    ]);
    const nested = errors as unknown as Record<string, Record<string, { type: string; message: string }>>;
    expect(nested.name?.first?.type).toBe('required');
    expect(nested.name?.first?.message).toBe('必填姓氏');
  });

  it('round-trips an issue list through both mappers', () => {
    const issues = [
      issue('email', 'format', '邮箱格式不正确'),
      issue('address.city', 'required', '城市必填'),
      issue('tags.0', 'maxLength', '过长'),
    ];
    const back = toA3SFormErrors(toReactHookFormErrors(issues));
    expect(back).toEqual(issues);
  });

  it('expands a multi-type error into one issue per type', () => {
    const errors = {
      age: {
        type: 'validate',
        message: '校验失败',
        types: { min: '太小', max: '太大' },
      },
    };
    const issues = toA3SFormErrors(errors as never);
    expect(issues).toEqual([
      { path: 'age', code: 'validate', message: '校验失败' },
      { path: 'age', code: 'min', message: '太小' },
      { path: 'age', code: 'max', message: '太大' },
    ]);
  });

  it('drops empty messages, ignores refs, and dedupes identical triples', () => {
    const errors = {
      a: { type: 't', message: '', ref: {} },
      b: { type: 't', message: '重复' },
      b2: { type: 't', message: '重复' },
    };
    const first = toA3SFormErrors(errors as never);
    const doubled = toA3SFormErrors({
      ...errors,
      b3: { type: 't', message: '重复' },
    } as never);
    expect(first).toEqual([
      { path: 'b', code: 't', message: '重复' },
      { path: 'b2', code: 't', message: '重复' },
    ]);
    expect(doubled.length).toBe(first.length + 1);
  });
});
