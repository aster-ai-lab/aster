import {
  type ComputedRef,
  computed,
  type InjectionKey,
  inject,
  type MaybeRefOrGetter,
  provide,
  readonly,
  type ShallowRef,
  shallowRef,
  toValue,
  type WritableComputedRef,
  watch,
} from 'vue';
import {
  type FieldError,
  type FormHostAdapter,
  type FormLocaleCatalogOverride,
  type FormPlan,
  getAtPath,
  IncrementalComputedRuleEvaluator,
  type JsonObject,
  type JsonValue,
  jsonValuesEqual,
  matchValuePathTemplate,
  updateFormValue,
  validateFormValue,
  validateFormValueAsync,
} from './core';

export type AsterVueFormValidationMode = 'onSubmit' | 'onBlur' | 'onChange' | 'all';

export interface UseAsterVueFormOptions<TFieldValues extends JsonObject = JsonObject> {
  plan: MaybeRefOrGetter<FormPlan>;
  initialValues?: TFieldValues;
  hostAdapter?: MaybeRefOrGetter<FormHostAdapter | undefined>;
  locale?: MaybeRefOrGetter<string | undefined>;
  localeCatalog?: MaybeRefOrGetter<FormLocaleCatalogOverride | undefined>;
  mode?: AsterVueFormValidationMode;
}

export interface AsterVueFormSetValueOptions {
  shouldDirty?: boolean;
  shouldTouch?: boolean;
  shouldValidate?: boolean;
}

export interface AsterVueFormState {
  isDirty: boolean;
  isValid: boolean;
  isValidating: boolean;
  isSubmitting: boolean;
  isSubmitted: boolean;
  isSubmitSuccessful: boolean;
  submitCount: number;
  dirtyFields: ReadonlySet<string>;
  touchedFields: ReadonlySet<string>;
  validatingFields: ReadonlySet<string>;
  errors: readonly FieldError[];
}

export interface AsterVueFieldBinding<TValue extends JsonValue = JsonValue> {
  readonly name: string;
  readonly modelValue: TValue | undefined;
  readonly 'onUpdate:modelValue': (value: TValue) => void;
  readonly onBlur: () => void;
}

export interface AsterVueFormContext<TFieldValues extends JsonObject = JsonObject> {
  plan: ComputedRef<FormPlan>;
  values: Readonly<ShallowRef<TFieldValues>>;
  errors: Readonly<ShallowRef<readonly FieldError[]>>;
  formState: ComputedRef<AsterVueFormState>;
  rendererProps: ComputedRef<{
    plan: FormPlan;
    value: JsonObject;
    onChange: (value: JsonObject) => void;
    errors: readonly FieldError[];
    hostAdapter?: FormHostAdapter;
    locale?: string;
    localeCatalog?: FormLocaleCatalogOverride;
  }>;
  getValue<TValue extends JsonValue = JsonValue>(path: string): TValue | undefined;
  setValue(path: string, value: JsonValue, options?: AsterVueFormSetValueOptions): void;
  setValues(value: TFieldValues, options?: AsterVueFormSetValueOptions): void;
  setError(error: FieldError): void;
  clearErrors(path?: string | readonly string[]): void;
  register<TValue extends JsonValue = JsonValue>(path: string): AsterVueFieldBinding<TValue>;
  validate(path?: string | readonly string[]): Promise<boolean>;
  reset(value?: TFieldValues): void;
  handleSubmit(
    onValid: (value: TFieldValues) => void | Promise<void>,
    onInvalid?: (errors: readonly FieldError[]) => void | Promise<void>,
  ): (event?: { preventDefault?: () => void }) => Promise<void>;
}

export interface UseAsterVueFieldOptions<
  TFieldValues extends JsonObject = JsonObject,
  TValue extends JsonValue = JsonValue,
> {
  name: string;
  form?: AsterVueFormContext<TFieldValues>;
  validateOnValueUpdate?: boolean;
  transform?: (value: TValue) => JsonValue;
}

