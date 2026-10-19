import { afterEach, describe, expect, it, vi } from 'vitest'

import {
  approveJoinRequest,
  configureJoinInvite,
  createOrgDepartment,
  createOrgPosition,
  createOrgRole,
  deleteOrgDepartment,
  deleteOrgPosition,
  deleteOrgRole,
  departmentErrorKey,
  fetchJoinInvite,
  listOrgDepartments,
  listOrgMembers,
  listOrgPositions,
  listOrgRoles,
  listRolePolicyAudits,
  memberErrorKey,
  positionErrorKey,
  rejectJoinRequest,
  removeOrgMember,
  renameOrgDepartment,
  renameOrgPosition,
  roleErrorKey,
  setOrgMemberDepartment,
  setOrgMemberPosition,
  setOrgMemberRole,
  updateOrgRole
} from '../orgApi'

const runtime = {
  apiBaseUrl: 'http://127.0.0.1:9',
  token: 'ab'.repeat(32)
}

function installRuntime() {
  Object.defineProperty(globalThis, '__A3S_STUDIO_RUNTIME__', {
    value: runtime,
    configurable: true
  })
}

function envelope(data: unknown, status = 200) {
  return new Response(JSON.stringify({ code: 0, message: 'ok', data }), { status })
}

afterEach(() => {
  Reflect.deleteProperty(globalThis, '__A3S_STUDIO_RUNTIME__')
  vi.unstubAllGlobals()
})

