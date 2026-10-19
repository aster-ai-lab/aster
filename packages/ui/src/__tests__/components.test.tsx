// @vitest-environment happy-dom
import { beforeEach, describe, expect, test, vi } from 'vitest';
import {
  cleanup,
  fireEvent,
  render,
  screen,
  waitFor,
  within,
} from '@testing-library/react';
import { useState } from 'react';

beforeEach(() => {
  cleanup();
  document.body.innerHTML = '';
});

describe('Wave A — P0', () => {
  test('EmptyState no-match exposes clear filters', async () => {
    const { EmptyState } = await import('../components/console/EmptyState');
    const onClear = vi.fn(() => undefined);
    render(
      <EmptyState
        variant="no-match"
        title="No matches"
        description="Try clearing filters"
        clearFiltersLabel="Clear filters"
        onClearFilters={onClear}
      />,
    );
    fireEvent.click(screen.getByRole('button', { name: 'Clear filters' }));
    expect(onClear).toHaveBeenCalled();
  });

  test('FormField shows error and marks control invalid', async () => {
    const { FormField } = await import('../components/console/FormField');
    const { Input } = await import('../components/primitives/input');
    render(
      <FormField label="Email" error="Required" required>
        <Input />
      </FormField>,
    );
    expect(screen.getByText('Required')).toBeTruthy();
    expect(screen.getByRole('textbox').getAttribute('aria-invalid')).toBe(
      'true',
    );
  });

  test('ConfirmDialog requires typed resource name', async () => {
    const { ConfirmDialog } = await import('../components/console/ConfirmDialog');
    const onConfirm = vi.fn(() => undefined);
    function Harness() {
      const [open, setOpen] = useState(true);
      return (
        <ConfirmDialog
          open={open}
          onOpenChange={setOpen}
          title="Delete role"
          confirmText="admin"
          onConfirm={onConfirm}
        />
      );
    }
    render(<Harness />);
    const dialog = screen.getByRole('dialog');
    const confirmBtn = within(dialog).getByRole('button', { name: 'Confirm' });
    expect((confirmBtn as HTMLButtonElement).disabled).toBe(true);
    fireEvent.change(within(dialog).getByRole('textbox'), {
      target: { value: 'admin' },
    });
    expect((confirmBtn as HTMLButtonElement).disabled).toBe(false);
    fireEvent.click(confirmBtn);
    expect(onConfirm).toHaveBeenCalled();
  });



  test('DataTable renders rows and row actions', async () => {
    const { DataTable } = await import('../components/console/DataTable');
    const onAction = vi.fn(() => undefined);
    render(
      <DataTable
        columns={[
          { id: 'name', header: 'Name', cell: (r: { name: string }) => r.name },
        ]}
        rows={[{ id: '1', name: 'Ada' }]}
        rowKey={(r) => r.id}
        rowActions={(r) => (
          <button type="button" onClick={() => onAction(r.id)}>
            Edit
          </button>
        )}
      />,
    );
    expect(screen.getByText('Ada')).toBeTruthy();
    fireEvent.click(screen.getByRole('button', { name: 'Edit' }));
    expect(onAction).toHaveBeenCalledWith('1');
  });

  test('DataTable sorts, filters, and paginates client-side', async () => {
    const { DataTable } = await import('../components/console/DataTable');
    type Row = { id: string; name: string; score: number };
    const rows: Row[] = [
      { id: '1', name: 'zeta', score: 3 },
      { id: '2', name: 'alpha', score: 1 },
      { id: '3', name: 'beta', score: 2 },
      { id: '4', name: 'gamma', score: 4 },
      { id: '5', name: 'delta', score: 5 },
    ];

    render(
      <DataTable
        columns={[
          {
            id: 'name',
            header: 'Name',
            sortable: true,
            filterable: true,
            getSortValue: (r) => r.name,
            getFilterValue: (r) => r.name,
            cell: (r) => r.name,
          },
          {
            id: 'score',
            header: 'Score',
            sortable: true,
            getSortValue: (r) => r.score,
            cell: (r) => String(r.score),
          },
        ]}
        rows={rows}
        rowKey={(r) => r.id}
        showGlobalFilter
        globalFilterPlaceholder="Filter rows"
        defaultPagination={{ page: 1, pageSize: 2 }}
        pageSizeOptions={[2, 5]}
      />,
    );

    fireEvent.click(screen.getByRole('button', { name: /Name/i }));
    const body = screen.getAllByRole('row').slice(2); // skip header + filter row
    expect(body[0]?.textContent).toContain('alpha');

    fireEvent.change(screen.getByPlaceholderText('Filter rows'), {
      target: { value: 'a' },
    });
    expect(screen.getByText('alpha')).toBeTruthy();
    expect(screen.queryByText('zeta')).toBeNull();

    fireEvent.click(screen.getByRole('button', { name: 'Next page' }));
    expect(screen.getByText(/of \d+/)).toBeTruthy();

    fireEvent.click(screen.getByLabelText('Rows per page'));
    const listbox = screen.getByRole('listbox');
    fireEvent.click(within(listbox).getByRole('button', { name: '5' }));
    expect(screen.getByText('alpha')).toBeTruthy();
    expect(screen.getByText('gamma')).toBeTruthy();
  });

  test('DataTable manual mode emits state without mutating rows', async () => {
    const { DataTable } = await import('../components/console/DataTable');
    const onSort = vi.fn(() => undefined);
    const onPage = vi.fn(() => undefined);
    render(
      <DataTable
        columns={[
          {
            id: 'name',
            header: 'Name',
            sortable: true,
            getSortValue: (r: { name: string }) => r.name,
            cell: (r) => r.name,
          },
        ]}
        rows={[
          { id: '1', name: 'b' },
          { id: '2', name: 'a' },
        ]}
        rowKey={(r) => r.id}
        manualSorting
        manualPagination
        totalRows={40}
        pagination={{ page: 2, pageSize: 10 }}
        onSortChange={onSort}
        onPaginationChange={onPage}
      />,
    );

    expect(screen.getByText('b')).toBeTruthy();
    fireEvent.click(screen.getByRole('button', { name: /Name/i }));
    expect(onSort).toHaveBeenCalled();
    fireEvent.click(screen.getByRole('button', { name: 'Next page' }));
    expect(onPage).toHaveBeenCalledWith({ page: 3, pageSize: 10 });
  });

  test('DataTable selected row uses design wash class', async () => {
    const { DataTable } = await import('../components/console/DataTable');
    const { container } = render(
      <DataTable
        columns={[
          { id: 'name', header: 'Name', cell: (r: { name: string }) => r.name },
        ]}
        rows={[
          { id: '1', name: 'Ada' },
          { id: '2', name: 'Grace' },
        ]}
        rowKey={(r) => r.id}
        selectedRowKey="2"
        onRowClick={() => undefined}
      />,
    );
    const selected = container.querySelector(
      '.ui-data-table__row.is-selected[data-row-key="2"]',
    );
    expect(selected).toBeTruthy();
    expect(container.querySelector('.ui-data-table')).toBeTruthy();
  });

  test('DataTable column menu hides columns and refreshes', async () => {
    const { DataTable } = await import('../components/console/DataTable');
    const { ListToolbar } = await import('../components/console/ListToolbar');
    const onRefresh = vi.fn(() => undefined);
    const { container } = render(
      <div>
        <ListToolbar
          search=""
          onSearchChange={() => undefined}
          searchPlaceholder="Find"
        />
        <DataTable
          columns={[
            { id: 'name', header: 'Name', cell: (r: { name: string }) => r.name },
            {
              id: 'score',
              header: 'Score',
              cell: (r: { score: number }) => String(r.score),
            },
          ]}
          rows={[{ id: '1', name: 'Ada', score: 3 }]}
          rowKey={(r) => r.id}
          onRefresh={onRefresh}
        />
      </div>,
    );

    const slot = container.querySelector('[data-a3s-list-tools]');
    expect(slot?.querySelector('[aria-label="Column filters"]')).toBeTruthy();
    fireEvent.click(screen.getByRole('button', { name: 'Column filters' }));
    const score = screen.getByRole('checkbox', { name: 'Score' });
    expect((score as HTMLInputElement).checked).toBe(true);
    fireEvent.click(score);
    expect(screen.queryByRole('columnheader', { name: 'Score' })).toBeNull();
    expect(screen.getByText('Ada')).toBeTruthy();

    const refresh = screen.getByRole('button', { name: 'Refresh' });
    fireEvent.click(refresh);
    expect(refresh.className).toContain('is-refreshing');
    await waitFor(() => expect(onRefresh).toHaveBeenCalled());
  });

  test('dataTableModel filter → sort → paginate', async () => {
    const {
      filterRows,
      sortRows,
      paginateRows,
      nextSortState,
    } = await import('../lib/resourceTableModel');
    type Row = { id: string; name: string };
    const columns = [
      {
        id: 'name',
        sortable: true as const,
        getSortValue: (r: Row) => r.name,
        getFilterValue: (r: Row) => r.name,
      },
    ];
    const rows: Row[] = [
      { id: '1', name: 'moon' },
      { id: '2', name: 'alpha' },
      { id: '3', name: 'beta' },
    ];
    const filtered = filterRows(rows, columns, {
      globalFilter: 'a',
      columnFilters: {},
    });
    expect(filtered.map((r) => r.name)).toEqual(['alpha', 'beta']);
    const sorted = sortRows(filtered, columns, {
      id: 'name',
      direction: 'desc',
    });
    expect(sorted.map((r) => r.name)).toEqual(['beta', 'alpha']);
    expect(paginateRows(sorted, { page: 1, pageSize: 1 }).map((r) => r.name)).toEqual([
      'beta',
    ]);
    expect(nextSortState(null, 'name')).toEqual({
      id: 'name',
      direction: 'asc',
    });
  });

  test('ListToolbar wires search', async () => {
    const { ListToolbar } = await import('../components/console/ListToolbar');
    const onSearch = vi.fn(() => undefined);
    render(
      <ListToolbar
        search=""
        onSearchChange={onSearch}
        searchPlaceholder="Find"
      />,
    );
    fireEvent.change(screen.getByPlaceholderText('Find'), {
      target: { value: 'roy' },
    });
    expect(onSearch).toHaveBeenCalledWith('roy');
  });

  test('SecretReveal locks after hide', async () => {
    const { SecretReveal } = await import('../components/console/SecretReveal');
    render(<SecretReveal value="sk-secret-value" />);
    fireEvent.click(screen.getByRole('button', { name: 'Show once' }));
    expect(screen.getByText('sk-secret-value')).toBeTruthy();
    fireEvent.click(screen.getByRole('button', { name: 'Hide permanently' }));
    expect(screen.queryByText('sk-secret-value')).toBeNull();
    expect(screen.queryByRole('button', { name: 'Show once' })).toBeNull();
  });

  test('ErrorBanner retry and ForbiddenPage permission id', async () => {
    const { ErrorBanner, ForbiddenPage } = await import(
      '../components/console/ErrorBanner'
    );
    const onRetry = vi.fn(() => undefined);
    const { rerender } = render(
      <ErrorBanner message="boom" onRetry={onRetry} />,
    );
    fireEvent.click(screen.getByRole('button', { name: 'Retry' }));
    expect(onRetry).toHaveBeenCalled();
    rerender(<ForbiddenPage permissionId="identity.users.write" />);
    expect(screen.getByText(/identity.users.write/)).toBeTruthy();
  });

  test('ToastProvider push shows operation link', async () => {
    const { ToastProvider, useToast } = await import('../components/console/Toast');
    const { Button } = await import('../components/primitives/button');
    function Probe() {
      const toast = useToast();
      return (
        <Button
          onClick={() =>
            toast.push({
              title: 'Saved',
              tone: 'success',
              operationId: 'op-1',
            })
          }
        >
          Notify
        </Button>
      );
    }
    render(
      <ToastProvider durationMs={60_000}>
        <Probe />
      </ToastProvider>,
    );
    fireEvent.click(screen.getByRole('button', { name: 'Notify' }));
    expect(await screen.findByText('Operation op-1')).toBeTruthy();
  });

  test('FilterChip aria-pressed', async () => {
    const { FilterChip } = await import('../components/console/FilterChip');
    render(<FilterChip label="Active" active />);
    expect(screen.getByRole('button', { name: 'Active' }).getAttribute('aria-pressed')).toBe(
      'true',
    );
  });

  test('Dialog opens with aria-modal', async () => {
    const { Dialog } = await import('../components/console/Dialog');
    render(
      <Dialog open onOpenChange={() => undefined} title="Invite">
        Body
      </Dialog>,
    );
    expect(screen.getByRole('dialog').getAttribute('aria-modal')).toBe('true');
  });

  test('nested FormDialog submit does not submit the form underneath', async () => {
    const { FormDialog } = await import('../components/console/Dialog');
    const parentSubmit = vi.fn(() => undefined);
    const childSubmit = vi.fn((event: { preventDefault: () => void }) => {
      event.preventDefault();
    });
    render(
      <FormDialog
        open
        onOpenChange={() => undefined}
        title="Profile"
        cancelLabel="Cancel"
        submitLabel="Save"
        onSubmit={parentSubmit}
      >
        <FormDialog
          open
          onOpenChange={() => undefined}
          title="Verify"
          cancelLabel="Cancel"
          submitLabel="Submit"
          onSubmit={childSubmit}
        >
          <input aria-label="Name" />
        </FormDialog>
      </FormDialog>,
    );
    const verify = screen.getByRole('dialog', { name: 'Verify' });
    fireEvent.submit(verify.querySelector('form') as HTMLFormElement);
    expect(childSubmit).toHaveBeenCalledTimes(1);
    expect(parentSubmit).not.toHaveBeenCalled();
  });
});

