# Test Mocks

Unified test mocks for the project, organized by process type and globally configured in test setup files.

## Overview

### Available Mocks

| Process | Mock | Description |
|---------|------|-------------|
| Renderer | `CacheService` | Three-tier cache (memory/shared/persist) |
| Renderer | `DataApiService` | HTTP client for Data API |
| Renderer | `PreferenceService` | User preferences |
| Renderer | `useDataApi` | Data API hooks (useQuery, useMutation, etc.) |
| Renderer | `usePreference` | Preference hooks |
| Renderer | `useCache` | Cache hooks |
| Renderer | `workspaceDocumentsFixture` | Native-document wire responses backed by a test's existing disk transport; not a native authority proof |

### File Structure

```
tests/__mocks__/
├── renderer/
│   ├── CacheService.ts
│   ├── DataApiService.ts
│   ├── PreferenceService.ts
│   ├── useDataApi.ts
│   ├── usePreference.ts
│   └── useCache.ts
├── RendererLoggerService.ts
└── MainLoggerService.ts
```

### Test Setup

Mocks are globally configured in setup files:
- **Renderer**: `tests/renderer.setup.ts`

### Import Path Alias

Use `@test-mocks/*` to import mock utilities:

```typescript
import { MockCacheUtils } from '@test-mocks/renderer/CacheService'
```

---

## Renderer Mocks

### CacheService

Three-tier cache system with type-safe methods (and casual/dynamic key methods on the Memory tier only).

#### Methods

| Category | Method | Signature |
|----------|--------|-----------|
| Memory (typed) | `get` | `<K>(key: K) => InferUseCacheValue<K>` |
| Memory (typed) | `set` | `<K>(key: K, value, ttl?) => void` |
| Memory (typed) | `has` | `<K>(key: K) => boolean` |
| Memory (typed) | `delete` | `<K>(key: K) => boolean` |
| Memory (typed) | `hasTTL` | `<K>(key: K) => boolean` |
| Memory (casual) | `getCasual` | `<T>(key: string) => T \| undefined` |
| Memory (casual) | `setCasual` | `<T>(key, value, ttl?) => void` |
| Memory (casual) | `hasCasual` | `(key: string) => boolean` |
| Memory (casual) | `deleteCasual` | `(key: string) => boolean` |
| Memory (casual) | `hasTTLCasual` | `(key: string) => boolean` |
| Shared (typed) | `getShared` | `<K>(key: K) => InferSharedCacheValue<K>` |
| Shared (typed) | `getSharedSnapshot` | `<K>(key: K) => InferSharedCacheValue<K> \| undefined` (pure physical read, no TTL/eviction/default; **internal** — production callers live in `useCache.ts` only, tests may probe it freely) |
| Shared (typed) | `setShared` | `<K>(key: K, value, ttl?) => void` |
| Shared (typed) | `hasShared` | `<K>(key: K) => boolean` |
| Shared (typed) | `deleteShared` | `<K>(key: K) => boolean` |
| Shared (typed) | `hasSharedTTL` | `<K>(key: K) => boolean` |
| Persist | `getPersist` | `<K>(key: K) => RendererPersistCacheSchema[K]` |
| Persist | `setPersist` | `<K>(key: K, value) => void` |
| Persist | `hasPersist` | `(key) => boolean` |
| Hook mgmt | `registerHook` | `(key: string) => void` |
| Hook mgmt | `unregisterHook` | `(key: string) => void` |
| Ready state | `isSharedCacheReady` | `() => boolean` |
| Ready state | `onSharedCacheReady` | `(callback) => () => void` |
| Lifecycle | `subscribe` | `(key, callback) => () => void` |
| Lifecycle | `cleanup` | `() => void` |

#### Usage