describe('orgApi proxy boundary', () => {
  it.each([undefined, null, '', 'A'.repeat(64), 'a'.repeat(63)])(
    'does not send an invitation write without a canonical reviewed revision: %s',
    (revision) => {
      installRuntime()
      const fetchMock = vi.fn(async () => envelope({}))
      vi.stubGlobal('fetch', fetchMock)
      expect(() => configureJoinInvite('org-1', { rotate: true, expected_revision: revision as string })).toThrow(
        'Invitation revision is unavailable'
      )
      expect(fetchMock).not.toHaveBeenCalled()
    }
  )

  it('maps every call onto the shell member tree with the recorded method', async () => {
    installRuntime()
    const fetchMock = vi.fn(async () => envelope({}))
    vi.stubGlobal('fetch', fetchMock)

    await listOrgMembers('org-1')
    expect(fetchMock).toHaveBeenLastCalledWith(
      'http://127.0.0.1:9/api/member/orgs/org-1/members',
      expect.objectContaining({ method: 'GET' })
    )

    await listOrgDepartments('org-1')
    expect(fetchMock).toHaveBeenLastCalledWith(
      'http://127.0.0.1:9/api/member/orgs/org-1/departments',
      expect.objectContaining({ method: 'GET' })
    )

    await createOrgDepartment('org-1', { parent_id: 'dept-root', name: '研发中心' })
    expect(fetchMock).toHaveBeenLastCalledWith(
      'http://127.0.0.1:9/api/member/orgs/org-1/departments',
      expect.objectContaining({ method: 'POST', body: JSON.stringify({ parent_id: 'dept-root', name: '研发中心' }) })
    )

    await renameOrgDepartment('org-1', 'dept-1', '产品部')
    expect(fetchMock).toHaveBeenLastCalledWith(
      'http://127.0.0.1:9/api/member/orgs/org-1/departments/dept-1',
      expect.objectContaining({ method: 'PUT', body: JSON.stringify({ name: '产品部' }) })
    )

    await deleteOrgDepartment('org-1', 'dept-1')
    expect(fetchMock).toHaveBeenLastCalledWith(
      'http://127.0.0.1:9/api/member/orgs/org-1/departments/dept-1',
      expect.objectContaining({ method: 'DELETE' })
    )

    await listOrgRoles('org-1')
    expect(fetchMock).toHaveBeenLastCalledWith(
      'http://127.0.0.1:9/api/member/orgs/org-1/roles',
      expect.objectContaining({ method: 'GET' })
    )

    await listOrgPositions('org-1')
    expect(fetchMock).toHaveBeenLastCalledWith(
      'http://127.0.0.1:9/api/member/orgs/org-1/positions',
      expect.objectContaining({ method: 'GET' })
    )

    await createOrgPosition('org-1', { name: '产品经理' })
    expect(fetchMock).toHaveBeenLastCalledWith(
      'http://127.0.0.1:9/api/member/orgs/org-1/positions',
      expect.objectContaining({ method: 'POST', body: JSON.stringify({ name: '产品经理' }) })
    )

    await renameOrgPosition('org-1', 'pos-1', '产品负责人')
    expect(fetchMock).toHaveBeenLastCalledWith(
      'http://127.0.0.1:9/api/member/orgs/org-1/positions/pos-1',
      expect.objectContaining({ method: 'PUT', body: JSON.stringify({ name: '产品负责人' }) })
    )

    await deleteOrgPosition('org-1', 'pos-1')
    expect(fetchMock).toHaveBeenLastCalledWith(
      'http://127.0.0.1:9/api/member/orgs/org-1/positions/pos-1',
      expect.objectContaining({ method: 'DELETE' })
    )

    await createOrgRole('org-1', {
      name: '财务',
      description: '管钱',
      data_scope: { kind: 'department_tree', department_ids: [] },
      rules: [{ action: 'invite', subject: 'Member' }]
    })
    expect(fetchMock).toHaveBeenLastCalledWith(
      'http://127.0.0.1:9/api/member/orgs/org-1/roles',
      expect.objectContaining({
        method: 'POST',
        body: JSON.stringify({
          name: '财务',
          description: '管钱',
          data_scope: { kind: 'department_tree', department_ids: [] },
          rules: [{ action: 'invite', subject: 'Member' }]
        })
      })
    )

    await updateOrgRole('org-1', 'role-1', {
      name: '财务',
      description: '',
      rules: [],
      expected_revision: 'a'.repeat(64)
    })
    expect(fetchMock).toHaveBeenLastCalledWith(
      'http://127.0.0.1:9/api/member/orgs/org-1/roles/role-1',
      expect.objectContaining({
        method: 'PUT',
        body: JSON.stringify({ name: '财务', description: '', rules: [], expected_revision: 'a'.repeat(64) })
      })
    )

    await deleteOrgRole('org-1', 'role-1', 'a'.repeat(64))
    expect(fetchMock).toHaveBeenLastCalledWith(
      'http://127.0.0.1:9/api/member/orgs/org-1/roles/role-1',
      expect.objectContaining({ method: 'DELETE', body: JSON.stringify({ expected_revision: 'a'.repeat(64) }) })
    )

    await listRolePolicyAudits('org-1', 'role/1')
    expect(fetchMock).toHaveBeenLastCalledWith(
      'http://127.0.0.1:9/api/member/orgs/org-1/roles/role%2F1/audit',
      expect.objectContaining({ method: 'GET' })
    )

    await setOrgMemberRole('org-1', 'user-1', 'role-2')
    expect(fetchMock).toHaveBeenLastCalledWith(
      'http://127.0.0.1:9/api/member/orgs/org-1/members/user-1/role',
      expect.objectContaining({ method: 'PUT', body: JSON.stringify({ role_id: 'role-2' }) })
    )

    await setOrgMemberPosition('org-1', 'user-1', null)
    expect(fetchMock).toHaveBeenLastCalledWith(
      'http://127.0.0.1:9/api/member/orgs/org-1/members/user-1/position',
      expect.objectContaining({ method: 'PUT', body: JSON.stringify({ position_id: null }) })
    )

    await setOrgMemberDepartment('org-1', 'user-1', 'dept-2')
    expect(fetchMock).toHaveBeenLastCalledWith(
      'http://127.0.0.1:9/api/member/orgs/org-1/members/user-1/department',
      expect.objectContaining({ method: 'PUT', body: JSON.stringify({ department_id: 'dept-2' }) })
    )

    await removeOrgMember('org-1', 'user-1')
    expect(fetchMock).toHaveBeenLastCalledWith(
      'http://127.0.0.1:9/api/member/orgs/org-1/members/user-1',
      expect.objectContaining({ method: 'DELETE' })
    )

    await approveJoinRequest('org-1', 'member-9')
    expect(fetchMock).toHaveBeenLastCalledWith(
      'http://127.0.0.1:9/api/member/orgs/org-1/join-requests/member-9/approval',
      expect.objectContaining({ method: 'PUT' })
    )

    await rejectJoinRequest('org-1', 'member-9')
    expect(fetchMock).toHaveBeenLastCalledWith(
      'http://127.0.0.1:9/api/member/orgs/org-1/join-requests/member-9/rejection',
      expect.objectContaining({ method: 'PUT' })
    )

    await fetchJoinInvite('org-1')
    expect(fetchMock).toHaveBeenLastCalledWith(
      'http://127.0.0.1:9/api/member/orgs/org-1/join-invite',
      expect.objectContaining({ method: 'GET' })
    )

    await configureJoinInvite('org-1', { rotate: true, expected_revision: 'a'.repeat(64) })
    expect(fetchMock).toHaveBeenLastCalledWith(
      'http://127.0.0.1:9/api/member/orgs/org-1/join-invite',
      expect.objectContaining({
        method: 'PUT',
        body: JSON.stringify({ rotate: true, expected_revision: 'a'.repeat(64) })
      })
    )
  })

  it('carries the shell bearer so the stored member session stays out of the renderer', async () => {
    installRuntime()
    const fetchMock = vi.fn<typeof fetch>(async () => envelope([]))
    vi.stubGlobal('fetch', fetchMock)
    await listOrgMembers('org-1')
    const init = fetchMock.mock.calls[0]?.[1] as Record<string, unknown>
    expect(init.headers).toMatchObject({ authorization: `Bearer ${runtime.token}` })
    expect(JSON.stringify(init)).not.toContain('a3s_ms_')
  })

  it('surfaces upstream error messages for the copy mapping', async () => {
    installRuntime()
    vi.stubGlobal(
      'fetch',
      vi.fn(
        async () =>
          new Response(JSON.stringify({ code: 403, message: 'root department cannot be deleted', data: null }), {
            status: 403
          })
      )
    )
    await expect(deleteOrgDepartment('org-1', 'dept-1')).rejects.toMatchObject({
      message: 'root department cannot be deleted'
    })
  })
})

