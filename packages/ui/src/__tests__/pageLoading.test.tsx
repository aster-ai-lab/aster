// @vitest-environment happy-dom
import { afterEach, beforeEach, expect, test, vi } from 'vitest';
import { act, cleanup, render, screen } from '@testing-library/react';
import { useState } from 'react';
import { MemoryRouter, useLocation, useNavigate } from 'react-router-dom';
import { PageLoading } from '../components/console/PageLoading';
import {
  clearPageLoadingSession,
  useRoutePageLoading,
  useShowPageLoading,
} from '../hooks/useRoutePageLoading';

beforeEach(() => {
  clearPageLoadingSession();
});

afterEach(() => {
  cleanup();
  document.body.innerHTML = '';
  document.documentElement.removeAttribute('data-tenant');
  clearPageLoadingSession();
});

function spinnerMarkup() {
  return document.querySelector('.page-loader-spinner')?.outerHTML ?? '';
}

test('page loader paints once for a pathname', async () => {
  const { unmount } = render(
    <MemoryRouter initialEntries={['/admin/fleet']}>
      <PageLoading variant="contain" minMs={60_000} />
      <PageLoading variant="contain" minMs={60_000} />
    </MemoryRouter>,
  );
  expect(document.querySelectorAll('.page-loader-spinner').length).toBe(1);
  await flush(1);
  unmount();

  render(
    <MemoryRouter initialEntries={['/admin/fleet']}>
      <PageLoading variant="contain" minMs={60_000} />
    </MemoryRouter>,
  );
  expect(document.querySelector('.page-loader-spinner')).toBeNull();

  cleanup();
  document.body.innerHTML = '';
  clearPageLoadingSession();
  render(
    <MemoryRouter initialEntries={['/admin/members']}>
      <PageLoading variant="contain" minMs={60_000} />
    </MemoryRouter>,
  );
  expect(document.querySelectorAll('.page-loader-spinner').length).toBe(1);
});

test('replay shows the loader again on a pathname that already played', async () => {
  const { unmount } = render(
    <MemoryRouter initialEntries={['/knowledge']}>
      <PageLoading variant="contain" minMs={60_000} />
    </MemoryRouter>,
  );
  await flush(1);
  unmount();

  render(
    <MemoryRouter initialEntries={['/knowledge']}>
      <PageLoading variant="contain" minMs={60_000} />
    </MemoryRouter>,
  );
  expect(document.querySelector('.page-loader-spinner')).toBeNull();

  cleanup();
  document.body.innerHTML = '';
  render(
    <MemoryRouter initialEntries={['/knowledge']}>
      <PageLoading variant="contain" replay minMs={60_000} />
    </MemoryRouter>,
  );
  expect(document.querySelectorAll('.page-loader-spinner').length).toBe(1);
});

test('page loader uses one spinner for every tenant', () => {
  const { unmount } = render(<PageLoading variant="contain" minMs={60_000} />);
  const circle = document.querySelector('.page-loader-spinner circle');
  expect(document.querySelector('.page-loader-spinner')?.getAttribute('viewBox')).toBe(
    '0 0 48 48',
  );
  expect(circle?.getAttribute('r')).toBe('18');
  expect(circle?.getAttribute('stroke-dasharray')).toBe('28 85');
  expect(document.querySelector('[data-draw]')).toBeNull();
  expect(document.querySelector('.page-loader-mark')).toBeNull();
  const plain = spinnerMarkup();
  unmount();
  cleanup();
  document.body.innerHTML = '';

  document.documentElement.dataset.tenant = 'a3s';
  const branded = render(<PageLoading variant="contain" minMs={60_000} />);
  expect(spinnerMarkup()).toBe(plain);
  branded.unmount();
  cleanup();
  document.body.innerHTML = '';

  document.documentElement.dataset.tenant = 'toyou';
  render(<PageLoading variant="contain" minMs={60_000} />);
  expect(spinnerMarkup()).toBe(plain);
});

function flush(ms: number) {
  return act(async () => {
    await new Promise((resolve) => window.setTimeout(resolve, ms));
  });
}

function ShowProbe({ pending }: { pending: boolean }) {
  const show = useShowPageLoading(pending, 'probe');
  return <div data-testid="show">{show ? 'show' : 'hide'}</div>;
}