```typescript
import { cacheService } from '@data/CacheService'
import { MockCacheUtils } from '@test-mocks/renderer/CacheService'

describe('Cache', () => {
  beforeEach(() => MockCacheUtils.resetMocks())

  it('basic usage', () => {
    cacheService.setCasual('key', { data: 'value' }, 5000)
    expect(cacheService.getCasual('key')).toEqual({ data: 'value' })
  })

  it('with test utilities', () => {
    MockCacheUtils.setInitialState({
      memory: [['key', 'value']],
      shared: [['shared.key', 'shared']],
      persist: [['persist.key', 'persist']]
    })
  })
})
```

---

### DataApiService

HTTP client with data change notifications and retry configuration.

#### Methods

| Method | Signature |
|--------|-----------|
| `get` | `(path, options?) => Promise<any>` |
| `post` | `(path, options) => Promise<any>` |
| `put` | `(path, options) => Promise<any>` |
| `patch` | `(path, options) => Promise<any>` |
| `delete` | `(path, options?) => Promise<any>` |
| `onDataChanged` | `(endpoints, listener) => () => void` — functional fan-out, production semantics |
| `configureRetry` | `(options) => void` |
| `getRetryConfig` | `() => RetryOptions` |

#### Usage

```typescript
import { dataApiService } from '@data/DataApiService'
import { MockDataApiUtils } from '@test-mocks/renderer/DataApiService'

describe('API', () => {
  beforeEach(() => MockDataApiUtils.resetMocks())

  it('basic request', async () => {
    const response = await dataApiService.get('/topics')
    expect(response.topics).toBeDefined()
  })

  it('custom response', async () => {
    MockDataApiUtils.setCustomResponse('/topics', 'GET', { custom: true })
    const response = await dataApiService.get('/topics')
    expect(response.custom).toBe(true)
  })

  it('error simulation', async () => {
    MockDataApiUtils.setErrorResponse('/topics', 'GET', new Error('Failed'))
    await expect(dataApiService.get('/topics')).rejects.toThrow('Failed')
  })

  it('data change convergence', () => {
    const listener = vi.fn()
    dataApiService.onDataChanged('/topics', listener)
    MockDataApiUtils.emitDataChange([{ endpoint: '/topics', kind: 'membership' }])
    expect(listener).toHaveBeenCalledWith([{ endpoint: '/topics', kind: 'membership' }])
  })
})
```

For hook consumers, the `useDataApi` mock exposes the same pair: `useDataChange` registers listeners and `MockUseDataApiUtils.emitDataChange(effects)` delivers a notification with production batch semantics. Both delegate to the `DataApiService` mock's fan-out (the production layering), so emitting through either utility reaches listeners registered through either module.

---

### useDataApi Hooks

React hooks for data operations.

#### Hooks

| Hook | Signature | Returns |
|------|-----------|---------|
| `useQuery` | `(path, options?)` | `{ data, loading, error, refetch, mutate }` |
| `useMutation` | `(method, path, options?)` | `{ trigger, isLoading, error }` |
| `useInfiniteQuery` | `(path, options?)` | `{ pages, isLoading, error, loadNext, mutate }` |
| `useWriteInfiniteCache` | `(path, options?)` | cache-only updater scoped to one infinite-query key |
| `usePaginatedQuery` | `(path, options?)` | `{ items, total, page, loading, error, hasMore, hasPrev, prevPage, nextPage, refresh, reset }` |
| `useInvalidateCache` | `()` | `(keys?) => Promise<any>` |
| `useReadCache` | `()` | `(path, query?) => TResponse \| undefined` |
| `useWriteCache` | `()` | `async (path, value, query?) => void` |

#### Usage