describe('Wave B — P1', () => {

  test('DescriptionList renders items', async () => {
    const { DescriptionList } = await import('../components/console/DescriptionList');
    render(
      <DescriptionList
        items={[
          { label: 'Org', value: 'Acme' },
          { label: 'Role', value: 'Admin' },
        ]}
      />,
    );
    expect(screen.getByText('Acme')).toBeTruthy();
  });

  test('Pagination next/prev and page numbers', async () => {
    const { Pagination, getPaginationItems } = await import(
      '../components/console/Pagination'
    );
    expect(getPaginationItems(5, 20, 1)).toEqual([
      1,
      'ellipsis',
      4,
      5,
      6,
      'ellipsis',
      20,
    ]);

    const onPage = vi.fn(() => undefined);
    render(
      <Pagination page={2} pageSize={10} total={40} onPageChange={onPage} />,
    );
    expect(screen.getByText('11–20 of 40')).toBeTruthy();
    fireEvent.click(screen.getByRole('button', { name: 'Next page' }));
    expect(onPage).toHaveBeenCalledWith(3);
    fireEvent.click(screen.getByRole('button', { name: 'Page 1' }));
    expect(onPage).toHaveBeenCalledWith(1);
  });

  test('WizardShell exposes steps', async () => {
    const { WizardShell } = await import('../components/console/WizardShell');
    render(
      <WizardShell
        steps={[
          { id: '1', title: 'Invite' },
          { id: '2', title: 'Done' },
        ]}
        currentId="1"
        onNext={() => undefined}
      >
        Body
      </WizardShell>,
    );
    expect(screen.getByText('Invite')).toBeTruthy();
    expect(screen.getByText('Body')).toBeTruthy();
  });


  test('DropdownMenu danger item', async () => {
    const { DropdownMenu } = await import('../components/console/DropdownMenu');
    const onDelete = vi.fn(() => undefined);
    render(
      <DropdownMenu
        trigger={<span>More</span>}
        items={[
          { id: 'del', label: 'Delete', tone: 'danger', onSelect: onDelete },
        ]}
      />,
    );
    fireEvent.click(screen.getByRole('button', { name: 'More actions' }));
    fireEvent.click(screen.getByRole('menuitem', { name: 'Delete' }));
    expect(onDelete).toHaveBeenCalled();
  });

  test('Alert DangerZone Skeleton Drawer', async () => {
    const { Alert } = await import('../components/primitives/alert');
    const { DangerZone } = await import('../components/console/DangerZone');
    const { Skeleton } = await import('../components/primitives/skeleton');
    const { Drawer } = await import('../components/console/Drawer');
    const onAction = vi.fn(() => undefined);
    const onOpenChange = vi.fn(() => undefined);
    render(
      <>
        <Alert tone="warn" title="Careful">
          Body
        </Alert>
        <DangerZone actionLabel="Disable" onAction={onAction} />
        <Skeleton className="h-4 w-10" />
        <Drawer open onOpenChange={onOpenChange} title="Detail">
          Drawer body
        </Drawer>
      </>,
    );
    fireEvent.click(screen.getByRole('button', { name: 'Disable' }));
    expect(onAction).toHaveBeenCalled();
    expect(screen.getByText('Drawer body')).toBeTruthy();
    expect(
      document.querySelector('.ui-drawer-root')?.className,
    ).toMatch(/ui-drawer-root/);
    fireEvent.click(screen.getByRole('button', { name: 'Close' }));
    expect(onOpenChange).toHaveBeenCalledWith(false);
  });
});

