import assert from 'node:assert/strict'
import test from 'node:test'

import { authorizeInternalProject } from './internalProjectAccess.ts'

const identity = {
  organizationId: '00000000-0000-4000-8000-000000000001',
  userId: '00000000-0000-4000-8000-000000000002',
  workspaceUserId: '00000000-0000-4000-8000-000000000010',
}
const project = { id: '00000000-0000-4000-8000-000000000003', organizationId: identity.organizationId }

function payloadFor(role: 'owner' | 'admin' | 'member' | 'client', projectRole?: 'viewer' | 'editor' | 'manager') {
  return {
    find: async ({ collection }: { collection: string }) => {
      if (collection === 'workspace-users') return { docs: [{ id: identity.workspaceUserId, organizationId: identity.organizationId, argusUserId: identity.userId, role }] }
      if (collection === 'project-memberships') return { docs: projectRole ? [{ role: projectRole }] : [] }
      return { docs: [] }
    },
  } as any
}

test('workspace owners and admins inherit project access inside their organization', async () => {
  assert.ok(await authorizeInternalProject(payloadFor('owner'), identity, project, 'manager'))
  assert.ok(await authorizeInternalProject(payloadFor('admin'), identity, project, 'manager'))
})

test('members require an explicit membership at or above the requested role', async () => {
  assert.equal(await authorizeInternalProject(payloadFor('member'), identity, project, 'viewer'), null)
  assert.ok(await authorizeInternalProject(payloadFor('member', 'viewer'), identity, project, 'viewer'))
  assert.equal(await authorizeInternalProject(payloadFor('member', 'viewer'), identity, project, 'editor'), null)
  assert.ok(await authorizeInternalProject(payloadFor('member', 'editor'), identity, project, 'editor'))
  assert.equal(await authorizeInternalProject(payloadFor('member', 'editor'), identity, project, 'manager'), null)
  assert.ok(await authorizeInternalProject(payloadFor('member', 'manager'), identity, project, 'manager'))
})

test('project access never crosses the organization boundary', async () => {
  const otherProject = { ...project, organizationId: '00000000-0000-4000-8000-000000000099' }
  assert.equal(await authorizeInternalProject(payloadFor('owner'), identity, otherProject, 'viewer'), null)
})
