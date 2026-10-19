// @vitest-environment jsdom
import { cleanup, fireEvent, render, screen } from '@testing-library/react';
import userEvent from '@testing-library/user-event';
import { afterEach, describe, expect, it, vi } from 'vitest';

import { CursorPager, Pagination } from '../Pagination';

afterEach(cleanup);

describe('collection pagination', () => {
  it('keeps the exact range without redundant navigation for a single or empty page', () => {
    const { rerender } = render(
      <Pagination page={1} pageSize={50} total={4} onPageChange={vi.fn()} />,
    );
    expect(screen.getByRole('status').textContent).toBe('1–4 of 4');
    expect(screen.queryByRole('group', { name: 'Pages' })).toBeNull();
    rerender(
      <Pagination page={1} pageSize={50} total={0} onPageChange={vi.fn()} />,
    );
    expect(screen.getByRole('status').textContent).toBe('0 results');
    expect(screen.queryByRole('button')).toBeNull();
  });

  it('bounds long navigation, reaches the last page and ignores a click on the current page', async () => {
    const user = userEvent.setup();
    const change = vi.fn();
    const { rerender } = render(
      <Pagination page={5} pageSize={10} total={200} onPageChange={change} />,
    );
    expect(screen.getAllByRole('button').length).toBeLessThanOrEqual(9);
    expect(screen.getByRole('button', { name: 'Page 1' })).toBeTruthy();
    await user.click(screen.getByRole('button', { name: 'Page 5' }));
    expect(change).not.toHaveBeenCalled();
    await user.click(screen.getByRole('button', { name: 'Page 20' }));
    expect(change).toHaveBeenCalledWith(20);
    rerender(
      <Pagination page={20} pageSize={10} total={200} onPageChange={change} />,
    );
    expect(
      (screen.getByRole('button', { name: 'Next page' }))
        .disabled,
    ).toBe(true);
    expect(screen.getByRole('status').textContent).toBe('191–200 of 200');
  });

  it('localizes cursor navigation and refuses repeat navigation while a page is pending', () => {
    const onNext = vi.fn();
    const onPrev = vi.fn();
    const props = {
      hasNext: true,
      hasPrev: false,
      onNext,
      onPrev,
      label: '已加载 20 条',
      labels: { next: '下一页', previous: '上一页', navigation: '日志分页' },
    };
    const { rerender } = render(<CursorPager {...props} />);
    expect(screen.getByRole('navigation', { name: '日志分页' })).toBeTruthy();
    expect(screen.getByRole('status').textContent).toBe('已加载 20 条');
    fireEvent.click(screen.getByRole('button', { name: '上一页' }));
    expect(onPrev).not.toHaveBeenCalled();
    fireEvent.click(screen.getByRole('button', { name: '下一页' }));
    expect(onNext).toHaveBeenCalledOnce();
    rerender(<CursorPager {...props} busy />);
    fireEvent.click(screen.getByRole('button', { name: '下一页' }));
    expect(onNext).toHaveBeenCalledOnce();
    expect(screen.getByRole('navigation').getAttribute('aria-busy')).toBe(
      'true',
    );
  });

  it('localizes labels, announces the range and marks the current page', () => {
    const onPageChange = vi.fn();
    render(
      <Pagination
        page={2}
        pageSize={20}
        total={101}
        onPageChange={onPageChange}
        labels={{
          navigation: '分页',
          pages: '页码',
          previous: '上一页',
          next: '下一页',
          page: (page) => `第 ${page} 页`,
          range: (start, end, total) => `${start}–${end}，共 ${total} 条`,
        }}
      />,
    );
    expect(screen.getByRole('navigation', { name: '分页' })).toBeTruthy();
    expect(screen.getByRole('status').textContent).toBe('21–40，共 101 条');
    expect(
      screen
        .getByRole('button', { name: '第 2 页' })
        .getAttribute('aria-current'),
    ).toBe('page');
    fireEvent.click(screen.getByRole('button', { name: '第 6 页' }));
    expect(onPageChange).toHaveBeenCalledWith(6);
  });

  it('clamps removed pages and disables every page action while loading', () => {
    const onPageChange = vi.fn();
    const { rerender } = render(
      <Pagination
        page={8}
        pageSize={20}
        total={21}
        onPageChange={onPageChange}
      />,
    );
    expect(screen.getByRole('status').textContent).toBe('21–21 of 21');
    fireEvent.click(screen.getByRole('button', { name: 'Previous page' }));
    expect(onPageChange).toHaveBeenCalledWith(1);
    onPageChange.mockClear();
    rerender(
      <Pagination
        page={2}
        pageSize={20}
        total={100}
        busy
        onPageChange={onPageChange}
      />,
    );
    for (const button of screen.getAllByRole('button')) {
      expect((button as HTMLButtonElement).disabled).toBe(true);
      fireEvent.click(button);
    }
    expect(onPageChange).not.toHaveBeenCalled();
  });
});