describe('Wave C — P2', () => {


  test('VirtualizedTable windows rows', async () => {
    const { VirtualizedTable } = await import('../components/console/VirtualizedTable');
    const rows = Array.from({ length: 200 }, (_, i) => ({
      id: String(i),
      name: `Row ${i}`,
    }));
    render(
      <VirtualizedTable
        rows={rows}
        rowKey={(r) => r.id}
        height={120}
        rowHeight={40}
        columns={[{ id: 'name', header: 'Name', cell: (r) => r.name }]}
      />,
    );
    expect(screen.getByText('Row 0')).toBeTruthy();
    expect(screen.queryByText('Row 199')).toBeNull();
  });

  test('CommandPalette filters and selects', async () => {
    const { CommandPalette } = await import('../components/console/CommandPalette');
    const onSelect = vi.fn(() => undefined);
    function Harness() {
      const [open, setOpen] = useState(true);
      return (
        <CommandPalette
          open={open}
          onOpenChange={setOpen}
          items={[
            { id: '1', label: 'Go users', onSelect },
            { id: '2', label: 'Go roles', onSelect: () => undefined },
          ]}
        />
      );
    }
    render(<Harness />);
    fireEvent.change(screen.getByPlaceholderText('Type a command…'), {
      target: { value: 'users' },
    });
    fireEvent.click(screen.getByRole('option', { name: /Go users/ }));
    expect(onSelect).toHaveBeenCalled();
  });

  test('CopyButton copies value', async () => {
    const { CopyButton } = await import('../components/primitives/copy-button');
    const writeText = vi.fn(async () => undefined);
    Object.defineProperty(navigator, 'clipboard', {
      configurable: true,
      value: { writeText },
    });
    render(<CopyButton value="abc" />);
    fireEvent.click(screen.getByRole('button', { name: 'Copy' }));
    expect(writeText).toHaveBeenCalledWith('abc');
  });

  test('CursorPager buttons', async () => {
    const { CursorPager } = await import('../components/console/Pagination');
    const onNext = vi.fn(() => undefined);
    render(
      <CursorPager
        hasNext
        hasPrev={false}
        onNext={onNext}
        onPrev={() => undefined}
      />,
    );
    fireEvent.click(screen.getByRole('button', { name: 'Next page' }));
    expect(onNext).toHaveBeenCalled();
  });

  test('PersonCell RoleChipList DirectorySplit', async () => {
    const { PersonCell } = await import('../components/console/PersonCell');
    const { RoleChipList } = await import('../components/console/RoleChipList');
    const { DirectorySplit } = await import('../components/console/DirectorySplit');
    const { IdentityStatusBadge } = await import(
      '../components/console/IdentityStatusBadge'
    );
    const { MetaSummary } = await import('../components/console/MetaSummary');
    const { NavLinkList } = await import('../components/console/NavLinkList');
    const { DetailSection } = await import('../components/console/DetailSection');
    const { ResourceMetaHeader } = await import(
      '../components/console/ResourceMetaHeader'
    );
    const onChild = vi.fn(() => undefined);
    render(
      <DirectorySplit
        treeLabel="Tree"
        detailLabel="Detail"
        tree={<PersonCell name="Ada" email="ada@kense.local" mark />}
        detail={
          <>
            <ResourceMetaHeader
              variant="flush"
              title="Engineering"
              badge={<IdentityStatusBadge status="active" label="Active" />}
              summary={[
                { id: 'members', label: 'Members', value: 12 },
                { id: 'children', label: 'Children', value: 2 },
              ]}
            />
            <MetaSummary
              items={[{ id: 'x', label: 'Leaders', value: '—' }]}
            />
            <DetailSection title="Children">
              <NavLinkList
                items={[
                  { id: '1', label: 'Platform', onSelect: onChild },
                ]}
              />
            </DetailSection>
            <RoleChipList items={[{ id: '1', label: 'admin' }]} />
          </>
        }
      />,
    );
    expect(screen.getByText('Ada')).toBeTruthy();
    expect(screen.getByText('ada@kense.local')).toBeTruthy();
    expect(screen.getByText('Engineering')).toBeTruthy();
    expect(screen.getByText('Active')).toBeTruthy();
    expect(screen.getByText('admin')).toBeTruthy();
    expect(screen.getByLabelText('Tree')).toBeTruthy();
    fireEvent.click(screen.getByRole('button', { name: 'Platform' }));
    expect(onChild).toHaveBeenCalled();
  });

});