describe('upstream message → copy key mapping', () => {
  it('maps department failures', () => {
    expect(departmentErrorKey(new Error('the root department cannot be deleted'))).toBe('org.members.deptRoot')
    expect(departmentErrorKey(new Error('department still has child departments'))).toBe('org.members.deptHasChildren')
    expect(departmentErrorKey(new Error('department still has users'))).toBe('org.members.deptHasMembers')
    expect(departmentErrorKey(new Error('department name is empty'))).toBe('org.members.deptNameInvalid')
    expect(departmentErrorKey(new Error('network gone'))).toBe('org.error.generic')
  })

  it('maps member failures', () => {
    expect(memberErrorKey(new Error('org must keep at least one admin'))).toBe('org.members.lastAdmin')
    expect(memberErrorKey(new Error('cannot remove yourself'))).toBe('org.members.selfRemove')
    expect(memberErrorKey(new Error('creator role cannot be changed'))).toBe('org.roles.ownerLocked')
    expect(memberErrorKey(new Error('department not found'))).toBe('org.members.deptMissing')
    expect(memberErrorKey(new Error('primary department must belong to the same org'))).toBe('org.members.deptMissing')
    expect(memberErrorKey(new Error('boom'))).toBe('org.error.generic')
  })

  it('maps position and role catalog failures', () => {
    expect(positionErrorKey(new Error('position name already exists'))).toBe('org.positions.duplicate')
    expect(positionErrorKey(new Error('position name is required'))).toBe('org.positions.nameInvalid')
    expect(positionErrorKey(new Error('position still has members; reassign them first'))).toBe(
      'org.positions.hasMembers'
    )
    expect(roleErrorKey(new Error('role name already exists'))).toBe('org.roles.duplicate')
    expect(roleErrorKey(new Error('role name is invalid'))).toBe('org.roles.nameInvalid')
    expect(roleErrorKey(new Error('role description is too long'))).toBe('org.roles.descriptionTooLong')
    expect(roleErrorKey(new Error('built-in roles cannot be deleted'))).toBe('org.roles.builtInLocked')
    expect(roleErrorKey(new Error('reassign members in this role first'))).toBe('org.roles.hasMembers')
    expect(positionErrorKey(new Error('boom'))).toBe('org.error.generic')
  })
})