export interface UseAsterVueFieldReturn<TValue extends JsonValue = JsonValue> {
  name: string;
  value: WritableComputedRef<TValue | undefined>;
  errors: ComputedRef<readonly FieldError[]>;
  errorMessage: ComputedRef<string | undefined>;
  meta: ComputedRef<{
    dirty: boolean;
    touched: boolean;
    valid: boolean;
    validating: boolean;
  }>;
  setValue(value: TValue): void;
  handleBlur(): void;
  validate(): Promise<boolean>;
}

export interface AsterVueFieldArrayEntry<TValue extends JsonValue = JsonValue> {
  key: string;
  index: number;
  value: TValue;
}

export interface UseAsterVueFieldArrayOptions<TFieldValues extends JsonObject = JsonObject> {
  name: string;
  form?: AsterVueFormContext<TFieldValues>;
  keyFactory?: () => string;
}

export interface UseAsterVueFieldArrayReturn<TValue extends JsonValue = JsonValue> {
  fields: ComputedRef<readonly AsterVueFieldArrayEntry<TValue>[]>;
  append(value: TValue): void;
  prepend(value: TValue): void;
  insert(index: number, value: TValue): void;
  remove(index?: number | readonly number[]): void;
  swap(left: number, right: number): void;
  move(from: number, to: number): void;
  update(index: number, value: TValue): void;
  replace(value: readonly TValue[]): void;
}

const formContextKey = Symbol('Aster UI Form') as InjectionKey<AsterVueFormContext<JsonObject>>;
let fieldArrayIdentity = 0;

function clone<T>(value: T): T {
  return structuredClone(value);
}

function pathMatchesScope(candidate: string, scope: string): boolean {
  return (
    candidate === scope || candidate.startsWith(`${scope}.`) || scope.startsWith(`${candidate}.`)
  );
}

function formFieldScope(plan: FormPlan, path: string | readonly string[] | undefined) {
  const paths = typeof path === 'string' ? [path] : path;
  if (paths?.length !== 1) return { kind: 'form' } as const;
  const valuePath = paths[0];
  const node = plan.nodes.find(
    (candidate) =>
      candidate.valuePath === valuePath ||
      matchValuePathTemplate(candidate.valuePathTemplate, valuePath) !== undefined,
  );
  return node
    ? ({ kind: 'field', nodeId: node.id, path: valuePath } as const)
    : ({ kind: 'form' } as const);
}

function fieldPaths(plan: FormPlan): string[] {
  return [
    ...new Set(
      plan.nodes.map((node) => node.valuePath).filter((path): path is string => Boolean(path)),
    ),
  ];
}