```typescript
import { useQuery, useMutation, useReadCache, useWriteCache } from '@data/hooks/useDataApi'
import { MockUseDataApiUtils } from '@test-mocks/renderer/useDataApi'

describe('Hooks', () => {
  beforeEach(() => MockUseDataApiUtils.resetMocks())

  it('useQuery', () => {
    const { data, loading } = useQuery('/topics')
    expect(loading).toBe(false)
    expect(data).toBeDefined()
  })

  it('useMutation', async () => {
    const { trigger } = useMutation('POST', '/topics')
    const result = await trigger({ body: { name: 'New' } })
    expect(result.created).toBe(true)
  })

  it('custom data', () => {
    MockUseDataApiUtils.mockQueryData('/topics', { custom: true })
    const { data } = useQuery('/topics')
    expect(data.custom).toBe(true)
  })

  it('useReadCache reads seeded values', () => {
    // Pre-populate the mock cache (key shape mirrors production:
    // omit `query` for [path], pass a non-empty `query` for [path, query]).
    MockUseDataApiUtils.seedCache('/topics', { topics: [{ id: 't1' }], total: 1 })

    const read = useReadCache()
    expect(read('/topics')).toEqual({ topics: [{ id: 't1' }], total: 1 })
  })

  it('useWriteCache persists to mock store (assertable via getCachedValue)', async () => {
    const write = useWriteCache()
    await write('/topics', { topics: [], total: 0 })

    expect(MockUseDataApiUtils.getCachedValue('/topics')).toEqual({ topics: [], total: 0 })
  })
})
```

Use `MockUseDataApiUtils.seedInfiniteQuery(path, pages, options)` before rendering when a test needs functional cache updates, route-parameter cache isolation, and mounted-hook rerenders. `getInfiniteQueryPages` and `setInfiniteQueryPages` inspect or replace the same cache entry.

> **Note:** `useReadCache`/`useWriteCache` share one in-memory `Map` under the hood. `resetMocks()` clears both call history and the cache store; use `clearCache()` if you want to drop cache entries without resetting hook mocks.

---

### useCache Hooks

React hooks for cache operations.

| Hook | Signature | Returns |
|------|-----------|---------|
| `useCache` | `(key, initValue?)` | `[value, setValue]` |
| `useSharedCache` | `(key, initValue?)` | `[value, setValue]` |
| `useSharedCacheValue` | `(key)` | `value \| undefined` (read-only, never materializes a default) |
| `useSharedCacheSelector` | `(keys, selector, isEqual?)` | `selector(values)` over the current mock shared values (read-only; like the other hook mocks it is per-render, not reactive) |
| `usePersistCache` | `(key)` | `[value, setValue]` |

`setValue` accepts a concrete value or a functional updater `(prev) => next` (mirrors production). The mock resolves the updater against the latest mocked value with the same default fallback, so functional-update call sites run unchanged under the mock.

```typescript
import { useCache } from '@data/hooks/useCache'

const [value, setValue] = useCache('key', 'default')
setValue('new value')
setValue((prev) => prev + '!') // functional updater
```

---

### usePreference Hooks

React hooks for preferences.

| Hook | Signature | Returns |
|------|-----------|---------|
| `usePreference` | `(key)` | `[value, setValue]` |
| `useMultiplePreferences` | `(keyMap)` | `[values, setValues]` |

```typescript
import { usePreference } from '@data/hooks/usePreference'

const [theme, setTheme] = usePreference('ui.theme')
await setTheme('dark')
```

---

## Best Practices

1. Infrastructure services come pre-mocked; override via `mockApplicationFactory({ Name: {...} })`, not ad-hoc `application.get` mocks.
2. Feature-specific services are stubbed locally in the test file.
3. Each infrastructure mock exposes helpers with `resetMocks()`. Call `resetMocks()` in `beforeEach`.

## Troubleshooting

| Issue | Solution |
|-------|----------|
| Mock not applied | Check test runs in correct process (renderer/main in vitest.config.ts) |
| Type errors | Ensure mock matches actual interface, use type assertions if needed |
| State pollution | Call `resetMocks()` in `beforeEach` |
| Import issues | Use path aliases (`@data/CacheService`) not relative paths |

`renderer/workspaceFileImport.ts` provides a call-recording HTTP boundary for
Knowledge folder traversal and collision-safe imports. It retains exact bytes and
can delay/refuse acknowledgments; it does not prove native filesystem authority.

`renderer/documentResultHandoff.ts` provides the bounded native PDF-result transfer fixture. Pair it with `workspaceFileImport` to verify that the main Documents picker persists the exact result bytes before acknowledging success.
