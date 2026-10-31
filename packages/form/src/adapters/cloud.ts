import type {
  ActionRequest,
  AsyncValidationRequest,
  AsyncValidationResponse,
  DataSourceRequest,
  DataSourceResponse,
  FormHostAdapter,
  JsonValue,
} from '../core/types';

export interface AsterCloudFormContext {
  organizationId: string;
  projectId: string;
  environmentId: string;
  locale?: string;
}

export interface AsterCloudFormBindings {
  context: AsterCloudFormContext;
  resolveDataSource?: (
    context: AsterCloudFormContext,
    request: DataSourceRequest,
    signal: AbortSignal,
  ) => Promise<DataSourceResponse>;
  invokeAction?: (
    context: AsterCloudFormContext,
    request: ActionRequest,
    signal: AbortSignal,
    // biome-ignore lint/suspicious/noConfusingVoidType: Host actions may intentionally return no payload.
  ) => Promise<JsonValue | void>;
  validateValue?: (
    context: AsterCloudFormContext,
    request: AsyncValidationRequest,
    signal: AbortSignal,
  ) => Promise<AsyncValidationResponse>;
}

export function createAsterCloudFormAdapter(bindings: AsterCloudFormBindings): FormHostAdapter {
  return {
    resolveDataSource: bindings.resolveDataSource
      ? (request, signal) =>
          bindings.resolveDataSource?.(bindings.context, request, signal) ?? Promise.resolve([])
      : undefined,
    validateValue: bindings.validateValue
      ? (request, signal) =>
          bindings.validateValue?.(bindings.context, request, signal) ??
          Promise.resolve({ issues: [] })
      : undefined,
    invokeAction: bindings.invokeAction
      ? (request, signal) =>
          bindings.invokeAction?.(bindings.context, request, signal) ?? Promise.resolve()
      : undefined,
  };
}