export function useAsterForm<TFieldValues extends JsonObject = JsonObject>(
  options: UseAsterVueFormOptions<TFieldValues>,
): AsterVueFormContext<TFieldValues> {
  const plan = computed(() => toValue(options.plan));
  const initial = clone(options.initialValues ?? ({} as TFieldValues));
  const defaultValues = shallowRef<TFieldValues>(clone(initial));
  const internalValues = shallowRef<TFieldValues>(clone(initial));
  const internalErrors = shallowRef<readonly FieldError[]>([]);
  const dirtyFields = shallowRef<ReadonlySet<string>>(new Set());
  const touchedFields = shallowRef<ReadonlySet<string>>(new Set());
  const validatingFields = shallowRef<ReadonlySet<string>>(new Set());
  const isValidating = shallowRef(false);
  const isSubmitting = shallowRef(false);
  const isSubmitted = shallowRef(false);
  const isSubmitSuccessful = shallowRef(false);
  const submitCount = shallowRef(0);
  const evaluator = new IncrementalComputedRuleEvaluator();
  let activeValidation: AbortController | undefined;

  const hostAdapter = () => (options.hostAdapter ? toValue(options.hostAdapter) : undefined);
  const locale = () => (options.locale ? toValue(options.locale) : undefined);
  const localeCatalog = () => (options.localeCatalog ? toValue(options.localeCatalog) : undefined);

  const setDerivedValues = (next: TFieldValues) => {
    internalValues.value = evaluator.evaluate(plan.value, next).value as TFieldValues;
  };

  const updateDirtyFields = (paths: readonly string[]) => {
    const next = new Set(dirtyFields.value);
    for (const path of paths) {
      if (
        jsonValuesEqual(getAtPath(internalValues.value, path), getAtPath(defaultValues.value, path))
      ) {
        next.delete(path);
      } else {
        next.add(path);
      }
    }
    dirtyFields.value = next;
  };

  const validate = async (path?: string | readonly string[]): Promise<boolean> => {
    activeValidation?.abort();
    const controller = new AbortController();
    activeValidation = controller;
    const scope = formFieldScope(plan.value, path);
    const paths = typeof path === 'string' ? [path] : (path ?? []);
    isValidating.value = true;
    validatingFields.value = new Set(paths);
    const result = await validateFormValueAsync(
      plan.value,
      internalValues.value,
      hostAdapter()?.validateValue,
      {
        scope,
        locale: locale(),
        localeCatalog: localeCatalog(),
      },
      controller.signal,
    );
    if (activeValidation !== controller || result.status === 'cancelled') return false;
    activeValidation = undefined;
    isValidating.value = false;
    validatingFields.value = new Set();
    internalValues.value = result.value as TFieldValues;
    internalErrors.value = result.errors;
    return result.errors.length === 0;
  };

  const setValue = (path: string, value: JsonValue, setOptions: AsterVueFormSetValueOptions = {}) => {
    setDerivedValues(updateFormValue(internalValues.value, path, value) as TFieldValues);
    if (setOptions.shouldDirty !== false) updateDirtyFields([path]);
    if (setOptions.shouldTouch) touchedFields.value = new Set([...touchedFields.value, path]);
    if (setOptions.shouldValidate ?? (options.mode === 'onChange' || options.mode === 'all')) {
      void validate(path);
    }
  };

  const setValues = (value: TFieldValues, setOptions: AsterVueFormSetValueOptions = {}) => {
    setDerivedValues(clone(value));
    const paths = fieldPaths(plan.value);
    if (setOptions.shouldDirty !== false) updateDirtyFields(paths);
    if (setOptions.shouldTouch) touchedFields.value = new Set(paths);
    if (setOptions.shouldValidate ?? (options.mode === 'onChange' || options.mode === 'all')) {
      void validate();
    }
  };

  const getValue = <TValue extends JsonValue = JsonValue>(path: string) =>
    getAtPath(internalValues.value, path) as TValue | undefined;

  const setError = (error: FieldError) => {
    internalErrors.value = [
      ...internalErrors.value.filter(
        (candidate) => candidate.path !== error.path || candidate.code !== error.code,
      ),
      error,
    ];
  };

  const clearErrors = (path?: string | readonly string[]) => {
    if (path === undefined) {
      internalErrors.value = [];
      return;
    }
    const paths = typeof path === 'string' ? [path] : path;
    internalErrors.value = internalErrors.value.filter(
      (error) => !paths.some((scope) => pathMatchesScope(error.path, scope)),
    );
  };

  const handleBlur = (path: string) => {
    touchedFields.value = new Set([...touchedFields.value, path]);
    if (options.mode === 'onBlur' || options.mode === 'all') void validate(path);
  };

  const register = <TValue extends JsonValue = JsonValue>(
    path: string,
  ): AsterVueFieldBinding<TValue> => ({
    name: path,
    get modelValue() {
      return getValue<TValue>(path);
    },
    'onUpdate:modelValue': (value) => setValue(path, value),
    onBlur: () => handleBlur(path),
  });

  const reset = (value: TFieldValues = defaultValues.value) => {
    activeValidation?.abort();
    activeValidation = undefined;
    evaluator.clear();
    defaultValues.value = clone(value);
    internalValues.value = clone(value);
    internalErrors.value = [];
    dirtyFields.value = new Set();
    touchedFields.value = new Set();
    validatingFields.value = new Set();
    isValidating.value = false;
    isSubmitting.value = false;
    isSubmitted.value = false;
    isSubmitSuccessful.value = false;
    submitCount.value = 0;
  };

  const handleSubmit = (
    onValid: (value: TFieldValues) => void | Promise<void>,
    onInvalid?: (errors: readonly FieldError[]) => void | Promise<void>,
  ) => {
    return async (event?: { preventDefault?: () => void }) => {
      event?.preventDefault?.();
      isSubmitting.value = true;
      isSubmitted.value = true;
      isSubmitSuccessful.value = false;
      submitCount.value += 1;
      try {
        const valid = await validate();
        if (valid) {
          await onValid(clone(internalValues.value));
          isSubmitSuccessful.value = true;
        } else {
          await onInvalid?.(internalErrors.value);
        }
      } finally {
        isSubmitting.value = false;
      }
    };
  };

  const formState = computed<AsterVueFormState>(() => ({
    isDirty: !jsonValuesEqual(internalValues.value, defaultValues.value),
    isValid:
      internalErrors.value.length === 0 &&
      validateFormValue(plan.value, internalValues.value, {
        locale: locale(),
        localeCatalog: localeCatalog(),
      }).length === 0,
    isValidating: isValidating.value,
    isSubmitting: isSubmitting.value,
    isSubmitted: isSubmitted.value,
    isSubmitSuccessful: isSubmitSuccessful.value,
    submitCount: submitCount.value,
    dirtyFields: dirtyFields.value,
    touchedFields: touchedFields.value,
    validatingFields: validatingFields.value,
    errors: internalErrors.value,
  }));

  const rendererProps = computed(() => ({
    plan: plan.value,
    value: internalValues.value as JsonObject,
    onChange: (value: JsonObject) =>
      setValues(value as TFieldValues, { shouldDirty: true, shouldTouch: true }),
    errors: internalErrors.value,
    hostAdapter: hostAdapter(),
    locale: locale(),
    localeCatalog: localeCatalog(),
  }));

  return {
    plan,
    values: readonly(internalValues) as Readonly<ShallowRef<TFieldValues>>,
    errors: readonly(internalErrors) as Readonly<ShallowRef<readonly FieldError[]>>,
    formState,
    rendererProps,
    getValue,
    setValue,
    setValues,
    setError,
    clearErrors,
    register,
    validate,
    reset,
    handleSubmit,
  };
}