function RouteProbe() {
  const location = useLocation();
  const navigate = useNavigate();
  const key = useRoutePageLoading(location.pathname);
  return (
    <div>
      <div data-testid="key">{key ?? ''}</div>
      <button type="button" onClick={() => navigate('/admin/fleet')}>
        fleet
      </button>
      <button type="button" onClick={() => navigate('/admin/members')}>
        members
      </button>
      <button type="button" onClick={() => navigate('/')}>
        home
      </button>
    </div>
  );
}

test('page loader plays once per pathname, including when pending flips back on', async () => {
  const { rerender, unmount } = render(
    <MemoryRouter>
      <ShowProbe pending />
    </MemoryRouter>,
  );
  expect(screen.getByTestId('show').textContent).toBe('show');

  rerender(
    <MemoryRouter>
      <ShowProbe pending={false} />
    </MemoryRouter>,
  );
  expect(screen.getByTestId('show').textContent).toBe('hide');

  rerender(
    <MemoryRouter>
      <ShowProbe pending />
    </MemoryRouter>,
  );
  expect(screen.getByTestId('show').textContent).toBe('hide');

  await flush(1);
  unmount();
  render(
    <MemoryRouter>
      <ShowProbe pending />
    </MemoryRouter>,
  );
  expect(screen.getByTestId('show').textContent).toBe('hide');
});

test('different slots on one path each play once', async () => {
  function Slots() {
    const [pending, setPending] = useState(true);
    const home = useShowPageLoading(pending, 'home');
    const plans = useShowPageLoading(pending, 'plans');
    return (
      <div>
        <div data-testid="home">{home ? 'show' : 'hide'}</div>
        <div data-testid="plans">{plans ? 'show' : 'hide'}</div>
        <button type="button" onClick={() => setPending(false)}>
          done
        </button>
        <button type="button" onClick={() => setPending(true)}>
          again
        </button>
      </div>
    );
  }

  render(
    <MemoryRouter>
      <Slots />
    </MemoryRouter>,
  );
  expect(screen.getByTestId('home').textContent).toBe('show');
  expect(screen.getByTestId('plans').textContent).toBe('show');
  await act(async () => {
    screen.getByRole('button', { name: 'done' }).click();
  });
  expect(screen.getByTestId('home').textContent).toBe('hide');
  await act(async () => {
    screen.getByRole('button', { name: 'again' }).click();
  });
  expect(screen.getByTestId('home').textContent).toBe('hide');
  expect(screen.getByTestId('plans').textContent).toBe('hide');
});

test('immediate shell loader does not wait for the boot hold', async () => {
  function Probe() {
    const location = useLocation();
    const navigate = useNavigate();
    const key = useRoutePageLoading(location.pathname, { immediate: true });
    return (
      <div>
        <div data-testid="now">{key ?? ''}</div>
        <button type="button" onClick={() => navigate('/org')}>
          org
        </button>
        <button type="button" onClick={() => navigate('/')}>
          back
        </button>
      </div>
    );
  }

  render(
    <MemoryRouter initialEntries={['/']}>
      <Probe />
    </MemoryRouter>,
  );
  await flush(1);
  expect(screen.getByTestId('now').textContent).toBe('');
  await act(async () => {
    screen.getByRole('button', { name: 'org' }).click();
  });
  expect(screen.getByTestId('now').textContent).toBe('/org');
  await flush(1);
  await act(async () => {
    screen.getByRole('button', { name: 'back' }).click();
  });
  await act(async () => {
    screen.getByRole('button', { name: 'org' }).click();
  });
  expect(screen.getByTestId('now').textContent).toBe('');
});

test('shell loader skips the boot path and replays a path only the first time', async () => {
  render(
    <MemoryRouter initialEntries={['/']}>
      <RouteProbe />
    </MemoryRouter>,
  );
  expect(screen.getByTestId('key').textContent).toBe('');

  await flush(1500);
  expect(screen.getByTestId('key').textContent).toBe('');

  await act(async () => {
    screen.getByRole('button', { name: 'fleet' }).click();
  });
  expect(screen.getByTestId('key').textContent).toBe('/admin/fleet');

  await flush(1);
  await act(async () => {
    screen.getByRole('button', { name: 'members' }).click();
  });
  expect(screen.getByTestId('key').textContent).toBe('/admin/members');

  await flush(1);
  await act(async () => {
    screen.getByRole('button', { name: 'fleet' }).click();
  });
  expect(screen.getByTestId('key').textContent).toBe('');

  await act(async () => {
    screen.getByRole('button', { name: 'home' }).click();
  });
  expect(screen.getByTestId('key').textContent).toBe('');
});