export function provideAsterForm<TFieldValues extends JsonObject>(
  form: AsterVueFormContext<TFieldValues>,
): AsterVueFormContext<TFieldValues> {
  provide(formContextKey, form as unknown as AsterVueFormContext<JsonObject>);
  return form;
}

export function useAsterFormContext<
  TFieldValues extends JsonObject = JsonObject,
>(): AsterVueFormContext<TFieldValues> {
  const form = inject(formContextKey);
  if (!form) throw new Error('useAsterFormContext must be used below provideAsterForm.');
  return form as unknown as AsterVueFormContext<TFieldValues>;
}

export function useAsterField<
  TFieldValues extends JsonObject = JsonObject,
  TValue extends JsonValue = JsonValue,
>(options: UseAsterVueFieldOptions<TFieldValues, TValue>): UseAsterVueFieldReturn<TValue> {
  const form = options.form ?? useAsterFormContext<TFieldValues>();
  const errors = computed(() =>
    form.errors.value.filter((error) => pathMatchesScope(error.path, options.name)),
  );
  const setValue = (value: TValue) =>
    form.setValue(options.name, options.transform?.(value) ?? value, {
      shouldValidate: options.validateOnValueUpdate,
    });
  const value: WritableComputedRef<TValue | undefined> = computed({
    get: () => form.getValue<TValue>(options.name),
    set: (next: TValue | undefined) => {
      if (next !== undefined) setValue(next);
    },
  });
  const handleBlur = () => form.register(options.name).onBlur();

  return {
    name: options.name,
    value,
    errors,
    errorMessage: computed(() => errors.value[0]?.message),
    meta: computed(() => ({
      dirty: form.formState.value.dirtyFields.has(options.name),
      touched: form.formState.value.touchedFields.has(options.name),
      valid: errors.value.length === 0,
      validating: form.formState.value.validatingFields.has(options.name),
    })),
    setValue,
    handleBlur,
    validate: () => form.validate(options.name),
  };
}

function defaultFieldArrayKey(): string {
  fieldArrayIdentity += 1;
  return globalThis.crypto?.randomUUID?.() ?? `aster-field-${fieldArrayIdentity}`;
}

export function useAsterFieldArray<
  TFieldValues extends JsonObject = JsonObject,
  TValue extends JsonValue = JsonValue,
>(options: UseAsterVueFieldArrayOptions<TFieldValues>): UseAsterVueFieldArrayReturn<TValue> {
  const form = options.form ?? useAsterFormContext<TFieldValues>();
  const createKey = options.keyFactory ?? defaultFieldArrayKey;
  const keys = shallowRef<string[]>([]);
  const current = () => {
    const value = form.getValue<JsonValue[]>(options.name);
    return Array.isArray(value) ? (value as TValue[]) : [];
  };
  const reconcileKeys = (length: number) => {
    keys.value = [
      ...keys.value.slice(0, length),
      ...Array.from({ length: Math.max(0, length - keys.value.length) }, createKey),
    ];
  };
  watch(
    () => current().length,
    (length) => reconcileKeys(length),
    { immediate: true },
  );
  const commit = (value: readonly TValue[]) =>
    form.setValue(options.name, clone(value) as unknown as JsonValue, { shouldDirty: true });
  const boundedIndex = (index: number, allowEnd = false) =>
    Math.max(0, Math.min(index, current().length - (allowEnd ? 0 : 1)));

  return {
    fields: computed(() =>
      current().map((value, index) => ({ key: keys.value[index] as string, index, value })),
    ),
    append(value) {
      keys.value = [...keys.value, createKey()];
      commit([...current(), value]);
    },
    prepend(value) {
      keys.value = [createKey(), ...keys.value];
      commit([value, ...current()]);
    },
    insert(index, value) {
      const target = boundedIndex(index, true);
      const next = [...current()];
      next.splice(target, 0, value);
      const nextKeys = [...keys.value];
      nextKeys.splice(target, 0, createKey());
      keys.value = nextKeys;
      commit(next);
    },
    remove(index) {
      if (index === undefined) {
        keys.value = [];
        commit([]);
        return;
      }
      const indexes = new Set(Array.isArray(index) ? index : [index]);
      keys.value = keys.value.filter((_, itemIndex) => !indexes.has(itemIndex));
      commit(current().filter((_, itemIndex) => !indexes.has(itemIndex)));
    },
    swap(left, right) {
      const first = boundedIndex(left);
      const second = boundedIndex(right);
      if (first === second) return;
      const next = [...current()];
      [next[first], next[second]] = [next[second] as TValue, next[first] as TValue];
      const nextKeys = [...keys.value];
      [nextKeys[first], nextKeys[second]] = [nextKeys[second] as string, nextKeys[first] as string];
      keys.value = nextKeys;
      commit(next);
    },
    move(from, to) {
      const source = boundedIndex(from);
      const target = boundedIndex(to);
      if (source === target) return;
      const next = [...current()];
      const [item] = next.splice(source, 1);
      next.splice(target, 0, item as TValue);
      const nextKeys = [...keys.value];
      const [key] = nextKeys.splice(source, 1);
      nextKeys.splice(target, 0, key as string);
      keys.value = nextKeys;
      commit(next);
    },
    update(index, value) {
      const target = boundedIndex(index);
      const next = [...current()];
      next[target] = value;
      commit(next);
    },
    replace(value) {
      keys.value = value.map(() => createKey());
      commit(value);
    },
  };
}
